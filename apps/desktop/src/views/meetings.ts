import { api, fixtureClient, type MeetingAnswer, type MeetingDetail, type MeetingFinished, type MeetingStatus, type MeetingVoice, type Passage, type Segment } from "../api";
import { SerialQueue, orFallback, persistAll } from "../async";
import { askText, askTextWithSwitch, confirmDialog, errorText, formatClock, h, markdown, menuButton, select } from "../dom";
import { recognitionSummary } from "../speaker-state";
import { MeetingDraft } from "../meeting-draft";
import type { Context } from "../main";
import { icon } from "../icons";
import { languageModelConfigured } from "../presentation";
import { VisibilityPoll } from "../window-lifecycle";
import { sourceBlocks, sourceRange, transcriptSourceBlocks } from "../source-navigation";
import { processingSummary } from "../processing-state";
import { meetingProcessingSummary, processingLabel, retainedMeetingFailure } from "../recording-processing";

const notesQueue = new SerialQueue();
const drafts = new Map<string, MeetingDraft>();
const liveDraft = new MeetingDraft();
const liveTitleDraft = new MeetingDraft();
const titleQueue = new SerialQueue();
let liveMeetingId: string | null = null;
const sessionMeetingIds = new Map<string, string>();
const detailState = new Map<string, { tab: "notes" | "transcript" | "ask"; template: string; working: boolean; message: string }>();
interface Exchange { question: string; answer?: MeetingAnswer; error?: string }
const conversations = new Map<string, { exchanges: Exchange[]; draft: string; scrollTop: number | null }>();
let answerReturn: { scope: string | null; sourceMeetingId: string } | null = null;
let panelNumber = 0;

function dateLabel(date: string) {
  const parsed = new Date(date);
  return Number.isNaN(parsed.valueOf()) ? date : parsed.toLocaleString("nl-BE", { dateStyle: "medium", timeStyle: "short" });
}

function speakerLabel(segment: Segment, detail?: MeetingDetail): string {
  if (segment.speaker === "me") return "Ik";
  if (segment.voice === null) return "Anderen";
  return detail?.speakers.find((name) => name.voice === segment.voice)?.name ?? `Anderen ${segment.voice}`;
}

function schedule(ctx: Context, interval: number, work: () => Promise<void>) {
  const poll = new VisibilityPoll(interval, work, ctx.report);
  poll.setVisible(ctx.visible, false);
  ctx.onVisibilityChange((visible) => poll.setVisible(visible));
  ctx.onDispose(() => poll.dispose());
}

function draftFor(id: string, notes: string) {
  if (liveMeetingId === id && liveDraft.dirty) return liveDraft;
  let draft = drafts.get(id);
  if (!draft) { draft = new MeetingDraft(); drafts.set(id, draft); }
  if (!draft.dirty) draft.sync(id, notes);
  return draft;
}

async function saveDraft(draft: MeetingDraft, meetingId: string) {
  await draft.persist((value) => notesQueue.run(() => api.saveMeetingNotes(meetingId, value)));
}

async function flushLiveTitle() {
  const sessionId = liveTitleDraft.sessionId;
  if (!sessionId) return;
  await liveTitleDraft.persist((title) => titleQueue.run(() => api.setMeetingTitle(title, sessionId)));
}

export async function flushMeetingDrafts() {
  await persistAll([
    async () => { await titleQueue.drain(); await flushLiveTitle(); },
    async () => {
      await notesQueue.drain();
      await persistAll([
        ...[...drafts].map(([id, draft]) => () => saveDraft(draft, id)),
        async () => { if (liveMeetingId) await saveDraft(liveDraft, liveMeetingId); },
      ]);
      await notesQueue.drain();
    },
  ]);
}

export function displayedMeetingSession(): string | null {
  return liveDraft.sessionId;
}

export function finishMeetingDrafts(event: MeetingFinished) {
  if (!event.id) return;
  const oldId = sessionMeetingIds.get(event.session_id);
  if (oldId && oldId !== event.id && drafts.has(oldId)) {
    drafts.set(event.id, drafts.get(oldId)!);
    drafts.delete(oldId);
  }
  sessionMeetingIds.set(event.session_id, event.id);
  if (liveDraft.sessionId === event.session_id) liveMeetingId = event.id;
}

function editNotes(ctx: Context, draft: MeetingDraft, id: string, rows = 8) {
  const input = h("textarea", { rows, placeholder: "Je notities — Pennel vult ze later aan met de meeting.", "aria-label": "Je notities" });
  input.value = draft.value;
  let timer: number | undefined;
  input.addEventListener("input", () => {
    draft.edit(input.value);
    window.clearTimeout(timer);
    timer = window.setTimeout(() => { void saveDraft(draft, id).catch(ctx.report); }, 600);
  });
  const flush = () => { window.clearTimeout(timer); return saveDraft(draft, id); };
  ctx.beforeLeave(flush);
  ctx.onFlush(flush);
  input.addEventListener("blur", () => { void flush().catch(ctx.report); });
  ctx.onDispose(() => { void flush().catch(ctx.report); });
  return { input, flush };
}

/** Names a voice in this meeting; with "Onthoud deze stem" switched on, later meetings name it too. */
async function nameVoice(id: string, voice: number, suggested: string, current: MeetingVoice | undefined, remembering = false): Promise<boolean> {
  if (!current?.can_remember) {
    const name = await askText("Naam voor deze stem", suggested, "Laat leeg om terug ‘Anderen’ te tonen.");
    if (name === null) return false;
    await api.renameSpeaker(id, voice, name);
    return true;
  }
  const answer = await askTextWithSwitch("Naam voor deze stem", suggested,
    "Laat leeg om terug ‘Anderen’ te tonen. Een onthouden stem krijgt in volgende meetings zelf deze naam.", "Onthoud deze stem", remembering || current.remembered);
  if (!answer) return false;
  if (answer.on && answer.text) await api.rememberSpeaker(id, voice, answer.text);
  else await api.renameSpeaker(id, voice, answer.text);
  return true;
}

function transcript(ctx: Context, segments: Segment[], detail?: MeetingDetail, id?: string, voices: MeetingVoice[] = []) {
  const result = h("div", { class: "transcript" });
  if (!segments.length) { result.append(h("p", { class: "muted" }, "Nog geen transcript.")); return result; }
  const play = ctx.info.capabilities.audio_playback;
  for (const [index, segment] of segments.entries()) {
    const available = !!detail?.audio_tracks.some((track) => track.speaker === segment.speaker && track.available);
    const stamp = h("button", { class: "transcript-stamp text-button", disabled: !id || !available || play.status !== "available",
      title: available ? play.reason ?? "Afspelen vanaf dit punt" : "Geen bewaarde audio beschikbaar." }, formatClock(segment.start_ms / 1000));
    if (id) stamp.addEventListener("click", () => { void api.playMeetingAudio(id, segment.speaker, segment.start_ms).catch(ctx.report); });
    const speakerColor = segment.speaker === "me" ? "var(--accent)" : segment.voice === null ? "var(--foreground)" : `var(--voice-${(segment.voice + 3) % 4 + 1})`;
    let speaker: HTMLElement = h("span", { class: "speaker", style: `color:${speakerColor}` }, speakerLabel(segment, detail));
    if (id && segment.voice !== null && segment.speaker === "others") {
      const currentName = detail?.speakers.find((item) => item.voice === segment.voice)?.name ?? "";
      const names = detail?.attendees ?? [];
      const chosen = names.indexOf(currentName);
      const original = chosen >= 0 ? `name:${chosen}` : "current";
      const voice = voices.find((item) => item.voice === segment.voice);
      const offerRemember = !!voice?.can_remember && !voice.remembered && !!currentName;
      const picker = select([
        ...chosen < 0 ? [["current", speakerLabel(segment, detail)] as [string, string]] : [],
        ...names.map((name, index) => [`name:${index}`, name] as [string, string]), ["custom", "Andere naam…"],
        ...offerRemember ? [["remember", "Onthoud deze stem…"] as [string, string]] : [],
      ], original, (value) => {
        void (async () => {
          const suggested = value.startsWith("name:") ? names[Number(value.slice(5))] : currentName;
          try {
            const changed = value.startsWith("name:") && !voice?.can_remember
              ? (await api.renameSpeaker(id, segment.voice!, suggested), true)
              : await nameVoice(id, segment.voice!, suggested, voice, value === "remember");
            if (changed) ctx.rerender(); else picker.value = original;
          } catch (error) { picker.value = original; ctx.report(error); }
        })();
      }, "menu");
      picker.classList.add("speaker", "speaker-picker");
      picker.style.color = speakerColor;
      picker.setAttribute("aria-label", `Naam voor deze stem: ${speakerLabel(segment, detail)}`);
      speaker = picker;
    }
    const body = h("div", { class: "transcript-copy" }, segment.failed ? h("span", { class: "muted" }, "Transcriptie mislukt") : segment.text);
    if (segment.failed && id) {
      const retry = h("button", { class: "text-button", onclick: async () => {
        retry.disabled = true;
        try { await api.retranscribeMeeting(id); ctx.rerender(); }
        catch (error) { ctx.report(error); retry.disabled = false; }
      } }, "Opnieuw proberen");
      body.append(retry);
    }
    result.append(h("div", { class: "transcript-line", "data-start-ms": segment.start_ms, "data-transcript-index": index }, stamp, speaker, body));
  }
  return result;
}

function citationLabel(passage: Passage) {
  const place = passage.kind === "transcript" ? passage.start_ms === null ? "transcript" : formatClock(passage.start_ms / 1000)
    : passage.kind === "summary" ? "samenvatting" : "notities";
  return `${passage.title} · ${dateLabel(passage.start_iso)} · ${place}`;
}

function recoveryNotice(ctx: Context, status: MeetingStatus, latest = () => status, includeTranscription = true): HTMLElement | null {
  const primary = status.persistence_error ?? status.terminal_error ?? status.error;
  const warnings = [...new Set([primary, includeTranscription ? status.transcription_warning : null].filter((message): message is string => !!message))];
  if (!warnings.length && !status.can_retry_persistence) return null;
  const message = h("p", { class: "error", role: status.terminal_error ? "alert" : "status" }, warnings.join(" "));
  const actions = h("div", { class: "row" });
  if (status.can_retry_persistence && status.session_id) {
    const retry = h("button", { class: "btn", onclick: async () => {
      retry.disabled = true;
      try { await flushMeetingDrafts(); await api.retryMeetingPersistence(status.session_id!); ctx.rerender(); }
      catch (error) { message.textContent = errorText(error); retry.disabled = false; }
    } }, "Opnieuw bewaren");
    actions.append(retry);
  }
  if (status.transcript.some((segment) => !segment.failed && segment.text.trim())) {
    const copied = h("span", { class: "help", role: "status" });
    actions.append(h("button", { class: "btn", onclick: async () => {
      try {
        await api.copyText(latest().transcript.filter((segment) => !segment.failed).map((segment) => `${formatClock(segment.start_ms / 1000)} ${speakerLabel(segment)}: ${segment.text}`).join("\n"));
        copied.textContent = "Transcript gekopieerd";
      } catch (error) { message.textContent = errorText(error); }
    } }, "Kopieer transcript"), copied);
  }
  return h("div", { class: "meeting-recovery-notice" }, message, actions);
}

function meetingProcessingNotice(ctx: Context, latest: () => MeetingStatus | undefined = () => undefined, active = false) {
  const element = h("p", { class: "help processing-path" });
  const refresh = () => {
    const label = processingLabel(meetingProcessingSummary(processingSummary(ctx.info.settings, "meeting"), latest()?.recognition, active));
    if (element.textContent !== label) element.textContent = label;
  };
  ctx.onInfoChange(refresh);
  refresh();
  return { element, refresh };
}

function chat(ctx: Context, meetingId: string | null) {
  const key = meetingId ?? "__all__";
  let conversation = conversations.get(key);
  if (!conversation) { conversation = { exchanges: [], draft: "", scrollTop: null }; conversations.set(key, conversation); }
  const state = conversation;
  const busy = state.exchanges.some((exchange) => !exchange.answer && !exchange.error);
  const history = h("div", { class: "chat-history", role: "log", "aria-live": "polite" });
  history.addEventListener("scroll", () => { state.scrollTop = history.scrollTop; });
  if (!state.exchanges.length) history.append(h("div", { class: "chat-placeholder" },
    h("h1", {}, meetingId ? "Vraag het deze meeting" : "Vraag het je meetings"),
    h("p", { class: "muted" }, meetingId ? "Bijvoorbeeld: ‘Wat zijn de actiepunten voor mij?’" : "Bijvoorbeeld: ‘Wat hebben we beslist over het budget?’ of ‘Wanneer sprak ik laatst met Jan?’")));
  for (const exchange of state.exchanges) {
    history.append(h("div", { class: "chat-exchange" }, h("p", { class: "chat-question" }, exchange.question),
      exchange.answer ? markdown(exchange.answer.text) : exchange.error ? h("p", { class: "error" }, exchange.error) : h("progress", { "aria-label": "Antwoord schrijven" }),
      exchange.answer ? h("div", { class: "citations" }, exchange.answer.citations.map((citation) => h("button", {
        class: "text-button citation", onclick: () => {
          state.scrollTop = history.scrollTop;
          answerReturn = { scope: meetingId, sourceMeetingId: citation.passage.meeting_id };
          ctx.navigate("meetings", citation.passage.meeting_id, citation.passage.kind === "transcript" ? citation.passage.start_ms ?? undefined : undefined,
            { kind: citation.passage.kind, start_ms: citation.passage.start_ms, text: citation.passage.text });
        },
      }, `[${citation.number}] ${citationLabel(citation.passage)}`))) : null));
  }
  const input = h("input", { type: "text", class: "grow", value: state.draft, disabled: busy,
    placeholder: meetingId ? "Vraag iets over deze meeting" : "Vraag iets over al je meetings", "aria-label": "Je vraag" });
  const submit = h("button", { class: "btn", disabled: busy || !state.draft.trim() }, busy ? "Bezig…" : "Vraag");
  input.addEventListener("input", () => { state.draft = input.value; submit.disabled = busy || !input.value.trim(); });
  const ask = async () => {
    const question = input.value.trim();
    if (!question || state.exchanges.some((exchange) => !exchange.answer && !exchange.error)) return;
    const history = state.exchanges.flatMap((exchange) => exchange.answer ? [{ question: exchange.question, answer: exchange.answer.text }] : []);
    const exchange: Exchange = { question };
    state.exchanges.push(exchange);
    state.draft = "";
    state.scrollTop = null;
    ctx.rerender();
    try { exchange.answer = await api.askMeetings(question, meetingId, history); }
    catch (error) { exchange.error = errorText(error); }
    ctx.rerender();
  };
  submit.addEventListener("click", () => { void ask(); });
  input.addEventListener("keydown", (event) => { if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); void ask(); } });
  const result = h("section", { class: "chat" }, history, h("div", { class: "chat-composer row" }, input, submit));
  requestAnimationFrame(() => { history.scrollTop = state.scrollTop ?? history.scrollHeight; });
  return result;
}

export async function meetingsView(ctx: Context): Promise<HTMLElement> {
  if (ctx.param === "__chat__") return h("div", { class: "meeting-selected" }, chat(ctx, null));
  if (ctx.param) {
    try { return h("div", { class: "meeting-selected" }, await meetingDetail(ctx, ctx.param)); }
    catch (error) { return h("div", { class: "empty-state" }, h("p", { class: "error" }, errorText(error)), h("button", { class: "btn", onclick: () => ctx.navigate("meetings") }, "Terug naar meetings")); }
  }
  const meetings = await api.meetings("");
  if (meetings.length) return h("div", { class: "empty-state library-placeholder" }, h("p", { class: "muted" }, "Kies een meeting of start een opname."));
  return h("div", { class: "empty-state meeting-library-empty" }, icon("notebook"), h("h2", {}, "Nog geen meetings."),
    h("p", { class: "muted" }, "Start een opname. Je notities en transcript komen hier."),
    h("button", { class: "btn primary", onclick: () => ctx.navigate("live") }, "Start je eerste opname"));
}

async function meetingDetail(ctx: Context, id: string): Promise<HTMLElement> {
  const detail = await api.meetingDetail(id);
  if (!detail.info) return h("div", { class: "empty-state muted" }, "Meeting niet gevonden.");
  const runtime = await api.meetingStatus();
  const active = runtime.busy && runtime.id === id;
  let state = detailState.get(id);
  if (!state) { state = { tab: "notes", template: ctx.info.settings.default_template, working: false, message: "" }; detailState.set(id, state); }
  const fixtureTab = fixtureClient()?.fixture?.meeting_tab;
  if (["notes", "transcript", "ask"].includes(String(fixtureTab))) state.tab = fixtureTab as "notes" | "transcript" | "ask";
  const current = state;
  const canSummarize = languageModelConfigured(ctx.info);
  const playback = ctx.info.capabilities.audio_playback;
  const audioAvailable = detail.audio_tracks.some((track) => track.available);
  const audioReason = !audioAvailable ? "Geen bewaarde audio beschikbaar." : playback.status !== "available" ? playback.reason ?? "Audio afspelen is niet beschikbaar." : null;
  const [voices, speakerSetup] = await Promise.all([orFallback(() => api.meetingVoices(id), [], ctx.report), orFallback(() => api.speakerSetup(), null, ctx.report)]);
  const remembered = voices.filter((voice) => voice.remembered && voice.name).map((voice) => voice.name);
  if (ctx.sourceTarget) current.tab = ctx.sourceTarget.kind === "transcript" ? "transcript" : "notes";
  else if (ctx.jumpMs !== undefined) current.tab = "transcript";
  const templates = await api.meetingTemplates();
  if (!templates.some((template) => template.id === current.template)) current.template = templates[0]?.id ?? "algemeen";
  const notesDraft = draftFor(id, detail.notes);
  const notes = editNotes(ctx, notesDraft, id);
  notes.input.disabled = active;
  if (active) notes.input.title = "Bewerk je notities in het opnamevenster tot de opname is afgerond.";
  const enhanced = h("div", { class: "enhanced-notes" });
  const summaryBlocks: { text: string; element: HTMLElement }[] = [];
  if (detail.summary) {
    const lines = await api.markLines(detail.summary, detail.notes);
    for (const line of lines) {
      const rendered = markdown(line.text);
      rendered.classList.add(line.mine ? "own-line" : "generated-line");
      enhanced.append(rendered);
      summaryBlocks.push({ text: line.text, element: rendered });
    }
  }
  const panelId = `meeting-panel-${++panelNumber}`;
  const tabs = h("div", { class: "segmented detail-tabs", role: "tablist", "aria-label": "Meeting" });
  const panel = h("section", { id: panelId, class: "meeting-panel", role: "tabpanel", tabindex: 0 });
  const draw = () => {
    footer.hidden = current.tab === "ask";
    diarize.hidden = current.tab !== "transcript";
    diarizeReason.hidden = current.tab !== "transcript" || !speakerReason;
    tabs.querySelectorAll("button").forEach((button) => {
      button.setAttribute("aria-selected", String(button.dataset.tab === current.tab));
      button.tabIndex = button.dataset.tab === current.tab ? 0 : -1;
    });
    panel.setAttribute("aria-labelledby", `${panelId}-${current.tab}`);
    if (current.tab === "ask") { panel.replaceChildren(chat(ctx, id)); return; }
    if (current.tab === "transcript") {
      panel.replaceChildren(h("div", { class: "transcript-tools" }, audioReason ? h("span", { class: "help" }, audioReason) : h("span", { class: "grow" }),
        h("button", { class: "text-button", disabled: !!audioReason, title: audioReason, onclick: () => api.stopAudio() }, "Stop audio")),
        ...remembered.length ? [h("p", { class: "help remembered-voices" }, `Onthouden: ${remembered.join(", ")}`)] : [],
        transcript(ctx, detail.transcript, detail, id, voices));
      return;
    }
    panel.replaceChildren(detail.summary ? enhanced : detail.notes.trim() ? h("div", { class: "markdown" }, detail.notes) : h("p", { class: "muted" }, "Nog geen samenvatting."));
    panel.append(h("details", { class: "own-notes", open: !detail.summary }, h("summary", {}, "Je notities"), notes.input));
  };
  for (const [tab, label] of [["notes", "Notities"], ["transcript", "Transcript"], ["ask", "Vragen"]] as const) {
    const button = h("button", { id: `${panelId}-${tab}`, role: "tab", "aria-controls": panelId, "data-tab": tab, onclick: () => { current.tab = tab; draw(); } }, label);
    button.addEventListener("keydown", (event) => {
      const order = ["notes", "transcript", "ask"] as const;
      const index = order.indexOf(current.tab);
      const next = event.key === "ArrowRight" ? (index + 1) % 3 : event.key === "ArrowLeft" ? (index + 2) % 3
        : event.key === "Home" ? 0 : event.key === "End" ? 2 : -1;
      if (next < 0) return;
      event.preventDefault(); current.tab = order[next]; draw();
      tabs.querySelector<HTMLButtonElement>(`[data-tab="${current.tab}"]`)?.focus();
    });
    tabs.append(button);
  }
  const message = h("p", { class: "meeting-message help", role: "status" }, current.message);
  const regenerate = h("button", { class: "btn", disabled: current.working || runtime.busy || !canSummarize,
    title: !canSummarize ? "Kies een taalmodel om een samenvatting te maken." : runtime.busy ? "Wacht tot de opname is afgerond." : null }, current.working ? "Bezig…" : detail.summary ? "Opnieuw genereren" : "Samenvatten");
  regenerate.addEventListener("click", async () => {
    current.working = true; regenerate.disabled = true; message.textContent = "Bezig…";
    try { await notes.flush(); await api.summarizeMeeting(id, current.template); current.message = ""; }
    catch (error) { current.message = errorText(error); }
    current.working = false; ctx.rerender();
  });
  const selected = select(templates.map((template) => [template.id, template.name]), current.template, (template) => { current.template = template; }, "menu");
  selected.setAttribute("aria-label", "Sjabloon");
  selected.disabled = runtime.busy;
  const copied = h("span", { class: "help", role: "status" });
  const copy = async (formatted: boolean, withTranscript = false) => {
    const exported = await api.exportMeeting(id, withTranscript);
    await api.copyText(exported.markdown, formatted ? exported.html : null);
    copied.textContent = withTranscript ? "Gekopieerd met transcript" : "Gekopieerd";
  };
  const rich = ctx.info.capabilities.rich_clipboard;
  const shareAction = (work: () => Promise<unknown>) => () => work().catch(ctx.report);
  const share = menuButton("Delen", [
    { label: "Kopieer opgemaakt", disabled: rich.status !== "available", title: rich.reason, run: shareAction(() => copy(true)) },
    { label: "Kopieer als Markdown", run: shareAction(() => copy(false)) },
    { label: "Kopieer met transcript", run: shareAction(() => copy(rich.status === "available", true)) },
    { label: "Deelnemers…", run: shareAction(async () => {
      const value = await askText("Deelnemers", detail.attendees.join(", "), "Scheid de namen met een komma.");
      if (value === null) return;
      await api.setMeetingAttendees(id, value.split(",").map((name) => name.trim()).filter(Boolean)); ctx.rerender();
    }) },
    { label: "Mail…", run: shareAction(() => api.shareMeeting(id, false, "mail")) },
    { label: "Deel…", disabled: ctx.info.platform !== "macos",
      title: ctx.info.platform === "macos" ? "Open het deelmenu" : "Kopieer je notities of open de meetingmap om te delen.",
      run: shareAction(() => api.shareMeeting(id, false, "share")) },
  ], "share-menu");
  const diarization = ctx.info.capabilities.diarization;
  const cloudRoute = ctx.info.settings.speaker_recognition === "cloud";
  const speakerReason = runtime.busy || detail.info.duration_min <= 0 ? "Rond eerst de opname af."
    : diarization.status !== "available" ? diarization.reason ?? "Sprekers herkennen is niet beschikbaar."
    : !detail.audio_tracks.some((track) => track.available) ? "Geen bewaarde audio beschikbaar."
    : !detail.transcript.some((segment) => !segment.failed) ? "Nog geen transcript."
    : !speakerSetup?.ready ? cloudRoute ? "Log eerst in bij Pennel Cloud in Instellingen → Account." : "Download eerst de sprekermodellen bij Instellingen → Meetings."
    : null;
  const canDiarize = !speakerReason;
  const diarize = h("button", { class: "btn", disabled: !canDiarize || current.working,
    title: speakerReason ?? (cloudRoute ? "Herkent stemmen via Pennel Cloud" : "Herkent stemmen op dit toestel") }, "Sprekers herkennen");
  diarize.addEventListener("click", async () => {
    current.working = true; diarize.disabled = true; message.textContent = "Sprekers herkennen…";
    try { current.message = recognitionSummary(await api.recognizeSpeakers(id)); }
    catch (error) { current.message = errorText(error); }
    current.working = false; ctx.rerender();
  });
  const diarizeReason = h("p", { class: "help meeting-control-reason", hidden: true }, speakerReason);
  const footer = h("footer", { class: "meeting-footer" }, h("div", { class: "row" }, selected, regenerate, h("span", { class: "grow" }), copied, diarize, share,
    h("button", { class: "btn", onclick: () => api.openPath(detail.dir) }, ctx.info.platform === "macos" ? "Toon in Finder" : "Open map")), diarizeReason);
  const summarySetup = !detail.summary && !canSummarize ? h("div", { class: "meeting-summary-setup help" }, "Kies een taalmodel om een samenvatting te maken.", h("button", { class: "text-button", onclick: () => ctx.navigate("models") }, "Modellen instellen")) : null;
  const conflicts = ctx.info.settings.cloud_sync === true ? await orFallback(() => api.meetingConflictFiles(id), [], ctx.report) : [];
  const banners = conflicts.map((name) => h("div", { class: "conflict-banner", role: "status" }, icon("warning"),
    h("span", {}, "Er is een conflictversie van dit bestand."),
    h("button", { class: "btn", onclick: () => { void api.openPath(detail.dir).catch(ctx.report); } }, "Bekijken"),
    h("button", { class: "btn danger", onclick: async () => {
      if (!await confirmDialog("Conflictversie verwijderen?", `${name} wordt verwijderd.`, "Verwijderen")) return;
      try { await api.deleteConflictCopy(id, name); } catch (error) { ctx.report(error); }
      ctx.rerender();
    } }, "Verwijderen")));
  const result = h("article", { class: "meeting-detail" },
    answerReturn?.sourceMeetingId === id ? h("button", { class: "text-button source-back", onclick: () => {
      const origin = answerReturn;
      answerReturn = null;
      if (!origin) return;
      if (origin.scope) {
        const state = detailState.get(origin.scope);
        if (state) state.tab = "ask";
        ctx.navigate("meetings", origin.scope);
      } else ctx.navigate("chat");
    } }, "Terug naar antwoord") : null,
    h("header", { class: "meeting-header" }, h("div", { class: "grow" }, h("h1", {}, detail.info.title),
      h("p", { class: "help" }, [dateLabel(detail.info.start_iso), detail.info.duration_min ? `${detail.info.duration_min} min` : "", detail.attendees.join(", ")].filter(Boolean).join(" · "))), tabs),
    runtime.id === id && runtime.recognition ? meetingProcessingNotice(ctx, () => runtime, true).element : null,
    ...banners, runtime.id === id ? recoveryNotice(ctx, runtime) : null, summarySetup, message, panel, footer);
  draw();
  if (ctx.sourceTarget) {
    const source = ctx.sourceTarget;
    requestAnimationFrame(() => {
      if (!result.isConnected) return;
      let matched = false;
      const highlight = (elements: HTMLElement[]) => {
        if (!elements.length) return;
        elements.forEach((element) => element.classList.add("source-highlight"));
        elements[0].tabIndex = -1;
        elements[0].focus({ preventScroll: true });
        elements[0].scrollIntoView({ block: "center" });
        matched = true;
      };
      if (source.kind === "notes") {
        const range = sourceRange(notes.input.value, source.text);
        const disclosure = result.querySelector<HTMLDetailsElement>(".own-notes");
        if (disclosure) disclosure.open = true;
        if (range) {
          notes.input.classList.add("source-highlight");
          notes.input.focus({ preventScroll: true });
          notes.input.setSelectionRange(range.start, range.end);
          notes.input.scrollIntoView({ block: "center" });
          matched = true;
        }
      } else if (source.kind === "summary") {
        const range = sourceBlocks(summaryBlocks.map((block) => block.text), source.text);
        if (range) highlight(summaryBlocks.slice(range.first, range.last + 1).map((block) => block.element));
      } else {
        const candidates = detail.transcript.map((segment, index) => ({ ...segment, index, text: `${speakerLabel(segment, detail)}: ${segment.text.trim()}` })).filter((segment) => !segment.failed);
        const range = transcriptSourceBlocks(candidates, source);
        if (range) highlight(candidates.slice(range.first, range.last + 1).flatMap((segment) => {
          const element = result.querySelector<HTMLElement>(`[data-transcript-index="${segment.index}"]`);
          return element ? [element] : [];
        }));
      }
      if (!matched) message.textContent = "Deze bron is intussen gewijzigd of niet meer beschikbaar.";
    });
  } else if (ctx.jumpMs !== undefined) {
    const jump = ctx.jumpMs;
    requestAnimationFrame(() => {
      const target = detail.transcript.find((segment) => segment.start_ms <= jump && segment.end_ms > jump)
        ?? detail.transcript.find((segment) => segment.start_ms >= jump);
      const line = target ? result.querySelector<HTMLElement>(`[data-start-ms="${target.start_ms}"]`) : null;
      line?.classList.add("highlight"); line?.scrollIntoView({ block: "center" });
    });
  }
  return result;
}

export async function meetingView(ctx: Context): Promise<HTMLElement> {
  const initial = await api.meetingStatus();
  if (retainedMeetingFailure(initial)) {
    return h("div", { class: "recording-view retained-recording" }, h("h1", {}, initial.title ?? "Opname bewaren"),
      h("p", { class: "help" }, `Opname gestopt · ${formatClock(initial.elapsed_secs)}`),
      h("p", { class: "muted" }, "Bewaar de resterende gegevens voordat je een nieuwe opname start."),
      meetingProcessingNotice(ctx, () => initial, true).element, recoveryNotice(ctx, initial));
  }
  if (!initial.busy && ctx.param) return meetingDetail(ctx, ctx.param);
  if (!initial.busy) {
    const title = h("input", { type: "text", placeholder: "Titel, bv. Weekly sync", class: "grow", "aria-label": "Titel van de meeting" });
    const status = h("p", { class: "error", role: "status" }, initial.error ?? "");
    const start = h("button", { class: "btn primary" }, "Start opname");
    const retained = !!initial.recognition && !!(initial.persistence_error || initial.terminal_error);
    const processing = meetingProcessingNotice(ctx, () => retained ? initial : undefined, retained);
    const audioReason = h("p", { class: "help" });
    const refreshAudioReason = () => {
      audioReason.textContent = ctx.info.capabilities.system_audio.reason ?? "";
      audioReason.hidden = !audioReason.textContent;
    };
    ctx.onInfoChange(refreshAudioReason);
    refreshAudioReason();
    start.addEventListener("click", async () => {
      start.disabled = true;
      try { await api.startMeeting(title.value); ctx.rerender(); }
      catch (error) { status.textContent = errorText(error); start.disabled = false; }
    });
    return h("div", { class: "recording-view" }, h("h1", {}, "Nieuwe meeting"),
      h("p", { class: "muted" }, "Pennel neemt je microfoon op als ‘Ik’ en wat je computer afspeelt als ‘Anderen’."),
      processing.element, h("div", { class: "row" }, title, start), status, recoveryNotice(ctx, initial), audioReason,
      h("button", { class: "text-button", onclick: () => api.showWindow("settings", "recording") }, "Opnemen instellen…"));
  }
  if (!initial.id) {
    schedule(ctx, 500, async () => { const status = await api.meetingStatus(); if (status.id || !status.busy) ctx.rerender(); });
    return h("div", { class: "recording-view" }, h("h1", {}, "Opname starten…"), meetingProcessingNotice(ctx, () => initial, true).element, h("progress", { "aria-label": "Opname starten" }));
  }
  if (liveDraft.dirty && liveMeetingId && liveDraft.sessionId !== initial.session_id) {
    const retained = new MeetingDraft();
    retained.sync(liveDraft.sessionId, liveDraft.value);
    retained.edit(liveDraft.value);
    drafts.set(liveMeetingId, retained);
  }
  liveDraft.sync(initial.session_id ?? initial.id, initial.notes);
  liveTitleDraft.sync(initial.session_id, initial.title ?? "Meeting");
  liveMeetingId = initial.id;
  if (initial.session_id) sessionMeetingIds.set(initial.session_id, initial.id);
  const notes = editNotes(ctx, liveDraft, initial.id!);
  notes.input.classList.add("live-notes");
  notes.input.placeholder = "Typ steekwoorden… of laat het leeg voor een samenvatting.";
  const title = h("input", { type: "text", class: "live-title", value: liveTitleDraft.value, "aria-label": "Titel van de meeting" });
  title.addEventListener("input", () => { liveTitleDraft.edit(title.value); void flushLiveTitle().catch(ctx.report); });
  title.addEventListener("blur", () => { void flushLiveTitle().catch(ctx.report); });
  ctx.onDispose(() => { void flushLiveTitle().catch(ctx.report); });
  const clock = h("span", { class: "live-clock" });
  const warning = h("p", { class: "help", role: "status" });
  const recovery = h("div", { class: "meeting-recovery" });
  let latestStatus = initial;
  const processing = meetingProcessingNotice(ctx, () => latestStatus, true);
  let recoveryKey = "";
  const consent = h("p", { class: "help", role: "status", hidden: true }, "Toestemmingszin gekopieerd — plak hem in de chat van je call.");
  const output = h("div", { class: "live-transcript" });
  const phase = h("span", { class: "help", role: "status" });
  const stop = h("button", { class: "btn", disabled: initial.phase !== "recording" }, "Stop opname");
  const update = (status: MeetingStatus) => {
    latestStatus = status;
    processing.refresh();
    if (document.activeElement !== title && !liveTitleDraft.dirty) {
      liveTitleDraft.sync(status.session_id, status.title ?? "Meeting");
      title.value = liveTitleDraft.value;
    }
    title.disabled = status.phase !== "recording";
    clock.textContent = `Opname loopt · ${formatClock(status.elapsed_secs)}`;
    phase.textContent = status.phase === "starting" ? "Opname starten…" : status.phase === "finalizing" || status.phase === "stopping"
      ? `Opname afronden…${status.pending ? ` ${status.pending} ${status.pending === 1 ? "stuk" : "stukken"} te transcriberen` : ""}` : "";
    const audioWarning = status.audio_warning ?? (!status.system_audio ? "Systeemgeluid niet beschikbaar — alleen je microfoon wordt opgenomen."
      : !status.heard_system_audio && status.elapsed_secs > 20 ? "Nog geen systeemgeluid gehoord." : "");
    const warningText = [status.transcription_warning, audioWarning].filter(Boolean).join(" ");
    if (warning.textContent !== warningText) warning.textContent = warningText;
    const nextRecovery = JSON.stringify([status.terminal_error, status.persistence_error, status.can_retry_persistence, status.session_id]);
    if (nextRecovery !== recoveryKey) {
      recoveryKey = nextRecovery;
      const notice = recoveryNotice(ctx, status, () => latestStatus, false);
      recovery.replaceChildren(...(notice ? [notice] : []));
    }
    stop.disabled = status.phase !== "recording";
    notes.input.disabled = status.phase !== "recording";
    consent.hidden = !(status.consent_copied ?? false);
    output.replaceChildren(transcript(ctx, status.transcript));
  };
  update(initial);
  stop.addEventListener("click", async () => {
    stop.disabled = true;
    try { await flushLiveTitle(); await notes.flush(); await api.stopMeeting(); ctx.rerender(); }
    catch (error) { warning.textContent = errorText(error); stop.disabled = false; }
  });
  schedule(ctx, 1000, async () => {
    const status = await api.meetingStatus();
    if (!status.busy || status.session_id !== initial.session_id) { ctx.rerender(); return; }
    update(status);
  });
  return h("div", { class: "recording-view live-recording" }, title,
    h("div", { class: "row" }, h("span", { class: "status-dot" }), clock, h("span", { class: "grow" }), stop), processing.element, consent, phase, warning, recovery, notes.input,
    h("details", { class: "live-transcript-disclosure" }, h("summary", {}, "Live transcript"), output));
}
