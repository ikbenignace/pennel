/* Loaded only by capture-desktop-reference.mjs before the development entry point. */
(() => {
  const options = globalThis.__PENNEL_FIXTURE_OPTIONS__;
  if (!options || !location.hostname.match(/^(127\.0\.0\.1|localhost)$/)) throw new Error("Visual fixtures require an owned local server");
  const clone = value => structuredClone(value);
  const now = Date.parse("2026-10-25T09:00:00Z");
  const ActualDate = Date;
  function FixtureDate(...args) {
    if (!new.target) return new ActualDate(now).toString();
    return Reflect.construct(ActualDate, args.length ? args : [now], new.target);
  }
  Object.setPrototypeOf(FixtureDate, ActualDate);
  FixtureDate.prototype = ActualDate.prototype;
  FixtureDate.now = () => now;
  globalThis.Date = FixtureDate;

  const meetingId = "2026-10-24 0900 Projectoverleg";
  const sentence = "Zullen we morgen om tien uur afspreken?";
  const pillSnapshot = options.window === "pill" ? {
    state: options.pill_state,
    preview: { session_id: "qa-" + options.name, revision: options.preview_text ? 7 : 0, text: options.preview_text || "" },
  } : { state: "hidden" };
  const notes = "- Planning afronden\n- Volgende afspraak bevestigen";
  const summary = "## Besluiten\n- De planning wordt maandag afgerond.\n\n## Actiepunten\n- [ ] Volgende afspraak bevestigen.";
  const meeting = { id: meetingId, title: "Projectoverleg", start_iso: "2026-10-24T09:00:00+02:00", duration_min: 25, has_summary: true };
  const library = [
    { id: "2026-10-25 0830 Weekstart", title: "Weekstart", start_iso: "2026-10-25T08:30:00+01:00", duration_min: 18, has_summary: true },
    meeting,
    { id: "2026-10-14 1400 Klantgesprek Studio Noord", title: "Klantgesprek Studio Noord", start_iso: "2026-10-14T14:00:00+02:00", duration_min: 42, has_summary: true },
  ];
  const meetingInfo = id => library.find(item => item.id === id) || meeting;
  const transcript = [
    { start_ms: 0, end_ms: 8000, speaker: "others", text: "We ronden de planning maandag af.", failed: false, voice: 1 },
    { start_ms: 8000, end_ms: 14000, speaker: "me", text: "Ik bevestig de volgende afspraak.", failed: false, voice: null },
  ];
  const available = () => ({ status: "available", reason: null });
  const providers = [
    { id: "local", kind: "local", name: "Lokaal", base_url: "" },
    { id: "groq", kind: "groq", name: "Groq", base_url: "https://api.groq.com/openai/v1" },
    { id: "apple-speech", kind: "apple_speech", name: "Apple-dicteren", base_url: "" },
    { id: "openai", kind: "open_ai", name: "OpenAI", base_url: "https://api.openai.com/v1" },
  ];
  const providerDefinitions = [
    ["local", "Lokaal op deze computer", false, "", true, false, "whisper-large-v3-turbo", ""],
    ["groq", "Groq", true, "https://api.groq.com/openai/v1", true, true, "whisper-large-v3-turbo", "llama-3.3-70b-versatile"],
    ["open_ai", "OpenAI", true, "https://api.openai.com/v1", true, true, "gpt-4o-mini-transcribe", "gpt-4.1-mini"],
    ["open_router", "OpenRouter", true, "https://openrouter.ai/api/v1", true, true, "openai/whisper-large-v3-turbo", "google/gemini-3.5-flash-lite"],
    ["mistral", "Mistral", true, "https://api.mistral.ai/v1", true, true, "voxtral-mini-latest", "mistral-small-latest"],
    ["apple_speech", "Apple-dicteren", false, "", true, false, "dictation", ""],
    ["apple_intelligence", "Apple Intelligence", false, "", false, true, "", "system"],
    ["ollama", "Ollama (lokaal)", false, "http://localhost:11434", false, true, "", "gemma3:4b"],
    ["lm_studio", "LM Studio (lokaal)", false, "http://localhost:1234/v1", false, true, "", ""],
    ["custom", "Eigen server", true, "", true, true, "", ""],
    ["pennel_cloud", "Pennel Cloud", false, "https://api.pennel.app/v1", true, true, "pennel-auto", "pennel-auto"],
  ];
  const kinds = providerDefinitions.map(([kind, label, requires_key, default_base_url, stt, llm, default_stt_model, default_llm_model]) => ({
    kind, label, requires_key, default_base_url, stt, llm, default_stt_model, default_llm_model,
    is_cli_agent: false, availability: available(),
  }));
  const selectedProvider = options.engine === "apple" ? "apple-speech" : options.engine === "custom" ? "openai" : "local";
  const model = selectedProvider === "apple-speech" ? "dictation" : selectedProvider === "openai" ? "gpt-4o-mini-transcribe" : "whisper-large-v3-turbo";
  let settings = {
    providers, dictation_stt: { provider_id: selectedProvider, model }, meeting_stt: { provider_id: "local", model: "whisper-large-v3-turbo" },
    llm: { provider_id: "groq", model: "llama-3.3-70b-versatile" }, mode: options.mode || "fast", language: "nl", dictionary: ["Pennel", "Studio Noord"],
    style_rules: [{ bundle_id: "com.tinyspeck.slackmacgap", style: "casual" }, { bundle_id: "com.apple.MobileSMS", style: "casual" }, { bundle_id: "com.apple.mail", style: "formal" }, { bundle_id: "com.microsoft.Word", style: "formal" }, { bundle_id: "com.apple.Terminal", style: "literal" }, { bundle_id: "com.microsoft.VSCode", style: "literal" }], history_retention_days: 30, hotkey: "fn", local_unload_minutes: 5,
    meetings_dir: "/QA/Pennel Preview/Meetings", default_template: "algemeen", meeting_audio_retention_days: 30,
    app_rules: [], calendar_rules: [], event_rules: [], calendar_enabled: false, echo_cancellation: true,
    record_shortcut_enabled: true, consent_enabled: false, auto_detect: true, diarization: false,
    appearance: options.appearance, dictation_pad_auto_paste: false, cloud_sync: Boolean(options.cloud), claude_access: Boolean(options.claude),
    onboarding: { version: 1, status: options.window === "onboarding" ? "pending" : options.shortcut_denied ? "skipped" : "completed", resume_step: options.step || "welcome" },
  };
  const capabilities = Object.fromEntries(["pad_external_paste", "global_hotkey", "record_shortcut", "microphone", "system_audio", "echo_cancellation", "call_detection", "calendar", "apple_speech", "apple_intelligence", "selection", "rich_clipboard", "audio_playback", "diarization"].map(name => [name, available()]));
  capabilities.local_acceleration = "metal";
  const permissionStatus = options.permission || "granted";
  let permissions = ["microphone", "accessibility", "calendar", "speech", "system_audio"].map(kind => ({ kind, status: kind === "calendar" ? "not_determined" : permissionStatus }));
  if (options.shortcut_denied) {
    permissions = permissions.map(permission => permission.kind === "accessibility" ? { ...permission, status: "denied" } : permission);
  }
  if (permissions.find(permission => permission.kind === "accessibility")?.status !== "granted") {
    const reason = "Geef Pennel toegang tot Toegankelijkheid om fn in andere apps te gebruiken.";
    capabilities.global_hotkey = { status: "permission_required", reason };
    capabilities.selection = { status: "permission_required", reason };
    capabilities.pad_external_paste = { status: "permission_required", reason };
  }
  const keys = { groq: Boolean(options.configured_text_provider), openai: false };
  const providerRevisions = Object.fromEntries(providers.map(provider => [provider.id, 0]));
  const models = (options.local_models || [{
    id: "whisper-large-v3-turbo", name: "Whisper Large v3 Turbo", description: "Spraakherkenning in meerdere talen.",
    size_bytes: 886381760, parameters: "809M", speed: 72, accuracy: 86, languages: ["nl", "en", "fr", "de"], recommended: true, legacy: false,
  }]).map(model => ({ ...model, state: model.id === "whisper-large-v3-turbo" && options.download_active
    ? { kind: "partial", bytes: Math.floor(model.size_bytes / 3) }
    : model.id === "whisper-large-v3-turbo" && !(options.window === "onboarding" && ["welcome", "engine"].includes(options.step)) ? { kind: "downloaded" } : { kind: "not_downloaded" } }));
  const diarizer = { id: "diar_streaming_sortformer_4spk-v2.1", name: "Sortformer (sprekers)", size_bytes: 139310336, state: { kind: "not_downloaded" } };
  const modelFor = id => models.find(model => model.id === id) || (id === diarizer.id ? diarizer : undefined);
  const activeDownloads = new Set(options.download_active ? ["whisper-large-v3-turbo"] : []);
  const downloadWaiters = new Map();
  let pad = sentence;
  let sessionCounter = 0;
  const sessions = new Map();
  const listeners = new Map();
  const calls = [];
  const unknown = [];
  const emit = (name, payload) => {
    for (const handler of listeners.get(name) || []) handler({ payload: clone(payload) });
  };
  const completeDownload = (id, error = null) => {
    if (!activeDownloads.delete(id)) return false;
    const model = modelFor(id);
    if (!error && model) {
      model.state = { kind: "downloaded" };
      emit("download-progress", { id, downloaded: model.size_bytes, total: model.size_bytes });
    }
    const waiting = downloadWaiters.get(id) || [];
    downloadWaiters.delete(id);
    emit("download-done", { id, error });
    for (const waiter of waiting) error ? waiter.reject(new Error(error)) : waiter.resolve(true);
    return true;
  };
  const shortcutDenied = () => permissions.find(permission => permission.kind === "accessibility")?.status !== "granted";
  const info = () => ({ settings: clone(settings), keys: clone(keys), provider_revisions: clone(providerRevisions), active_model_downloads: [...activeDownloads],
    unsaved_history_count: history.filter(entry => entry.status === "unsaved").length,
    kinds: clone(kinds), platform: "macos", version: "0.1.0", hotkey_error: shortcutDenied() ? capabilities.global_hotkey.reason : null,
    log_path: "/QA/Pennel Preview/pennel.log", data_dir: "/QA/Pennel Preview", capabilities: clone(capabilities), recording_app_ids: options.recording_app_ids || [], diarizer_model_id: diarizer.id, preview: true, migration_warning: null });
  const history = [
    ...(options.history_unsaved ? [{ id: "qa-dictation-unsaved", created_at_ms: now - 60_000, app_bundle_id: "com.apple.mail", raw: sentence, text: sentence, status: "unsaved", error: "De dicteergeschiedenis kon niet worden bewaard.", has_audio: false }] : []),
    { id: "qa-dictation-2", created_at_ms: now - 15 * 60_000, app_bundle_id: "dev.pennel.Pennel.preview", raw: sentence, text: sentence, status: "done", error: null, has_audio: false },
    { id: "qa-dictation-1", created_at_ms: now - 60 * 60_000, app_bundle_id: "com.apple.mail", raw: "ik werk het voorstel bij en stuur het morgen door", text: "Ik werk het voorstel bij en stuur het morgen door.", status: "done", error: null, has_audio: false },
    { id: "qa-dictation-0", created_at_ms: now - 26 * 60 * 60_000, app_bundle_id: "com.tinyspeck.slackmacgap", raw: "kan iemand de notities van gisteren delen", text: "Kan iemand de notities van gisteren delen?", status: "done", error: null, has_audio: false },
    { id: "qa-dictation-failed", created_at_ms: now - 27 * 60 * 60_000, app_bundle_id: "com.apple.Notes", raw: "", text: "", status: "failed", error: "Geen verbinding met de provider.", has_audio: true },
  ];
  // `cloud` is unset (signed out), "synced" or "conflict"; all accounts, devices and codes are synthetic.
  const signedIn = Boolean(options.cloud);
  const notSignedIn = () => ({ code: "not_signed_in", message: "Je bent niet ingelogd bij Pennel Cloud." });
  const cloudAccount = { email: "noor@example.com", audio_seconds: 5400, tokens: 120000, storage_bytes: 52_428_800, limit_audio_seconds: 36000, limit_tokens: 2_000_000, limit_storage_bytes: 5_368_709_120 };
  const cloudDevices = [
    { id: "qa-device-mac", name: "MacBook van Noor", platform: "macos", last_seen_iso: "2026-10-25T08:55:00Z", current: true },
    { id: "qa-device-windows", name: "Werklaptop", platform: "windows", last_seen_iso: "2026-10-24T16:10:00Z", current: false },
  ];
  const syncReport = () => ({ status: { kind: signedIn ? "synced" : "signed_out" }, brake: false, uploaded: 0, downloaded: 0, conflicts: options.cloud === "conflict" ? 1 : 0, last_success_ms: signedIn ? now - 4 * 60_000 : null });
  const fixture = {
    ...options, now: new ActualDate(now).toISOString(), meeting_id: meetingId,
    onboarding: { step: options.step, engine: options.engine || "local", download_error: options.download_error || null, practice_error: options.practice_error || null },
  };
  const sessionSnapshot = kind => ({ session_id: `qa-${kind}-${++sessionCounter}`, kind,
    phase: kind === "practice" && options.practice_error ? "error" : kind === "practice" && options.practice_text ? "done" : "idle", text: kind === "pad" ? pad : options.practice_text || "",
    error: kind === "practice" ? options.practice_error || null : null, target_name: null,
    external_paste_available: !shortcutDenied(), external_paste_reason: shortcutDenied() ? capabilities.pad_external_paste.reason : null });
  let recordingStorageError = Boolean(options.recording_storage_error);
  let recordingRecovered = false;
  const meetingStatus = () => {
    const retained = options.section === "recording" && !recordingRecovered;
    const storageError = retained && recordingStorageError;
    const paused = retained && Boolean(options.transcription_paused);
    const persistenceError = storageError ? "Je opname kon niet volledig worden bewaard. Probeer opnieuw te bewaren." : null;
    return {
      recording: retained && !storageError, phase: storageError ? "error" : retained ? "recording" : "idle", busy: retained && !storageError,
      error: persistenceError, notes, session_id: retained ? "qa-recording" : null, id: retained ? meetingId : null,
      title: "Projectoverleg", elapsed_secs: 754, system_audio: true, heard_system_audio: true,
      echo_cancellation_active: retained && !storageError, audio_warning: null, persistence_error: persistenceError,
      transcription_warning: paused ? "Transcriptie is tijdelijk gepauzeerd. Je audio en herstelbestanden blijven lokaal bewaard." : null,
      terminal_error: storageError ? "De opname is gestopt omdat bewaren niet lukt." : null, can_retry_persistence: storageError,
      pending: storageError ? 1 : paused ? 3 : 0, transcript: retained ? clone(transcript) : [],
      ...(retained ? { recognition: { provider_kind: "local", provider_name: "Lokaal", model: settings.meeting_stt.model, language: "nl" } } : {}),
    };
  };

  async function invoke(command, args = {}) {
    calls.push(command);
    switch (command) {
      case "app_info": return info();
      case "save_settings": settings = clone(args.settings); return info();
      case "get_permissions": return clone(permissions);
      case "request_permission": {
        permissions = permissions.map(permission => permission.kind === args.kind ? { ...permission, status: "granted" } : permission);
        if (args.kind === "accessibility") for (const capability of ["global_hotkey", "selection", "pad_external_paste"]) capabilities[capability] = available();
        return clone(permissions);
      }
      case "onboarding_readiness": {
        const slot = args.target === "meeting" ? settings.meeting_stt : settings.dictation_stt;
        const account = settings.providers.find(account => account.id === slot.provider_id);
        const definition = kinds.find(kind => kind.kind === account?.kind);
        const denied = permissions.some(permission => permission.kind === "microphone" && ["denied", "restricted", "not_determined"].includes(permission.status));
        const configured = Boolean(account && slot.model && (account.kind === "local" ? models.some(model => model.id === slot.model && model.state.kind === "downloaded") : !definition?.requires_key || keys[account.id]));
        const accessibility = permissions.find(permission => permission.kind === "accessibility")?.status === "granted";
        return { configured, can_record: configured && !denied, can_use_shortcut: accessibility, can_paste_externally: accessibility, reason: denied ? "Geef Pennel toegang tot je microfoon." : configured ? null : "Kies een werkende spraakherkenning.", resume_step: configured ? "permissions" : "engine" };
      }
      case "pad_text": return pad;
      case "save_pad_text": {
        pad = String(args.text || "");
        for (const session of sessions.values()) if (session.kind === "pad") {
          session.text = pad;
          if (session.phase === "error") { session.phase = "idle"; session.error = null; }
          emit("dictation-session", session);
        }
        return null;
      }
      case "open_dictation_session": { const session = sessionSnapshot(args.kind); sessions.set(session.session_id, session); return clone(session); }
      case "get_dictation_session": return clone(sessions.get(args.sessionId || args.session_id));
      case "toggle_dictation_session": {
        const session = sessions.get(args.sessionId || args.session_id);
        if (!session) throw new Error("Onbekende QA-sessie");
        if (session.phase === "listening") {
          session.phase = "done"; session.text = session.kind === "pad" ? pad + "\n\n" + sentence : sentence;
          if (session.kind === "pad") pad = session.text;
        } else session.phase = "listening";
        emit("dictation-session", session); return null;
      }
      case "cancel_dictation_session": { const session = sessions.get(args.sessionId || args.session_id); if (session) { session.phase = "idle"; emit("dictation-session", session); } return null; }
      case "close_dictation_session": sessions.delete(args.sessionId || args.session_id); return null;
      case "dictation_state": return clone(pillSnapshot);
      case "frontend_ready": globalThis.__PENNEL_FIXTURE_READY__ = true; return null;
      case "frontend_flushed": return null;
      case "app_identity": return null;
      case "history": return clone(history);
      case "delete_history": {
        const index = history.findIndex(entry => entry.id === args.id);
        if (index !== -1) history.splice(index, 1);
        emit("history-changed", null); emit("settings-changed", null); return null;
      }
      case "save_history": {
        const entry = history.find(entry => entry.id === args.id);
        if (!entry || entry.status === "failed") throw new Error("Onbekend tijdelijk QA-dictaat");
        entry.status = "done"; entry.error = null;
        emit("history-changed", null); emit("settings-changed", null); return null;
      }
      case "copy_history": {
        if (!history.some(entry => entry.id === args.id)) throw new Error("Onbekend QA-dictaat");
        return null;
      }
      case "retry_history": return sentence;
      case "local_models": return clone(args.supportedOnly || args.supported_only ? models.filter(model => model.languages.some(language => language.split(/[-_]/)[0] === (args.language || "nl"))) : models);
      case "model_state": return clone(modelFor(args.id)?.state || { kind: "not_downloaded" });
      case "download_model": {
        if (!modelFor(args.id)) throw new Error("Onbekend QA-model");
        if (activeDownloads.has(args.id)) return null;
        activeDownloads.add(args.id);
        queueMicrotask(() => completeDownload(args.id, options.download_error || null));
        return null;
      }
      case "join_model_download": {
        if (!activeDownloads.has(args.id)) return false;
        const model = modelFor(args.id);
        return new Promise((resolve, reject) => {
          if (!downloadWaiters.has(args.id)) downloadWaiters.set(args.id, []);
          downloadWaiters.get(args.id).push({ resolve, reject });
          emit("download-progress", { id: args.id, downloaded: model?.state.bytes || 0, total: model?.size_bytes || 0 });
        });
      }
      case "cancel_download": {
        completeDownload(args.id, "cancelled");
        return null;
      }
      case "delete_model": {
        if (activeDownloads.has(args.id)) throw new Error("Dit model wordt nog gedownload.");
        const model = modelFor(args.id); if (model) model.state = { kind: "not_downloaded" }; return info();
      }
      case "add_provider": { const definition = kinds.find(kind => kind.kind === args.kind); settings.providers.push({ id: `qa-${args.kind}`, kind: args.kind, name: definition?.label || args.kind, base_url: definition?.default_base_url || "" }); return info(); }
      case "remove_provider": settings.providers = settings.providers.filter(provider => provider.id !== args.id); return info();
      case "set_key": keys[args.account] = Boolean(args.key); providerRevisions[args.account] = (providerRevisions[args.account] || 0) + 1; return info();
      case "provider_models": return [{ id: "gpt-4o-mini-transcribe", name: "GPT-4o mini Transcribe", audio_input: true, is_router: false }, { id: "llama-3.3-70b-versatile", name: "Llama 3.3", audio_input: false, is_router: false }];
      case "test_provider_task": {
        if (!["dictation_stt", "meeting_stt", "llm"].includes(args.slot)) throw new Error("Onbekende QA-taak");
        const choice = settings[args.slot];
        const provider = settings.providers.find(provider => provider.id === choice.provider_id);
        const kind = kinds.find(kind => kind.kind === provider?.kind);
        const configured = Boolean(provider && kind && (args.slot === "llm" ? kind.llm : kind.stt) && choice.model && (!kind.requires_key || keys[provider.id])
          && (provider.kind !== "local" || modelFor(choice.model)?.state.kind === "downloaded"));
        return { verified: configured, message: configured ? "Test geslaagd met uitsluitend synthetische voorbeelddata." : "Stel eerst het gekozen model en de provider in." };
      }
      case "detect_cli_agents": return [];
      case "check_cli_agent": return "Niet uitgevoerd in deze fixture.";
      case "meeting_status": return meetingStatus();
      case "retry_meeting_persistence": {
        const status = meetingStatus();
        if (!status.can_retry_persistence || args.sessionId !== status.session_id) throw new Error("Deze QA-opnamesessie is niet meer actief.");
        recordingStorageError = false; recordingRecovered = true;
        emit("meeting-finished", { id: meetingId, session_id: status.session_id, error: null, flush_warning: null });
        emit("meeting-updated", null); return null;
      }
      case "meetings": return options.empty ? [] : clone(library);
      case "meeting_detail": return { info: clone(meetingInfo(args.id)), notes, summary, transcript: clone(transcript), speakers: [{ voice: 1, name: "Noor" }], dir: `/QA/Pennel Preview/Meetings/${meetingId}`, attendees: ["Noor", "Sam"], audio_tracks: [{ speaker: "me", available: false }, { speaker: "others", available: false }] };
      case "search_library": return { meetings: options.empty ? [] : clone(library), passages: [] };
      case "ask_meetings": return { text: "De planning wordt maandag afgerond. Jij bevestigt de volgende afspraak.", citations: [{ number: 1, passage: { meeting_id: meetingId, title: "Projectoverleg", start_iso: meeting.start_iso, kind: "transcript", start_ms: 0, text: transcript[0].text, snippet: transcript[0].text } }] };
      case "meeting_templates": return [["algemeen", "Algemeen"], ["een-op-een", "1-op-1"], ["klantgesprek", "Klantgesprek"], ["standup", "Standup"]].map(([id, name]) => ({ id, name, builtin: true, customized: false }));
      case "template_source": return "## Samenvatting\n## Beslissingen\n## Actiepunten";
      case "mark_lines": return summary.split("\n").filter(Boolean).map(text => ({ text, mine: false }));
      case "calendar_snapshot": return { has_access: false, calendars: [], current_event: null };
      case "record_prompt": return null;
      case "check_for_updates": return { available: false, version: null, body: null };
      case "export_meeting": return { title: "Projectoverleg", markdown: summary, html: "<h2>Besluiten</h2><p>De planning wordt maandag afgerond.</p>" };
      case "choose_meetings_dir": return null;
      case "pick_style_application": return { id: "com.apple.mail", name: "Mail" };
      case "start_meeting": return meetingId;
      case "save_template": return "qa-template";
      case "rename_meeting": return meetingId;
      case "summarize_meeting": return summary;
      case "retranscribe_meeting": return 0;
      case "diarize_meeting": return 1;
      case "cloud_account": if (!signedIn) throw notSignedIn(); return clone(cloudAccount);
      case "cloud_devices": if (!signedIn) throw notSignedIn(); return clone(cloudDevices);
      case "claude_access": case "set_claude_access": return { enabled: Boolean(options.claude), status: options.claude ? { kind: "connected", last_request_ms: now - 2 * 60_000 } : { kind: "off" }, mcp_url: "https://api.pennel.app/mcp" };
      case "cloud_sync_last": return syncReport();
      case "cloud_folder_warning": return null;
      case "meeting_sync_states": return signedIn && !options.empty ? [{ id: meetingId, state: options.cloud === "conflict" ? "conflict" : "synced" }] : [];
      case "meeting_conflict_files": return options.cloud === "conflict" ? ["Notities (conflict MacBook van Noor).md"] : [];
      case "cloud_login_start": return { user_code: "QAFX-2026", verification_uri: "https://example.com/qa-login", device_code: "qa-device-code", interval_secs: 5 };
      case "cloud_login_poll": return { kind: "pending" };
      case "cloud_sync_now": return syncReport();
      case "cloud_logout": case "cloud_rename_device": case "cloud_revoke_device": case "cloud_sync_disable": case "cloud_sync_confirm_deletes":
      case "delete_conflict_copy": case "open_url": case "copy_secret_text": return null;
      case "show_window": case "show_workspace": case "copy_text": case "play_meeting_audio": case "stop_audio":
      case "answer_record_prompt": case "toggle_dictation": case "install_update": case "share_meeting": case "open_log": case "open_path":
      case "set_meeting_notes": case "set_meeting_title": case "save_meeting_notes": case "set_meeting_attendees": case "rename_speaker":
      case "delete_meeting": case "delete_template": case "test_apple_intelligence": case "stop_meeting": return null;
      default: unknown.push(command); throw new Error(`Onbekend fixturecommando: ${command}`);
    }
  }
  globalThis.__PENNEL_PLATFORM__ = {
    fixture,
    invoke,
    async listen(name, handler) {
      if (!listeners.has(name)) listeners.set(name, new Set());
      listeners.get(name).add(handler);
      return () => listeners.get(name)?.delete(handler);
    },
  };
  globalThis.__PENNEL_FIXTURE__ = { options: fixture, calls, unknown, emit, info, completeDownload };
})();
