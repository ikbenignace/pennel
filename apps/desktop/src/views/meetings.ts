import { api, fixtureClient, type MeetingAnswer, type MeetingDetail, type MeetingFinished, type MeetingStatus, type Passage, type Segment } from "../api";
import { LatestRequest, SerialQueue, orFallback, persistAll } from "../async";
import { askText, confirmDialog, errorText, formatClock, h, markdown, select, snippet } from "../dom";
import { MeetingDraft } from "../meeting-draft";
import { badgeFor, resolveOpenMeeting, syncActive, type SyncItemState } from "../sync-badge";
import type { Context } from "../main";
import { icon } from "../icons";
import { languageModelConfigured } from "../presentation";

let query = "";
const notesQueue = new SerialQueue();
const drafts = new Map<string, MeetingDraft>();
const liveDraft = new MeetingDraft();
const liveTitleDraft = new MeetingDraft();
const titleQueue = new SerialQueue();
let liveMeetingId: string | null = null;
const sessionMeetingIds = new Map<string, string>();
const detailState = new Map<string, { tab: "notes" | "transcript" | "ask"; template: string; working: boolean; message: string }>();
interface Exchange { question: string; answer?: MeetingAnswer; error?: string }
const conversations = new Map<string, { exchanges: Exchange[]; draft: string }>();

function dateLabel(date: string) {
  const parsed = new Date(date);
  return Number.isNaN(parsed.valueOf()) ? date : parsed.toLocaleString("nl-BE", { dateStyle: "medium", timeStyle: "short" });
}

function dayGroup(date: string) {
  const day = new Date(date);
  const today = new Date(); today.setHours(0, 0, 0, 0);
  const week = new Date(today); week.setDate(week.getDate() - 7);
  if (day >= today) return "Vandaag";
  return day >= week ? "Deze week" : "Eerder";
}

function speakerLabel(segment: Segment, detail?: MeetingDetail): string {
  if (segment.speaker === "me") return "Ik";
  if (segment.voice === null) return "Anderen";
  return detail?.speakers.find((name) => name.voice === segment.voice)?.name ?? `Anderen ${segment.voice}`;
}

function schedule(ctx: Context, interval: number, work: () => Promise<void>) {
  let running = false;
  const timer = window.setInterval(() => {
    if (running) return;
    running = true;
    void work().catch(ctx.report).finally(() => { running = false; });
  }, interval);
  ctx.onDispose(() => window.clearInterval(timer));
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
  input.addEventListener("blur", () => { void flush().catch(ctx.report); });
  ctx.onDispose(() => { void flush().catch(ctx.report); });
  return { input, flush };
}

function transcript(ctx: Context, segments: Segment[], detail?: MeetingDetail, id?: string) {
  const result = h("div", { class: "transcript" });
  if (!segments.length) { result.append(h("p", { class: "muted" }, "Nog geen transcript.")); return result; }
  const play = ctx.info.capabilities.audio_playback;
  for (const segment of segments) {
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
      const picker = select([
        ...chosen < 0 ? [["current", speakerLabel(segment, detail)] as [string, string]] : [],
        ...names.map((name, index) => [`name:${index}`, name] as [string, string]), ["custom", "Andere naam…"],
      ], original, (value) => {
        void (async () => {
          const name = value.startsWith("name:") ? names[Number(value.slice(5))]
            : value === "custom" ? await askText("Naam voor deze stem", currentName, "Laat leeg om terug ‘Anderen’ te tonen.") : null;
          if (name === null || name === undefined) { picker.value = original; return; }
          try { await api.renameSpeaker(id, segment.voice!, name); ctx.rerender(); }
          catch (error) { picker.value = original; ctx.report(error); }
        })();
      });
      picker.className = "speaker speaker-picker";
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
    result.append(h("div", { class: "transcript-line", "data-start-ms": segment.start_ms }, stamp, speaker, body));
  }
  return result;
}

function citationLabel(passage: Passage) {
  const place = passage.kind === "transcript" ? passage.start_ms === null ? "transcript" : formatClock(passage.start_ms / 1000)
    : passage.kind === "summary" ? "samenvatting" : "notities";
  return `${passage.title} · ${dateLabel(passage.start_iso)} · ${place}`;
}

function chat(ctx: Context, meetingId: string | null) {
  const key = meetingId ?? "__all__";
  let conversation = conversations.get(key);
  if (!conversation) { conversation = { exchanges: [], draft: "" }; conversations.set(key, conversation); }
  const state = conversation;
  const busy = state.exchanges.some((exchange) => !exchange.answer && !exchange.error);
  const history = h("div", { class: "chat-history", role: "log", "aria-live": "polite" });
  if (!state.exchanges.length) history.append(h("div", { class: "chat-placeholder" },
    h("h1", {}, meetingId ? "Vraag het deze meeting" : "Vraag het je meetings"),
    h("p", { class: "muted" }, meetingId ? "Bijvoorbeeld: ‘Wat zijn de actiepunten voor mij?’" : "Bijvoorbeeld: ‘Wat hebben we beslist over het budget?’ of ‘Wanneer sprak ik laatst met Jan?’")));
  for (const exchange of state.exchanges) {
    history.append(h("div", { class: "chat-exchange" }, h("p", { class: "chat-question" }, exchange.question),
      exchange.answer ? markdown(exchange.answer.text) : exchange.error ? h("p", { class: "error" }, exchange.error) : h("progress", { "aria-label": "Antwoord schrijven" }),
      exchange.answer ? h("div", { class: "citations" }, exchange.answer.citations.map((citation) => h("button", {
        class: "text-button citation", onclick: () => ctx.navigate("meetings", citation.passage.meeting_id, citation.passage.kind === "transcript" ? citation.passage.start_ms ?? undefined : undefined),
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
    ctx.rerender();
    try { exchange.answer = await api.askMeetings(question, meetingId, history); }
    catch (error) { exchange.error = errorText(error); }
    ctx.rerender();
  };
  submit.addEventListener("click", () => { void ask(); });
  input.addEventListener("keydown", (event) => { if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); void ask(); } });
  const result = h("section", { class: "chat" }, history, h("div", { class: "chat-composer row" }, input, submit));
  requestAnimationFrame(() => { history.scrollTop = history.scrollHeight; });
  return result;
}

export async function meetingsView(ctx: Context): Promise<HTMLElement> {
  if (ctx.param === "__chat__") return h("div", { class: "meeting-selected" },
    h("button", { class: "text-button meeting-back", onclick: () => ctx.navigate("meetings") }, icon("arrowLeft"), "Alle vergaderingen"), chat(ctx, null));
  if (ctx.param) {
    try { return h("div", { class: "meeting-selected" }, h("button", { class: "text-button meeting-back", onclick: () => ctx.navigate("meetings") }, icon("arrowLeft"), "Alle vergaderingen"), await meetingDetail(ctx, ctx.param)); }
    catch (error) { return h("div", { class: "empty-state" }, h("p", { class: "error" }, errorText(error)), h("button", { class: "btn", onclick: () => ctx.navigate("meetings") }, "Terug naar meetings")); }
  }
  const sidebar = h("aside", { class: "library-sidebar", "aria-label": "Meetingbibliotheek" });
  const input = h("input", { type: "search", value: query, placeholder: "Zoeken", "aria-label": "Zoeken in meetings" });
  const list = h("div", { class: "meeting-list" });
  const live = h("button", { class: "live-meeting text-button", hidden: true, onclick: () => api.showWindow("live") });
  const requests = new LatestRequest();
  let syncOn = ctx.info.settings.cloud_sync === true;
  if (syncOn) syncOn = syncActive(true, (await api.cloudSyncLast().catch(() => null))?.status.kind);
  let syncStates = new Map<string, SyncItemState>();
  let shown: { id: string; title: string; start_iso: string }[] = [];
  const loadStates = async () => {
    syncStates = syncOn ? new Map((await api.meetingSyncStates()).map((item) => [item.id, item.state])) : new Map();
  };
  const load = () => requests.run(async () => query.trim() ? api.searchLibrary(query) : { meetings: await api.meetings(""), passages: [] as Passage[] }, (result) => {
    list.replaceChildren();
    shown = result.meetings;
    if (!result.meetings.length) {
      list.append(h("div", { class: "meeting-library-empty" }, icon("notebook"), h("h2", {}, query.trim() ? "Geen meetings gevonden." : "Nog geen vergaderingen."),
        h("p", { class: "muted" }, query.trim() ? "Probeer een andere zoekterm." : "Start een opname. Je notities en transcript komen hier."),
        !query.trim() ? h("button", { class: "btn primary", onclick: () => ctx.navigate("live") }, "Start je eerste opname") : null)); return;
    }
    let group = "";
    for (const meeting of result.meetings) {
      const day = dayGroup(meeting.start_iso);
      if (day !== group) { group = day; list.append(h("h3", { class: "meeting-day" }, day)); }
      const badge = badgeFor(syncStates.get(meeting.id), syncOn);
      const selectMeeting = h("button", { class: "meeting-select", "aria-current": ctx.param === meeting.id ? "true" : null,
        onclick: () => ctx.navigate("meetings", meeting.id) },
        h("span", { class: "meeting-name" }, meeting.title,
          badge ? h("span", { class: `sync-badge ${syncStates.get(meeting.id)}`, title: badge.label, role: "img", "aria-label": badge.label }, badge.icon) : null),
        h("span", { class: "help" }, `${dateLabel(meeting.start_iso)}${meeting.duration_min ? ` · ${meeting.duration_min} min` : ""}`),
        result.passages.filter((passage) => passage.meeting_id === meeting.id).slice(0, 2).map((passage) => snippet(passage.snippet)));
      const rename = async () => {
        const title = await askText("Meeting hernoemen", meeting.title);
        if (!title) return;
        const id = await api.renameMeeting(meeting.id, title);
        ctx.navigate("meetings", id);
      };
      const remove = async () => {
        if (!await confirmDialog(`${meeting.title} verwijderen?`, "Notities, transcript en audio worden verwijderd.", "Verwijder")) return;
        await api.deleteMeeting(meeting.id);
        if (ctx.param === meeting.id) ctx.navigate("meetings"); else ctx.rerender();
      };
      const menu = h("details", { class: "meeting-menu" }, h("summary", { "aria-label": `Acties voor ${meeting.title}` }, "⋯"),
        h("div", { class: "menu-panel" }, h("button", { class: "text-button", onclick: rename }, "Hernoemen…"),
          h("button", { class: "text-button danger", onclick: remove }, "Verwijder…")));
      list.append(h("div", { class: `meeting-row${ctx.param === meeting.id ? " selected" : ""}` }, selectMeeting, menu));
    }
  }, ctx.report);
  let searchTimer: number | undefined;
  input.addEventListener("input", () => { query = input.value; window.clearTimeout(searchTimer); searchTimer = window.setTimeout(() => { void load(); }, 180); });
  ctx.onDispose(() => { window.clearTimeout(searchTimer); requests.cancel(); });
  await loadStates().catch(ctx.report);
  await load();
  {
    const syncQueue = new SerialQueue();
    let disposed = false;
    const unlisten = api.onSyncChanged((report) => {
      void syncQueue.run(async () => {
        if (disposed) return;
        const wasOn = syncOn;
        syncOn = syncActive(ctx.info.settings.cloud_sync === true, report.status.kind);
        if (!syncOn) {
          syncStates = new Map();
          if (wasOn) await load();
          return;
        }
        const open = ctx.param && ctx.param !== "__chat__" ? ctx.param : null;
        const remembered = shown.find((meeting) => meeting.id === open) ?? null;
        const previous = open ? syncStates.get(open) : undefined;
        await loadStates();
        const all = await api.meetings("");
        if (disposed) return;
        if (open) {
          const resolved = resolveOpenMeeting(open, all, remembered);
          if (resolved !== open) { ctx.navigate("meetings", resolved); return; }
          if ((previous === "conflict") !== (syncStates.get(open) === "conflict")) { ctx.rerender(); return; }
        }
        await load();
      }).catch(ctx.report);
    });
    ctx.onDispose(() => { disposed = true; void unlisten.then((stop) => stop()).catch(() => undefined); });
  }
  const updateLive = async () => {
    const status = await api.meetingStatus();
    live.hidden = !status.busy;
    live.textContent = `● ${status.title ?? "Meeting"}${status.phase === "finalizing" ? " · Afronden…" : ""}`;
  };
  await updateLive();
  schedule(ctx, 2000, updateLive);
  sidebar.append(h("div", { class: "page-heading" }, h("h1", {}, "Vergaderingen"), h("span", { class: "grow" }), h("button", { class: "btn", onclick: () => ctx.navigate("live") }, "Nieuwe opname")),
    h("div", { class: "library-controls row" }, h("div", { class: "library-search" }, input), h("span", { class: "grow" }),
      h("button", { class: "text-button", onclick: () => ctx.navigate("chat") }, "Vraag het je meetings")), live, list);
  return h("div", { class: "library library-overview" }, sidebar);
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
  let speakerModelReady = false;
  try { speakerModelReady = (await api.modelState(ctx.info.diarizer_model_id)).kind === "downloaded"; } catch { /* The unavailable model remains actionable in Meetings settings. */ }
  if (ctx.jumpMs !== undefined) current.tab = "transcript";
  const templates = await api.meetingTemplates();
  if (!templates.some((template) => template.id === current.template)) current.template = templates[0]?.id ?? "algemeen";
  const notesDraft = draftFor(id, detail.notes);
  const notes = editNotes(ctx, notesDraft, id);
  notes.input.disabled = active;
  if (active) notes.input.title = "Bewerk je notities in het opnamevenster tot de opname is afgerond.";
  const enhanced = h("div", { class: "enhanced-notes" });
  if (detail.summary) {
    const lines = await api.markLines(detail.summary, detail.notes);
    for (const line of lines) {
      const rendered = markdown(line.text);
      rendered.classList.add(line.mine ? "own-line" : "generated-line");
      enhanced.append(rendered);
    }
  }
  const tabs = h("div", { class: "segmented detail-tabs", role: "tablist", "aria-label": "Meeting" });
  const panel = h("section", { class: "meeting-panel", role: "tabpanel" });
  const draw = () => {
    footer.hidden = current.tab === "ask";
    diarize.hidden = current.tab !== "transcript";
    diarizeReason.hidden = current.tab !== "transcript" || !speakerReason;
    tabs.querySelectorAll("button").forEach((button) => {
      button.setAttribute("aria-selected", String(button.dataset.tab === current.tab));
      button.tabIndex = button.dataset.tab === current.tab ? 0 : -1;
    });
    if (current.tab === "ask") { panel.replaceChildren(chat(ctx, id)); return; }
    if (current.tab === "transcript") {
      panel.replaceChildren(h("div", { class: "transcript-tools" }, audioReason ? h("span", { class: "help" }, audioReason) : h("span", { class: "grow" }),
        h("button", { class: "text-button", disabled: !!audioReason, title: audioReason, onclick: () => api.stopAudio() }, "Stop audio")), transcript(ctx, detail.transcript, detail, id));
      return;
    }
    panel.replaceChildren(detail.summary ? enhanced : detail.notes.trim() ? h("div", { class: "markdown" }, detail.notes) : h("p", { class: "muted" }, "Nog geen samenvatting."));
    panel.append(h("details", { class: "own-notes", open: !detail.summary }, h("summary", {}, "Je notities"), notes.input));
  };
  for (const [tab, label] of [["notes", "Notities"], ["transcript", "Transcript"], ["ask", "Vragen"]] as const) {
    const button = h("button", { role: "tab", "data-tab": tab, onclick: () => { current.tab = tab; draw(); } }, label);
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
  const selected = select(templates.map((template) => [template.id, template.name]), current.template, (template) => { current.template = template; });
  selected.disabled = runtime.busy;
  const share = h("details", { class: "share-menu" }, h("summary", { class: "btn" }, "Delen"));
  const copied = h("span", { class: "help", role: "status" });
  const copy = async (formatted: boolean, withTranscript = false) => {
    const exported = await api.exportMeeting(id, withTranscript);
    await api.copyText(exported.markdown, formatted ? exported.html : null);
    copied.textContent = withTranscript ? "Gekopieerd met transcript" : "Gekopieerd";
    share.open = false;
  };
  const rich = ctx.info.capabilities.rich_clipboard;
  share.append(h("div", { class: "menu-panel" },
    h("button", { class: "text-button", disabled: rich.status !== "available", title: rich.reason, onclick: () => copy(true) }, "Kopieer opgemaakt"),
    h("button", { class: "text-button", onclick: () => copy(false) }, "Kopieer als Markdown"),
    h("button", { class: "text-button", onclick: () => copy(rich.status === "available", true) }, "Kopieer met transcript"),
    h("button", { class: "text-button", onclick: async () => {
      const value = await askText("Deelnemers", detail.attendees.join(", "), "Scheid de namen met een komma.");
      if (value === null) return;
      await api.setMeetingAttendees(id, value.split(",").map((name) => name.trim()).filter(Boolean)); ctx.rerender();
    } }, "Deelnemers…"),
    h("button", { class: "text-button", onclick: () => api.shareMeeting(id, false, "mail") }, "Mail…"),
    h("button", { class: "text-button", disabled: ctx.info.platform !== "macos",
      title: ctx.info.platform === "macos" ? "Open het deelmenu" : "Kopieer je notities of open de meetingmap om te delen.",
      onclick: () => api.shareMeeting(id, false, "share") }, "Deel…")));
  const diarization = ctx.info.capabilities.diarization;
  const speakerReason = runtime.busy || detail.info.duration_min <= 0 ? "Rond eerst de opname af."
    : diarization.status !== "available" ? diarization.reason ?? "Sprekers herkennen is niet beschikbaar."
    : !detail.audio_tracks.some((track) => track.speaker === "others" && track.available) ? "Geen bewaard ‘Anderen’-spoor beschikbaar."
    : !detail.transcript.some((segment) => segment.speaker === "others" && !segment.failed) ? "Geen transcript van andere sprekers beschikbaar."
    : !speakerModelReady ? "Download eerst het sprekermodel bij Instellingen → Meetings." : null;
  const canDiarize = !speakerReason;
  const diarize = h("button", { class: "btn", disabled: !canDiarize || current.working,
    title: speakerReason ?? "Herkent stemmen met het lokale sprekermodel" }, "Sprekers herkennen");
  diarize.addEventListener("click", async () => {
    current.working = true; diarize.disabled = true; message.textContent = "Sprekers herkennen…";
    try { const voices = await api.diarizeMeeting(id); current.message = `${voices} stemmen herkend.`; }
    catch (error) { current.message = errorText(error); }
    current.working = false; ctx.rerender();
  });
  const diarizeReason = h("p", { class: "help meeting-control-reason", hidden: true }, speakerReason);
  const footer = h("footer", { class: "meeting-footer" }, h("div", { class: "row" }, selected, regenerate, h("span", { class: "grow" }), copied, diarize, share,
    h("button", { class: "btn", onclick: () => api.openPath(detail.dir) }, ctx.info.platform === "macos" ? "Toon in Finder" : "Open map")), diarizeReason);
  const summarySetup = !detail.summary && !canSummarize ? h("div", { class: "meeting-summary-setup help" }, "Kies een taalmodel om een samenvatting te maken.", h("button", { class: "text-button", onclick: () => ctx.navigate("models") }, "Modellen instellen")) : null;
  const conflicts = ctx.info.settings.cloud_sync === true ? await orFallback(() => api.meetingConflictFiles(id), [], ctx.report) : [];
  const banners = conflicts.map((name) => h("div", { class: "conflict-banner row", role: "status" },
    h("span", { class: "grow" }, "Er is een conflictversie van dit bestand."),
    h("button", { class: "btn", onclick: () => { void api.openPath(detail.dir).catch(ctx.report); } }, "Bekijken"),
    h("button", { class: "btn danger", onclick: async () => {
      if (!await confirmDialog("Conflictversie verwijderen?", `${name} wordt verwijderd.`, "Verwijderen")) return;
      try { await api.deleteConflictCopy(id, name); } catch (error) { ctx.report(error); }
      ctx.rerender();
    } }, "Verwijderen")));
  const result = h("article", { class: "meeting-detail" },
    h("header", { class: "meeting-header" }, h("div", { class: "grow" }, h("h1", {}, detail.info.title),
      h("p", { class: "help" }, [dateLabel(detail.info.start_iso), detail.info.duration_min ? `${detail.info.duration_min} min` : "", detail.attendees.join(", ")].filter(Boolean).join(" · ")))),
    h("div", { class: "meeting-tabs" }, tabs), ...banners, summarySetup, message, panel, footer);
  draw();
  if (ctx.jumpMs !== undefined) {
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
  if (!initial.busy && ctx.param) return meetingDetail(ctx, ctx.param);
  if (!initial.busy) {
    const title = h("input", { type: "text", placeholder: "Titel, bv. Weekly sync", class: "grow", "aria-label": "Titel van de meeting" });
    const status = h("p", { class: "error", role: "status" }, initial.error ?? "");
    const start = h("button", { class: "btn primary" }, "Start opname");
    start.addEventListener("click", async () => {
      start.disabled = true;
      try { await api.startMeeting(title.value); ctx.rerender(); }
      catch (error) { status.textContent = errorText(error); start.disabled = false; }
    });
    return h("div", { class: "recording-view" }, h("h1", {}, "Nieuwe meeting"),
      h("p", { class: "muted" }, "Pennel neemt je microfoon op als ‘Ik’ en wat je computer afspeelt als ‘Anderen’."),
      h("div", { class: "row" }, title, start), status,
      ctx.info.capabilities.system_audio.reason ? h("p", { class: "help" }, ctx.info.capabilities.system_audio.reason) : null,
      h("button", { class: "text-button", onclick: () => api.showWindow("settings", "recording") }, "Opnemen instellen…"));
  }
  if (!initial.id) {
    schedule(ctx, 500, async () => { const status = await api.meetingStatus(); if (status.id || !status.busy) ctx.rerender(); });
    return h("div", { class: "recording-view" }, h("h1", {}, "Opname starten…"), h("progress", { "aria-label": "Opname starten" }));
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
  const consent = h("p", { class: "help", role: "status", hidden: true }, "Toestemmingszin gekopieerd — plak hem in de chat van je call.");
  const output = h("div", { class: "live-transcript" });
  const phase = h("span", { class: "help", role: "status" });
  const stop = h("button", { class: "btn", disabled: initial.phase !== "recording" }, "Stop opname");
  const update = (status: MeetingStatus) => {
    if (document.activeElement !== title && !liveTitleDraft.dirty) {
      liveTitleDraft.sync(status.session_id, status.title ?? "Meeting");
      title.value = liveTitleDraft.value;
    }
    title.disabled = status.phase !== "recording";
    clock.textContent = `Opname loopt · ${formatClock(status.elapsed_secs)}`;
    phase.textContent = status.phase === "starting" ? "Opname starten…" : status.phase === "finalizing" || status.phase === "stopping"
      ? `Opname afronden…${status.pending ? ` ${status.pending} ${status.pending === 1 ? "stuk" : "stukken"} te transcriberen` : ""}` : "";
    warning.textContent = status.audio_warning ?? (!status.system_audio ? "Systeemgeluid niet beschikbaar — alleen je microfoon wordt opgenomen."
      : !status.heard_system_audio && status.elapsed_secs > 20 ? "Nog geen systeemgeluid gehoord." : "");
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
    h("div", { class: "row" }, h("span", { class: "status-dot" }), clock, h("span", { class: "grow" }), stop), consent, phase, warning, notes.input,
    h("details", { class: "live-transcript-disclosure" }, h("summary", {}, "Live transcript"), output));
}
