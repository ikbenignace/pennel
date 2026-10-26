import { bindLocalization, l, t, formatNumber, localizeBackendText, type UiText } from "../i18n.ts";
import { meetingDate as dateLabel, speakerLabel, speakerUiLabel, sourceSpeakerLabel, templateUiLabel } from "../meeting-labels.ts";
import { api, fixtureClient, type MeetingAnswer, type MeetingDetail, type MeetingFinished, type MeetingStatus, type MeetingVoice, type Passage, type Segment, type SpeakerRecognition } from "../api";
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
const detailState = new Map<string, { tab: "notes" | "transcript" | "ask"; template: string; working: boolean; message: string; recognition?: SpeakerRecognition }>();
interface Exchange { question: string; answer?: MeetingAnswer; error?: string }
const conversations = new Map<string, { exchanges: Exchange[]; draft: string; scrollTop: number | null }>();
let answerReturn: { scope: string | null; sourceMeetingId: string } | null = null;
let panelNumber = 0;

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
  const input = h("textarea", { rows, placeholder: l("meetings.notes_placeholder"), "aria-label": l("meetings.your_notes") });
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

/** Names a voice in this meeting; remembered voices receive the same name in future meetings. */
async function nameVoice(id: string, voice: number, suggested: string, current: MeetingVoice | undefined, remembering = false): Promise<boolean> {
  if (!current?.can_remember) {
    const name = await askText(l("meetings.voice_name"), suggested, l("meetings.voice_name_help"));
    if (name === null) return false;
    await api.renameSpeaker(id, voice, name);
    return true;
  }
  const answer = await askTextWithSwitch(l("meetings.voice_name"), suggested,
    l("meetings.voice_remember_help"), l("meetings.remember_voice"), remembering || current.remembered);
  if (!answer) return false;
  if (answer.on && answer.text) await api.rememberSpeaker(id, voice, answer.text);
  else await api.renameSpeaker(id, voice, answer.text);
  return true;
}

function transcript(ctx: Context, segments: Segment[], detail?: MeetingDetail, id?: string, voices: MeetingVoice[] = []) {
  const result = h("div", { class: "transcript" });
  if (!segments.length) { result.append(h("p", { class: "muted" }, l("meetings.empty_transcript"))); return result; }
  const play = ctx.info.capabilities.audio_playback;
  for (const [index, segment] of segments.entries()) {
    const available = !!detail?.audio_tracks.some((track) => track.speaker === segment.speaker && track.available);
    const stamp = h("button", { class: "transcript-stamp text-button", disabled: !id || !available || play.status !== "available",
      title: available ? play.reason ? localizeBackendText(play.reason) : l("meetings.play_from_here") : l("meetings.no_saved_audio") }, formatClock(segment.start_ms / 1000));
    if (play.reason) bindLocalization(stamp, () => { stamp.title = available ? localizeBackendText(play.reason!) : t("meetings.no_saved_audio"); });
    if (id) stamp.addEventListener("click", () => { void api.playMeetingAudio(id, segment.speaker, segment.start_ms).catch(ctx.report); });
    const speakerColor = segment.speaker === "me" ? "var(--accent)" : segment.voice === null ? "var(--foreground)" : `var(--voice-${(segment.voice + 3) % 4 + 1})`;
    let speaker: HTMLElement = h("span", { class: "speaker", style: `color:${speakerColor}` }, speakerUiLabel(segment, detail));
    if (id && segment.voice !== null && segment.speaker === "others") {
      const currentName = detail?.speakers.find((item) => item.voice === segment.voice)?.name ?? "";
      const names = detail?.attendees ?? [];
      const chosen = names.indexOf(currentName);
      const original = chosen >= 0 ? `name:${chosen}` : "current";
      const voice = voices.find((item) => item.voice === segment.voice);
      const offerRemember = !!voice?.can_remember && !voice.remembered && !!currentName;
      const picker = select([
        ...chosen < 0 ? [["current", speakerUiLabel(segment, detail)] as [string, UiText]] : [],
        ...names.map((name, index) => [`name:${index}`, name] as [string, string]), ["custom", l("meetings.other_name")],
        ...offerRemember ? [["remember", l("meetings.remember_voice_action")] as [string, UiText]] : [],
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
      const relabelPicker = () => picker.setAttribute("aria-label", t("meetings.voice_name_label", { name: speakerLabel(segment, detail) }));
      relabelPicker();
      bindLocalization(picker, relabelPicker);
      speaker = picker;
    }
    const body = h("div", { class: "transcript-copy" }, segment.failed ? h("span", { class: "muted" }, l("meetings.transcription_failed")) : segment.text);
    if (segment.failed && id) {
      const retry = h("button", { class: "text-button", onclick: async () => {
        retry.disabled = true;
        try { await api.retranscribeMeeting(id); ctx.rerender(); }
        catch (error) { ctx.report(error); retry.disabled = false; }
      } }, l("common.retry"));
      body.append(retry);
    }
    result.append(h("div", { class: "transcript-line", "data-start-ms": segment.start_ms, "data-transcript-index": index }, stamp, speaker, body));
  }
  return result;
}

function citationLabel(passage: Passage) {
  const place = passage.kind === "transcript" ? passage.start_ms === null ? t("meetings.source_transcript") : formatClock(passage.start_ms / 1000)
    : passage.kind === "summary" ? t("meetings.source_summary") : t("meetings.source_notes");
  return `${passage.title} · ${dateLabel(passage.start_iso)} · ${place}`;
}

function recoveryNotice(ctx: Context, status: MeetingStatus, latest = () => status, includeTranscription = true): HTMLElement | null {
  const primary = status.persistence_error ?? status.terminal_error ?? status.error;
  let actionError: string | null = null;
  const warnings = () => actionError ? [localizeBackendText(actionError)] : [...new Set([primary, includeTranscription ? status.transcription_warning : null].filter((message): message is string => !!message).map(localizeBackendText))];
  if (!warnings().length && !status.can_retry_persistence) return null;
  const message = h("p", { class: "error", role: status.terminal_error ? "alert" : "status" }, warnings().join(" "));
  bindLocalization(message, () => { message.textContent = warnings().join(" "); });
  const actions = h("div", { class: "row" });
  if (status.can_retry_persistence && status.session_id) {
    const retry = h("button", { class: "btn", onclick: async () => {
      retry.disabled = true;
      try { await flushMeetingDrafts(); await api.retryMeetingPersistence(status.session_id!); ctx.rerender(); }
      catch (error) { actionError = errorText(error); message.textContent = localizeBackendText(actionError); retry.disabled = false; }
    } }, l("meetings.retry_save"));
    actions.append(retry);
  }
  if (status.transcript.some((segment) => !segment.failed && segment.text.trim())) {
    const copied = h("span", { class: "help", role: "status" });
    actions.append(h("button", { class: "btn", onclick: async () => {
      try {
        await api.copyText(latest().transcript.filter((segment) => !segment.failed).map((segment) => `${formatClock(segment.start_ms / 1000)} ${speakerLabel(segment)}: ${segment.text}`).join("\n"));
        copied.replaceChildren(h("span", {}, l("meetings.copied_transcript")));
      } catch (error) { actionError = errorText(error); message.textContent = localizeBackendText(actionError); }
    } }, l("meetings.copy_transcript")), copied);
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
  bindLocalization(element, refresh);
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
    h("h1", {}, meetingId ? l("meetings.ask_this") : l("meetings.ask_all")),
    h("p", { class: "muted" }, meetingId ? l("meetings.ask_example_this") : l("meetings.ask_example_all"))));
  for (const exchange of state.exchanges) {
    let response: HTMLElement;
    if (exchange.answer) response = markdown(exchange.answer.text);
    else if (exchange.error) {
      response = h("p", { class: "error" }, localizeBackendText(exchange.error));
      bindLocalization(response, () => { response.textContent = localizeBackendText(exchange.error ?? ""); });
    } else response = h("progress", { "aria-label": l("meetings.writing_answer") });
    const citations = exchange.answer ? h("div", { class: "citations" }, exchange.answer.citations.map((citation) => {
      const label = () => `[${citation.number}] ${citationLabel(citation.passage)}`;
      const button = h("button", { class: "text-button citation", onclick: () => {
        state.scrollTop = history.scrollTop;
        answerReturn = { scope: meetingId, sourceMeetingId: citation.passage.meeting_id };
        ctx.navigate("meetings", citation.passage.meeting_id, citation.passage.kind === "transcript" ? citation.passage.start_ms ?? undefined : undefined,
          { kind: citation.passage.kind, start_ms: citation.passage.start_ms, text: citation.passage.text });
      } }, label());
      bindLocalization(button, () => { button.textContent = label(); });
      return button;
    })) : null;
    history.append(h("div", { class: "chat-exchange" }, h("p", { class: "chat-question" }, exchange.question), response, citations));
  }
  const input = h("input", { type: "text", class: "grow", value: state.draft, disabled: busy,
    placeholder: meetingId ? l("meetings.ask_placeholder_this") : l("meetings.ask_placeholder_all"), "aria-label": l("meetings.your_question") });
  const submit = h("button", { class: "btn", disabled: busy || !state.draft.trim() }, busy ? l("meetings.working") : l("meetings.ask"));
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
    catch (error) { return h("div", { class: "empty-state" }, h("p", { class: "error" }, errorText(error)), h("button", { class: "btn", onclick: () => ctx.navigate("meetings") }, l("meetings.back_meetings"))); }
  }
  const meetings = await api.meetings("");
  if (meetings.length) return h("div", { class: "empty-state library-placeholder" }, h("p", { class: "muted" }, l("meetings.choose_or_record")));
  return h("div", { class: "empty-state meeting-library-empty" }, icon("notebook"), h("h2", {}, l("meetings.empty_meetings")),
    h("p", { class: "muted" }, l("meetings.empty_meetings_help")),
    h("button", { class: "btn primary", onclick: () => ctx.navigate("live") }, l("meetings.first_recording")));
}

async function meetingDetail(ctx: Context, id: string): Promise<HTMLElement> {
  const detail = await api.meetingDetail(id);
  if (!detail.info) return h("div", { class: "empty-state muted" }, l("meetings.not_found"));
  const meetingInfo = detail.info;
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
  const audioReason = () => !audioAvailable ? t("meetings.no_saved_audio") : playback.status !== "available" ? playback.reason ? localizeBackendText(playback.reason) : t("meetings.audio_unavailable") : null;
  const [voices, speakerSetup] = await Promise.all([orFallback(() => api.meetingVoices(id), [], ctx.report), orFallback(() => api.speakerSetup(), null, ctx.report)]);
  const remembered = voices.filter((voice) => voice.remembered && voice.name).map((voice) => voice.name);
  if (ctx.sourceTarget) current.tab = ctx.sourceTarget.kind === "transcript" ? "transcript" : "notes";
  else if (ctx.jumpMs !== undefined) current.tab = "transcript";
  const templates = await api.meetingTemplates();
  if (!templates.some((template) => template.id === current.template)) current.template = templates[0]?.id ?? "algemeen";
  const notesDraft = draftFor(id, detail.notes);
  const notes = editNotes(ctx, notesDraft, id);
  notes.input.disabled = active;
  if (active) {
    notes.input.title = t("meetings.edit_recording_notes");
    bindLocalization(notes.input, () => { notes.input.title = t("meetings.edit_recording_notes"); });
  }
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
  const tabs = h("div", { class: "segmented detail-tabs", role: "tablist", "aria-label": l("meetings.default_title") });
  const panel = h("section", { id: panelId, class: "meeting-panel", role: "tabpanel", tabindex: 0 });
  const draw = () => {
    footer.hidden = current.tab === "ask";
    diarize.hidden = current.tab !== "transcript";
    diarizeReason.hidden = current.tab !== "transcript" || !speakerReason();
    tabs.querySelectorAll("button").forEach((button) => {
      button.setAttribute("aria-selected", String(button.dataset.tab === current.tab));
      button.tabIndex = button.dataset.tab === current.tab ? 0 : -1;
    });
    panel.setAttribute("aria-labelledby", `${panelId}-${current.tab}`);
    if (current.tab === "ask") { panel.replaceChildren(chat(ctx, id)); return; }
    if (current.tab === "transcript") {
      const playbackReason = h("span", { class: audioReason() ? "help" : "grow" }, audioReason());
      bindLocalization(playbackReason, () => { playbackReason.textContent = audioReason(); });
      const stopAudio = h("button", { class: "text-button", disabled: !!audioReason(), title: audioReason(), onclick: () => api.stopAudio() }, l("meetings.stop_audio"));
      bindLocalization(stopAudio, () => { stopAudio.title = audioReason() ?? ""; });
      panel.replaceChildren(h("div", { class: "transcript-tools" }, playbackReason, stopAudio),
        ...remembered.length ? [h("p", { class: "help remembered-voices" }, l("meetings.remembered_voices", { names: remembered.join(", ") }))] : [],
        transcript(ctx, detail.transcript, detail, id, voices));
      return;
    }
    panel.replaceChildren(detail.summary ? enhanced : detail.notes.trim() ? h("div", { class: "markdown" }, detail.notes) : h("p", { class: "muted" }, l("meetings.empty_summary")));
    panel.append(h("details", { class: "own-notes", open: !detail.summary }, h("summary", {}, l("meetings.your_notes")), notes.input));
  };
  for (const [tab, label] of [["notes", l("meetings.notes")], ["transcript", l("meetings.transcript")], ["ask", l("meetings.questions")]] as const) {
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
  let sourceChanged = false;
  let recognizing = false;
  const messageText = () => sourceChanged ? t("meetings.source_changed") : current.working ? t(recognizing ? "meetings.recognizing_speakers" : "meetings.working") : current.recognition ? recognitionSummary(current.recognition) : localizeBackendText(current.message);
  const message = h("p", { class: "meeting-message help", role: "status" }, messageText());
  bindLocalization(message, () => { message.textContent = messageText(); });
  const regenerate = h("button", { class: "btn", disabled: current.working || runtime.busy || !canSummarize,
    title: !canSummarize ? l("meetings.choose_text_model") : runtime.busy ? l("backend.wait_until_the_recording_has_finished") : null }, current.working ? l("dictation.working") : detail.summary ? l("meetings.regenerate") : l("meetings.summarize"));
  regenerate.addEventListener("click", async () => {
    recognizing = false; current.recognition = undefined; current.working = true; regenerate.disabled = true; message.textContent = t("dictation.working");
    try { await notes.flush(); await api.summarizeMeeting(id, current.template); current.message = ""; }
    catch (error) { current.message = errorText(error); }
    current.working = false; ctx.rerender();
  });
  const selected = select(templates.map((template) => [template.id, templateUiLabel(template)]), current.template, (template) => { current.template = template; }, "menu");
  selected.setAttribute("aria-label", t("meetings.template"));
  bindLocalization(selected, () => selected.setAttribute("aria-label", t("meetings.template")));
  selected.disabled = runtime.busy;
  const copied = h("span", { class: "help", role: "status" });
  const copy = async (formatted: boolean, withTranscript = false) => {
    const exported = await api.exportMeeting(id, withTranscript);
    await api.copyText(exported.markdown, formatted ? exported.html : null);
    copied.replaceChildren(h("span", {}, withTranscript ? l("meetings.copied_with_transcript") : l("meetings.copied")));
  };
  const rich = ctx.info.capabilities.rich_clipboard;
  const shareAction = (work: () => Promise<unknown>) => () => work().catch(ctx.report);
  const share = menuButton(l("meetings.share"), [
    { label: l("meetings.copy_formatted"), disabled: rich.status !== "available", title: rich.reason ? localizeBackendText(rich.reason) : null, run: shareAction(() => copy(true)) },
    { label: l("meetings.copy_markdown"), run: shareAction(() => copy(false)) },
    { label: l("meetings.copy_with_transcript"), run: shareAction(() => copy(rich.status === "available", true)) },
    { label: l("meetings.attendees_action"), run: shareAction(async () => {
      const value = await askText(l("meetings.attendees"), detail.attendees.join(", "), l("meetings.attendees_help"));
      if (value === null) return;
      await api.setMeetingAttendees(id, value.split(",").map((name) => name.trim()).filter(Boolean)); ctx.rerender();
    }) },
    { label: l("meetings.email"), run: shareAction(() => api.shareMeeting(id, false, "mail")) },
    { label: l("meetings.share_action"), disabled: ctx.info.platform !== "macos",
      title: ctx.info.platform === "macos" ? l("meetings.open_share_menu") : l("meetings.share_unavailable"),
      run: shareAction(() => api.shareMeeting(id, false, "share")) },
  ], "share-menu");
  const diarization = ctx.info.capabilities.diarization;
  const cloudRoute = ctx.info.settings.speaker_recognition === "cloud";
  const speakerReason = () => runtime.busy || meetingInfo.duration_min <= 0 ? t("meetings.finish_first")
    : diarization.status !== "available" ? diarization.reason ? localizeBackendText(diarization.reason) : t("meetings.speaker_recognition_unavailable")
    : !detail.audio_tracks.some((track) => track.available) ? t("meetings.no_saved_audio")
    : !detail.transcript.some((segment) => !segment.failed) ? t("meetings.empty_transcript")
    : !speakerSetup?.ready ? cloudRoute ? t("backend.sign_in_to_pennel_cloud_under_settings_account_first") : t("meetings.download_speaker_models")
    : null;
  const canDiarize = !speakerReason();
  const diarize = h("button", { class: "btn", disabled: !canDiarize || current.working,
    title: speakerReason() ?? (cloudRoute ? l("meetings.speaker_cloud_help") : l("meetings.speaker_local_help")) }, l("meetings.recognize_speakers"));
  diarize.addEventListener("click", async () => {
    recognizing = true; current.recognition = undefined; current.working = true; diarize.disabled = true; message.textContent = t("meetings.recognizing_speakers");
    try { current.recognition = await api.recognizeSpeakers(id); current.message = recognitionSummary(current.recognition); }
    catch (error) { current.message = errorText(error); }
    current.working = false; ctx.rerender();
  });
  const diarizeReason = h("p", { class: "help meeting-control-reason", hidden: true }, speakerReason());
  bindLocalization(diarizeReason, () => { diarizeReason.textContent = speakerReason(); });
  bindLocalization(diarize, () => { diarize.title = speakerReason() ?? t(cloudRoute ? "meetings.speaker_cloud_help" : "meetings.speaker_local_help"); });
  const footer = h("footer", { class: "meeting-footer" }, h("div", { class: "row" }, selected, regenerate, h("span", { class: "grow" }), copied, diarize, share,
    h("button", { class: "btn", onclick: () => api.openPath(detail.dir) }, ctx.info.platform === "macos" ? l("meetings.show_finder") : l("meetings.open_folder"))), diarizeReason);
  const summarySetup = !detail.summary && !canSummarize ? h("div", { class: "meeting-summary-setup help" }, l("meetings.choose_text_model"), h("button", { class: "text-button", onclick: () => ctx.navigate("models") }, l("meetings.set_up_models"))) : null;
  const conflicts = ctx.info.settings.cloud_sync === true ? await orFallback(() => api.meetingConflictFiles(id), [], ctx.report) : [];
  const banners = conflicts.map((name) => h("div", { class: "conflict-banner", role: "status" }, icon("warning"),
    h("span", {}, l("meetings.conflict_notice")),
    h("button", { class: "btn", onclick: () => { void api.openPath(detail.dir).catch(ctx.report); } }, l("meetings.view")),
    h("button", { class: "btn danger", onclick: async () => {
      if (!await confirmDialog(l("meetings.delete_conflict_title"), l("meetings.file_will_delete", { name }), l("common.delete"))) return;
      try { await api.deleteConflictCopy(id, name); } catch (error) { ctx.report(error); }
      ctx.rerender();
    } }, l("common.delete"))));
  const metadataText = () => [dateLabel(meetingInfo.start_iso), meetingInfo.duration_min ? t("meetings.duration_minutes", { count: formatNumber(meetingInfo.duration_min) }) : "", detail.attendees.join(", ")].filter(Boolean).join(" · ");
  const metadata = h("p", { class: "help" }, metadataText());
  bindLocalization(metadata, () => { metadata.textContent = metadataText(); });
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
    } }, l("meetings.back_answer")) : null,
    h("header", { class: "meeting-header" }, h("div", { class: "grow" }, h("h1", {}, meetingInfo.title),
      metadata), tabs),
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
        const candidates = detail.transcript.map((segment, index) => ({ ...segment, index, text: `${sourceSpeakerLabel(segment, detail)}: ${segment.text.trim()}` })).filter((segment) => !segment.failed);
        const range = transcriptSourceBlocks(candidates, source);
        if (range) highlight(candidates.slice(range.first, range.last + 1).flatMap((segment) => {
          const element = result.querySelector<HTMLElement>(`[data-transcript-index="${segment.index}"]`);
          return element ? [element] : [];
        }));
      }
      if (!matched) { sourceChanged = true; message.textContent = messageText(); }
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
    return h("div", { class: "recording-view retained-recording" }, h("h1", {}, initial.title ?? l("meetings.save_recording")),
      h("p", { class: "help" }, l("meetings.recording_stopped", { time: formatClock(initial.elapsed_secs) })),
      h("p", { class: "muted" }, l("meetings.save_before_new")),
      meetingProcessingNotice(ctx, () => initial, true).element, recoveryNotice(ctx, initial));
  }
  if (!initial.busy && ctx.param) return meetingDetail(ctx, ctx.param);
  if (!initial.busy) {
    const title = h("input", { type: "text", placeholder: l("meetings.meeting_title_placeholder"), class: "grow", "aria-label": l("meetings.meeting_title_label") });
    const status = h("p", { class: "error", role: "status" }, initial.error ? localizeBackendText(initial.error) : "");
    const start = h("button", { class: "btn primary" }, l("common.start_recording"));
    const retained = !!initial.recognition && !!(initial.persistence_error || initial.terminal_error);
    const processing = meetingProcessingNotice(ctx, () => retained ? initial : undefined, retained);
    const audioReason = h("p", { class: "help" });
    const refreshAudioReason = () => {
      audioReason.textContent = ctx.info.capabilities.system_audio.reason ? localizeBackendText(ctx.info.capabilities.system_audio.reason) : "";
      audioReason.hidden = !audioReason.textContent;
    };
    const microphoneAccess = h("button", { class: "btn", onclick: async () => {
      try { await api.requestPermission("microphone"); await ctx.refresh(); } catch (error) { ctx.report(error); }
    } }, l("onboarding.open_settings"));
    const refreshMicrophoneAccess = () => {
      microphoneAccess.hidden = ctx.info.capabilities.microphone.status !== "permission_required";
    };
    ctx.onInfoChange(refreshAudioReason);
    bindLocalization(audioReason, refreshAudioReason);
    ctx.onInfoChange(refreshMicrophoneAccess);
    refreshAudioReason();
    refreshMicrophoneAccess();
    start.addEventListener("click", async () => {
      start.disabled = true;
      try { await api.startMeeting(title.value); ctx.rerender(); }
      catch (error) { status.textContent = errorText(error); start.disabled = false; await ctx.refresh().catch(ctx.report); }
    });
    return h("div", { class: "recording-view" }, h("h1", {}, l("meetings.new_meeting")),
      h("p", { class: "muted" }, l("meetings.recording_tracks_help")),
      processing.element, h("div", { class: "row" }, title, start), status, microphoneAccess, recoveryNotice(ctx, initial), audioReason,
      h("button", { class: "text-button", onclick: () => api.showWindow("settings", "recording") }, l("meetings.recording_settings")));
  }
  if (!initial.id) {
    schedule(ctx, 500, async () => { const status = await api.meetingStatus(); if (status.id || !status.busy) ctx.rerender(); });
    return h("div", { class: "recording-view" }, h("h1", {}, l("meetings.starting")), meetingProcessingNotice(ctx, () => initial, true).element, h("progress", { "aria-label": l("meetings.starting_label") }));
  }
  if (liveDraft.dirty && liveMeetingId && liveDraft.sessionId !== initial.session_id) {
    const retained = new MeetingDraft();
    retained.sync(liveDraft.sessionId, liveDraft.value);
    retained.edit(liveDraft.value);
    drafts.set(liveMeetingId, retained);
  }
  liveDraft.sync(initial.session_id ?? initial.id, initial.notes);
  liveTitleDraft.sync(initial.session_id, initial.title ?? t("meetings.default_title"));
  liveMeetingId = initial.id;
  if (initial.session_id) sessionMeetingIds.set(initial.session_id, initial.id);
  const notes = editNotes(ctx, liveDraft, initial.id!);
  notes.input.classList.add("live-notes");
  notes.input.placeholder = t("meetings.live_notes_placeholder");
  bindLocalization(notes.input, () => { notes.input.placeholder = t("meetings.live_notes_placeholder"); });
  const title = h("input", { type: "text", class: "live-title", value: liveTitleDraft.value, "aria-label": l("meetings.meeting_title_label") });
  title.addEventListener("input", () => { liveTitleDraft.edit(title.value); void flushLiveTitle().catch(ctx.report); });
  title.addEventListener("blur", () => { void flushLiveTitle().catch(ctx.report); });
  ctx.onDispose(() => { void flushLiveTitle().catch(ctx.report); });
  const clock = h("span", { class: "live-clock" });
  const warning = h("p", { class: "help", role: "status" });
  const recovery = h("div", { class: "meeting-recovery" });
  let latestStatus = initial;
  const processing = meetingProcessingNotice(ctx, () => latestStatus, true);
  let recoveryKey = "";
  const consent = h("p", { class: "help", role: "status", hidden: true }, l("meetings.consent_copied"));
  const output = h("div", { class: "live-transcript" });
  const phase = h("span", { class: "help", role: "status" });
  const stop = h("button", { class: "btn", disabled: initial.phase !== "recording" }, l("common.stop_recording"));
  const update = (status: MeetingStatus) => {
    latestStatus = status;
    processing.refresh();
    if (document.activeElement !== title && !liveTitleDraft.dirty) {
      liveTitleDraft.sync(status.session_id, status.title ?? t("meetings.default_title"));
      title.value = liveTitleDraft.value;
    }
    title.disabled = status.phase !== "recording";
    clock.textContent = t("meetings.recording_clock", { time: formatClock(status.elapsed_secs) });
    phase.textContent = status.phase === "starting" ? t("meetings.starting") : status.phase === "finalizing" || status.phase === "stopping"
      ? status.pending ? t(status.pending === 1 ? "meetings.finishing_pending_one" : "meetings.finishing_pending_many", { count: formatNumber(status.pending) }) : t("meetings.finishing_recording") : "";
    const audioWarning = status.audio_warning ?? (!status.system_audio ? t("meetings.no_system_audio")
      : !status.heard_system_audio && status.elapsed_secs > 20 ? t("meetings.no_system_audio_heard") : "");
    const warningText = [status.transcription_warning, audioWarning].filter((message): message is string => !!message).map(localizeBackendText).join(" ");
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
  bindLocalization(clock, () => { update(latestStatus); });
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
    h("details", { class: "live-transcript-disclosure" }, h("summary", {}, l("meetings.live_transcript")), output));
}
