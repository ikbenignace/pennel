//! The webview's API. Blocking work (network, model loading) runs off the main thread.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use pennel_core::{
    ChatTurn, DetectedAgent, HistoryEntry, LibrarySearch, LocalModel, MeetingAnswer, MeetingExport,
    MeetingInfo, MeetingTemplate, ModelState, ProviderKind, ProviderTask, RemoteModel, Segment,
    Settings, SpeakerName,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::capabilities::{self, Capability, PlatformCapabilities};
use crate::diagnostics::{self, Log};
use crate::dictation::Msg;
use crate::meeting_runtime;
use crate::state::Shared;

const LOG: Log = Log::new("app");

type Result<T> = std::result::Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(err)?
}

#[derive(Serialize)]
pub struct KindInfo {
    kind: ProviderKind,
    label: &'static str,
    requires_key: bool,
    default_base_url: String,
    stt: bool,
    llm: bool,
    default_stt_model: String,
    default_llm_model: String,
    is_cli_agent: bool,
    availability: Capability,
}

const KINDS: [(ProviderKind, &str); 16] = [
    (ProviderKind::OpenAi, "OpenAI"),
    (ProviderKind::OpenRouter, "OpenRouter"),
    (ProviderKind::Groq, "Groq"),
    (ProviderKind::Mistral, "Mistral"),
    (ProviderKind::Ollama, "Ollama (lokaal)"),
    (ProviderKind::LmStudio, "LM Studio (lokaal)"),
    (ProviderKind::Custom, "Eigen server"),
    (ProviderKind::Local, "Lokaal op deze computer"),
    (ProviderKind::AppleSpeech, "Apple-dicteren"),
    (ProviderKind::AppleIntelligence, "Apple Intelligence"),
    (ProviderKind::ClaudeCode, "Claude Code"),
    (ProviderKind::Codex, "Codex"),
    (ProviderKind::OpenCode, "OpenCode"),
    (ProviderKind::Copilot, "GitHub Copilot"),
    (ProviderKind::CursorAgent, "Cursor"),
    (ProviderKind::GeminiCli, "Gemini CLI"),
];

fn provider_kinds(apple_speech: &Capability, apple_intelligence: &Capability) -> Vec<KindInfo> {
    KINDS
        .iter()
        .map(|(kind, label)| KindInfo {
            kind: *kind,
            label,
            requires_key: pennel_core::requires_key(*kind),
            default_base_url: pennel_core::default_base_url(*kind),
            stt: pennel_core::supports(*kind, ProviderTask::DictationStt),
            llm: pennel_core::supports(*kind, ProviderTask::Llm),
            default_stt_model: pennel_core::default_model(*kind, ProviderTask::DictationStt),
            default_llm_model: pennel_core::default_model(*kind, ProviderTask::Llm),
            is_cli_agent: kind.is_cli_agent(),
            availability: match *kind {
                ProviderKind::AppleSpeech => apple_speech.clone(),
                ProviderKind::AppleIntelligence => apple_intelligence.clone(),
                _ => Capability::available(),
            },
        })
        .collect()
}

#[derive(Serialize)]
pub struct AppInfo {
    settings: Settings,
    keys: HashMap<String, bool>,
    kinds: Vec<KindInfo>,
    platform: &'static str,
    version: String,
    preview: bool,
    migration_warning: Option<String>,
    hotkey_error: Option<String>,
    log_path: Option<String>,
    data_dir: String,
    capabilities: PlatformCapabilities,
    diarizer_model_id: String,
    recording_app_ids: Vec<String>,
}

pub(crate) fn info(shared: &Shared) -> AppInfo {
    let settings = shared.settings();
    let keys = settings
        .providers
        .iter()
        .map(|p| (p.id.clone(), shared.secrets.get(&p.id).is_some()))
        .collect();
    let hotkey_error = shared.hotkey_error.lock().unwrap().clone();
    let capabilities = capabilities::snapshot(hotkey_error.as_deref());
    AppInfo {
        keys,
        settings,
        kinds: provider_kinds(&capabilities.apple_speech, &capabilities.apple_intelligence),
        platform: std::env::consts::OS,
        version: shared.version.clone(),
        preview: shared.preview,
        migration_warning: shared.migration_warning.clone(),
        hotkey_error,
        log_path: diagnostics::file_path().map(|p| p.to_string_lossy().into_owned()),
        data_dir: shared.data_dir.to_string_lossy().into_owned(),
        capabilities,
        diarizer_model_id: shared.engine.diarizer_model_id(),
        recording_app_ids: pennel_core::known_call_apps()
            .into_iter()
            .chain(pennel_core::browser_apps())
            .collect(),
    }
}

#[tauri::command]
pub fn app_info(shared: State<'_, Arc<Shared>>) -> AppInfo {
    info(&shared)
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
) -> Result<AppInfo> {
    let old_settings = shared.settings();
    if shared.preview && old_settings.meetings_dir != settings.meetings_dir {
        return Err(
            "Deze testversie gebruikt een eigen meetingmap om je bestaande gegevens te beschermen."
                .into(),
        );
    }
    let recording = shared.recording.lock().unwrap();
    if old_settings.meetings_dir != settings.meetings_dir
        && (recording.is_busy()
            || shared
                .library_jobs
                .load(std::sync::atomic::Ordering::SeqCst)
                > 0)
    {
        return Err(
            "De meetingmap kan pas worden gewijzigd nadat de opname en verwerking zijn afgerond."
                .into(),
        );
    }
    let previous = old_settings.hotkey;
    shared.engine.update_settings(settings).map_err(err)?;
    drop(recording);
    let hotkey = shared.settings().hotkey;
    if hotkey != previous {
        let listener = shared.hotkey.lock().unwrap().clone();
        if let Some(listener) = listener {
            listener.set_trigger(hotkey);
        }
        let _ = shared.dictation.send(Msg::TriggerChanged);
        if let Some(hint) = app.try_state::<crate::TrayHint>() {
            let _ = hint.0.set_text(crate::hotkey_hint(&shared));
        }
        LOG.info(format!("sneltoets gewijzigd naar {hotkey:?}"));
    }
    crate::record_shortcut::refresh(&app, &shared);
    crate::apply_appearance(&app);
    if old_settings.dictation_pad_auto_paste && !shared.settings().dictation_pad_auto_paste {
        crate::owned_dictation::invalidate_external(&app);
    }
    let _ = app.emit("settings-changed", ());
    Ok(info(&shared))
}

#[tauri::command]
pub async fn add_provider(kind: ProviderKind, shared: State<'_, Arc<Shared>>) -> Result<AppInfo> {
    let shared = shared.inner().clone();
    blocking(move || {
        let executable = if kind.is_cli_agent() {
            pennel_core::detect_cli_agents()
                .into_iter()
                .find(|agent| agent.kind == kind)
                .map(|agent| agent.path)
        } else {
            None
        };
        let mut settings = shared.settings();
        let mut account = pennel_core::new_account(kind, settings.providers.clone());
        if let Some(path) = executable {
            account.base_url = path;
        }
        settings.providers.push(account);
        shared.engine.update_settings(settings).map_err(err)?;
        Ok(info(&shared))
    })
    .await
}

#[tauri::command]
pub fn remove_provider(id: String, shared: State<'_, Arc<Shared>>) -> Result<AppInfo> {
    let mut settings = shared.settings();
    settings.providers.retain(|p| p.id != id);
    shared.engine.update_settings(settings).map_err(err)?;
    let _ = shared.secrets.set(&id, "");
    Ok(info(&shared))
}

#[tauri::command]
pub fn set_key(account: String, key: String, shared: State<'_, Arc<Shared>>) -> Result<AppInfo> {
    shared.secrets.set(&account, &key)?;
    Ok(info(&shared))
}

#[tauri::command]
pub async fn provider_models(
    id: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<Vec<RemoteModel>> {
    let shared = shared.inner().clone();
    blocking(move || {
        let settings = shared.settings();
        let account = settings
            .providers
            .iter()
            .find(|p| p.id == id)
            .ok_or("onbekende provider")?;
        if account.kind.is_cli_agent()
            || matches!(
                account.kind,
                ProviderKind::AppleSpeech | ProviderKind::AppleIntelligence | ProviderKind::Local
            )
        {
            return Ok(Vec::new());
        }
        pennel_core::provider_models(account.base_url.clone(), shared.secrets.get(&id)).map_err(err)
    })
    .await
}

#[derive(Serialize)]
pub struct LocalModelView {
    #[serde(flatten)]
    model: LocalModel,
    state: ModelState,
}

#[tauri::command]
pub fn local_models(
    language: Option<String>,
    supported_only: bool,
    shared: State<'_, Arc<Shared>>,
) -> Vec<LocalModelView> {
    shared
        .engine
        .local_models(language, supported_only)
        .into_iter()
        .map(|model| LocalModelView {
            state: shared.engine.model_state(model.id.clone()),
            model,
        })
        .collect()
}

#[tauri::command]
pub fn model_state(id: String, shared: State<'_, Arc<Shared>>) -> ModelState {
    if validate_meeting_id(&id).is_err() {
        return ModelState::NotDownloaded;
    }
    shared.engine.model_state(id)
}

struct DownloadEvents {
    app: AppHandle,
    id: String,
    last: std::sync::Mutex<std::time::Instant>,
}

#[derive(Serialize, Clone)]
struct DownloadProgress {
    id: String,
    downloaded: u64,
    total: u64,
}

#[derive(Serialize, Clone)]
struct DownloadDone {
    id: String,
    error: Option<String>,
}

impl pennel_core::DownloadListener for DownloadEvents {
    fn on_progress(&self, downloaded: u64, total: u64) {
        let mut last = self.last.lock().unwrap();
        if last.elapsed().as_millis() < 200 && downloaded < total {
            return;
        }
        *last = std::time::Instant::now();
        let _ = self.app.emit(
            "download-progress",
            DownloadProgress {
                id: self.id.clone(),
                downloaded,
                total,
            },
        );
    }
}

#[tauri::command]
pub fn download_model(id: String, app: AppHandle, shared: State<'_, Arc<Shared>>) {
    let shared = shared.inner().clone();
    std::thread::spawn(move || {
        let listener = Arc::new(DownloadEvents {
            app: app.clone(),
            id: id.clone(),
            last: std::sync::Mutex::new(std::time::Instant::now()),
        });
        let result = shared.engine.download_model(id.clone(), listener);
        if let Err(e) = &result {
            LOG.error(format!("download {id}: {e}"));
        } else {
            LOG.info(format!("download {id} klaar"));
            let _ = shared.engine.update_settings(shared.settings());
        }
        let _ = app.emit(
            "download-done",
            DownloadDone {
                id,
                error: result.err().map(|e| e.to_string()),
            },
        );
    });
}

#[tauri::command]
pub fn cancel_download(id: String, shared: State<'_, Arc<Shared>>) {
    shared.engine.cancel_download(id);
}

#[tauri::command]
pub fn delete_model(id: String, shared: State<'_, Arc<Shared>>) -> Result<AppInfo> {
    shared.engine.delete_model(id).map_err(err)?;
    Ok(info(&shared))
}

#[tauri::command]
pub fn history(limit: u32, shared: State<'_, Arc<Shared>>) -> Result<Vec<HistoryEntry>> {
    shared.engine.history(limit).map_err(err)
}

#[tauri::command]
pub async fn retry_history(id: String, shared: State<'_, Arc<Shared>>) -> Result<String> {
    let shared = shared.inner().clone();
    blocking(move || {
        let keys = shared.keys();
        shared.engine.retry(id, keys).map(|r| r.text).map_err(err)
    })
    .await
}

#[tauri::command]
pub fn delete_history(id: String, shared: State<'_, Arc<Shared>>) -> Result<()> {
    shared.engine.delete_history(id).map_err(err)
}

#[tauri::command]
pub fn open_log(app: AppHandle) -> Result<()> {
    open_log_file(&app)
}

pub fn open_log_file(app: &AppHandle) -> Result<()> {
    let path = diagnostics::file_path().ok_or("geen logboek")?;
    if !path.exists() {
        Log::new("app").info("logboek aangemaakt");
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
pub fn open_path(path: String, app: AppHandle, shared: State<'_, Arc<Shared>>) -> Result<()> {
    let path = PathBuf::from(path).canonicalize().map_err(err)?;
    let root = PathBuf::from(shared.engine.meeting_dir(String::new()))
        .canonicalize()
        .map_err(err)?;
    if path != root && path.parent() != Some(root.as_path()) {
        return Err("Alleen de ingestelde meetingmap kan worden geopend.".into());
    }
    if !path.is_dir() {
        return Err("Geen meetingmap gevonden.".into());
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(err)
}

fn validate_meeting_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || id
            .chars()
            .any(|character| matches!(character, '/' | '\\' | ':' | '\0') || character.is_control())
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err("Ongeldig meetingnummer.".into());
    }
    Ok(())
}

pub(crate) fn checked_meeting_dir(engine: &pennel_core::PennelEngine, id: &str) -> Result<PathBuf> {
    validate_meeting_id(id)?;
    let root = PathBuf::from(engine.meeting_dir(String::new()))
        .canonicalize()
        .map_err(err)?;
    let directory = root.join(id).canonicalize().map_err(err)?;
    if directory.parent() != Some(root.as_path()) || !directory.is_dir() {
        return Err("Meeting niet gevonden in de ingestelde meetingmap.".into());
    }
    if engine.meeting_info(id.to_owned()).is_none() {
        return Err("Meeting niet gevonden.".into());
    }
    Ok(directory)
}

fn require_inactive_meeting(shared: &Shared, id: &str) -> Result<()> {
    let lifecycle = shared.recording.lock().unwrap();
    let active_id = shared
        .meeting_context
        .lock()
        .unwrap()
        .as_ref()
        .map(|context| context.session.id());
    if lifecycle.is_busy() && active_id.as_deref() == Some(id) {
        return Err("Wacht tot de opname is afgerond.".into());
    }
    Ok(())
}

#[derive(Serialize)]
pub struct MeetingStatus {
    recording: bool,
    busy: bool,
    phase: pennel_core::RecordingPhase,
    error: Option<String>,
    session_id: Option<String>,
    id: Option<String>,
    title: Option<String>,
    elapsed_secs: u64,
    system_audio: bool,
    heard_system_audio: bool,
    audio_warning: Option<String>,
    consent_copied: bool,
    pending: u32,
    transcript: Vec<Segment>,
    notes: String,
}

#[tauri::command]
pub fn meeting_status(shared: State<'_, Arc<Shared>>) -> MeetingStatus {
    status(&shared)
}

fn status(shared: &Shared) -> MeetingStatus {
    let (state, details) = {
        let lifecycle = shared.recording.lock().unwrap();
        let state = lifecycle.snapshot();
        let details = shared
            .meeting_context
            .lock()
            .unwrap()
            .as_ref()
            .filter(|context| state.session_id.as_deref() == Some(context.token.as_str()))
            .map(|context| {
                (
                    context.session.clone(),
                    context.title.clone(),
                    context
                        .duration_secs
                        .unwrap_or_else(|| context.started.elapsed().as_secs()),
                    context.system_audio,
                    context.heard_system_audio,
                    context.audio_warning.clone(),
                    context.consent_copied,
                )
            });
        (state, details)
    };
    let recording = state.phase == pennel_core::RecordingPhase::Recording;
    let busy = !matches!(
        state.phase,
        pennel_core::RecordingPhase::Idle | pennel_core::RecordingPhase::Error
    );
    let live_system = shared
        .meeting
        .lock()
        .unwrap()
        .as_ref()
        .filter(|meeting| {
            details
                .as_ref()
                .is_some_and(|(session, ..)| session.id() == meeting.session.id())
        })
        .map(|meeting| {
            (
                meeting.has_system_audio(),
                meeting.heard_system_audio(),
                meeting.audio_warning(),
            )
        });
    let mut result = MeetingStatus {
        recording,
        busy,
        phase: state.phase,
        error: state.error,
        session_id: state.session_id,
        id: None,
        title: None,
        elapsed_secs: 0,
        system_audio: false,
        heard_system_audio: false,
        audio_warning: None,
        consent_copied: false,
        pending: 0,
        transcript: Vec::new(),
        notes: String::new(),
    };
    if let Some((
        session,
        title,
        elapsed_secs,
        system_audio,
        heard_system_audio,
        audio_warning,
        consent_copied,
    )) = details
    {
        let id = session.id();
        result.notes = shared.engine.meeting_notes(id.clone());
        result.id = Some(id);
        result.title = Some(title);
        result.elapsed_secs = elapsed_secs;
        result.consent_copied = consent_copied;
        (
            result.system_audio,
            result.heard_system_audio,
            result.audio_warning,
        ) = live_system.unwrap_or((system_audio, heard_system_audio, audio_warning));
        result.pending = session.pending();
        result.transcript = session.transcript();
    }
    result
}

#[tauri::command]
pub async fn start_meeting(title: String, shared: State<'_, Arc<Shared>>) -> Result<String> {
    let shared = shared.inner().clone();
    blocking(move || meeting_runtime::start(&shared, title, None, Vec::new())).await
}

#[tauri::command]
pub fn stop_meeting(app: AppHandle, shared: State<'_, Arc<Shared>>) -> Result<()> {
    meeting_runtime::stop(app, shared.inner().clone(), None)
}

#[tauri::command]
pub fn set_meeting_notes(
    text: String,
    session_id: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<()> {
    let lifecycle = shared.recording.lock().unwrap();
    let state = lifecycle.snapshot();
    if !matches!(
        state.phase,
        pennel_core::RecordingPhase::Recording | pennel_core::RecordingPhase::Stopping
    ) || state.session_id.as_deref() != Some(session_id.as_str())
    {
        return Err("Deze opnamesessie is niet meer beschikbaar om notities te wijzigen.".into());
    }
    let session = shared
        .meeting_context
        .lock()
        .unwrap()
        .as_ref()
        .filter(|context| context.token == session_id)
        .map(|context| context.session.clone())
        .ok_or("geen meeting bezig")?;
    session.set_notes(text).map_err(err)
}

#[tauri::command]
pub fn set_meeting_title(
    title: String,
    session_id: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<()> {
    meeting_runtime::set_title(&shared, title, &session_id)
}

#[tauri::command]
pub fn meetings(query: String, shared: State<'_, Arc<Shared>>) -> Vec<MeetingInfo> {
    if query.trim().is_empty() {
        shared.engine.meetings()
    } else {
        shared.engine.search_meetings(query)
    }
}

#[derive(Serialize)]
pub struct MeetingDetail {
    info: Option<MeetingInfo>,
    notes: String,
    summary: Option<String>,
    transcript: Vec<Segment>,
    speakers: Vec<SpeakerName>,
    dir: String,
    attendees: Vec<String>,
    audio_tracks: Vec<AudioTrack>,
}

#[derive(Debug, Serialize)]
pub struct AudioTrack {
    speaker: pennel_core::Speaker,
    available: bool,
}

fn audio_tracks(dir: &Path) -> Vec<AudioTrack> {
    let root = dir.canonicalize().ok();
    [
        (pennel_core::Speaker::Me, "ik.m4a"),
        (pennel_core::Speaker::Others, "anderen.m4a"),
    ]
    .into_iter()
    .map(|(speaker, file)| AudioTrack {
        speaker,
        available: root.as_ref().is_some_and(|root| {
            root.join(file)
                .canonicalize()
                .is_ok_and(|path| path.is_file() && path.parent() == Some(root.as_path()))
        }),
    })
    .collect()
}

#[tauri::command]
pub fn meeting_detail(id: String, shared: State<'_, Arc<Shared>>) -> Result<MeetingDetail> {
    let engine = &shared.engine;
    let directory = checked_meeting_dir(engine, &id)?;
    Ok(MeetingDetail {
        info: engine.meeting_info(id.clone()),
        notes: engine.meeting_notes(id.clone()),
        summary: engine.meeting_markdown(id.clone()),
        transcript: engine.meeting_transcript(id.clone()),
        speakers: engine.meeting_speakers(id.clone()),
        attendees: engine.meeting_attendees(id.clone()),
        dir: directory.to_string_lossy().into_owned(),
        audio_tracks: audio_tracks(&directory),
    })
}

#[tauri::command]
pub fn meeting_templates(shared: State<'_, Arc<Shared>>) -> Vec<MeetingTemplate> {
    shared.engine.meeting_templates()
}

#[tauri::command]
pub async fn summarize_meeting(
    id: String,
    template_id: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<String> {
    let shared = shared.inner().clone();
    blocking(move || {
        let _library_job = meeting_runtime::LibraryJob::new(&shared)?;
        checked_meeting_dir(&shared.engine, &id)?;
        require_inactive_meeting(&shared, &id)?;
        let keys = shared.keys();
        shared
            .engine
            .summarize_meeting(id, template_id, keys)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn retranscribe_meeting(id: String, shared: State<'_, Arc<Shared>>) -> Result<u32> {
    let shared = shared.inner().clone();
    blocking(move || {
        let _library_job = meeting_runtime::LibraryJob::new(&shared)?;
        checked_meeting_dir(&shared.engine, &id)?;
        require_inactive_meeting(&shared, &id)?;
        let keys = shared.keys();
        shared.engine.retranscribe_failed(id, keys).map_err(err)
    })
    .await
}

#[tauri::command]
pub fn rename_meeting(id: String, title: String, shared: State<'_, Arc<Shared>>) -> Result<String> {
    checked_meeting_dir(&shared.engine, &id)?;
    require_inactive_meeting(&shared, &id)?;
    shared.engine.rename_meeting(id, title).map_err(err)
}

#[tauri::command]
pub fn delete_meeting(id: String, shared: State<'_, Arc<Shared>>) -> Result<()> {
    checked_meeting_dir(&shared.engine, &id)?;
    require_inactive_meeting(&shared, &id)?;
    shared.engine.delete_meeting(id).map_err(err)
}

#[tauri::command]
pub async fn search_library(
    query: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<LibrarySearch> {
    let shared = shared.inner().clone();
    blocking(move || {
        let _library_job = meeting_runtime::LibraryJob::new(&shared)?;
        Ok(shared.engine.search_library(query))
    })
    .await
}

#[tauri::command]
pub async fn ask_meetings(
    question: String,
    history: Vec<ChatTurn>,
    meeting_id: Option<String>,
    shared: State<'_, Arc<Shared>>,
) -> Result<MeetingAnswer> {
    let shared = shared.inner().clone();
    blocking(move || {
        let _library_job = meeting_runtime::LibraryJob::new(&shared)?;
        if question.trim().is_empty() {
            return Err("Typ eerst een vraag.".into());
        }
        if let Some(id) = &meeting_id {
            checked_meeting_dir(&shared.engine, id)?;
        }
        shared
            .engine
            .ask_meetings(question, history, meeting_id, shared.keys())
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub fn template_source(id: String, shared: State<'_, Arc<Shared>>) -> Option<String> {
    shared.engine.template_source(id)
}

#[tauri::command]
pub fn save_template(
    id: Option<String>,
    name: String,
    body: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<String> {
    if name.trim().is_empty() {
        return Err("Geef het sjabloon een naam.".into());
    }
    shared.engine.save_template(id, name, body).map_err(err)
}

#[tauri::command]
pub fn delete_template(id: String, shared: State<'_, Arc<Shared>>) -> Result<()> {
    shared.engine.delete_template(id).map_err(err)
}

#[tauri::command]
pub fn rename_speaker(
    id: String,
    voice: u32,
    name: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<()> {
    checked_meeting_dir(&shared.engine, &id)?;
    require_inactive_meeting(&shared, &id)?;
    shared.engine.rename_speaker(id, voice, name).map_err(err)
}

#[tauri::command]
pub async fn diarize_meeting(id: String, shared: State<'_, Arc<Shared>>) -> Result<u32> {
    let shared = shared.inner().clone();
    blocking(move || {
        let _library_job = meeting_runtime::LibraryJob::new(&shared)?;
        let dir = checked_meeting_dir(&shared.engine, &id)?;
        require_inactive_meeting(&shared, &id)?;
        let audio = dir.join("anderen.m4a").canonicalize().map_err(err)?;
        if audio.parent() != Some(dir.as_path()) {
            return Err("Het audiospoor valt buiten de meetingmap.".into());
        }
        let samples = crate::platform_audio::decode_audio(&audio)?;
        shared.engine.diarize_meeting(id, samples).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn export_meeting(
    id: String,
    include_transcript: bool,
    shared: State<'_, Arc<Shared>>,
) -> Result<MeetingExport> {
    let shared = shared.inner().clone();
    blocking(move || {
        let _library_job = meeting_runtime::LibraryJob::new(&shared)?;
        checked_meeting_dir(&shared.engine, &id)?;
        shared
            .engine
            .export_meeting(id, include_transcript)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub fn save_meeting_notes(id: String, text: String, shared: State<'_, Arc<Shared>>) -> Result<()> {
    checked_meeting_dir(&shared.engine, &id)?;
    shared.engine.set_meeting_notes(id, text).map_err(err)
}

#[tauri::command]
pub fn set_meeting_attendees(
    id: String,
    attendees: Vec<String>,
    shared: State<'_, Arc<Shared>>,
) -> Result<()> {
    checked_meeting_dir(&shared.engine, &id)?;
    shared
        .engine
        .set_meeting_attendees(id, attendees)
        .map_err(err)
}

#[tauri::command]
pub async fn detect_cli_agents() -> Result<Vec<DetectedAgent>> {
    blocking(|| Ok(pennel_core::detect_cli_agents())).await
}

#[tauri::command]
pub async fn check_cli_agent(
    id: String,
    model: String,
    shared: State<'_, Arc<Shared>>,
) -> Result<String> {
    let shared = shared.inner().clone();
    blocking(move || {
        let account = shared
            .settings()
            .providers
            .into_iter()
            .find(|provider| provider.id == id)
            .ok_or("Onbekende provider.")?;
        if !account.kind.is_cli_agent() {
            return Err("Deze provider is geen CLI-agent.".into());
        }
        let path = if account.base_url.is_empty() {
            pennel_core::detect_cli_agents()
                .into_iter()
                .find(|agent| agent.kind == account.kind)
                .map(|agent| agent.path)
                .ok_or(
                    "Deze CLI-agent is niet geïnstalleerd. Installeer de agent en meld je aan.",
                )?
        } else {
            account.base_url
        };
        pennel_core::check_cli_agent(account.kind, path, model).map_err(err)
    })
    .await
}

#[tauri::command]
pub fn show_main(app: AppHandle) {
    crate::show_main(&app, None);
}

#[tauri::command]
pub fn hide_window(window: tauri::Window) {
    #[cfg(target_os = "macos")]
    if matches!(window.label(), "record-prompt" | "pill")
        && let Ok(pointer) = window.ns_window()
    {
        unsafe {
            crate::macos::hide_panel(pointer);
        }
        return;
    }
    let _ = window.hide();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_contract_exposes_every_persisted_kind_with_correct_tasks() {
        let kinds = provider_kinds(
            &Capability::unsupported("macOS vereist"),
            &Capability::unavailable("Schakel Apple Intelligence in."),
        );
        let json = serde_json::to_value(kinds).unwrap();
        let kinds = json.as_array().unwrap();
        let mut actual: Vec<&str> = kinds
            .iter()
            .map(|kind| kind["kind"].as_str().unwrap())
            .collect();
        actual.sort_unstable();
        assert_eq!(
            actual,
            [
                "apple_intelligence",
                "apple_speech",
                "claude_code",
                "codex",
                "copilot",
                "cursor_agent",
                "custom",
                "gemini_cli",
                "groq",
                "lm_studio",
                "local",
                "mistral",
                "ollama",
                "open_ai",
                "open_code",
                "open_router"
            ]
        );
        for cli in kinds.iter().filter(|kind| kind["is_cli_agent"] == true) {
            assert_eq!(cli["requires_key"], false);
            assert_eq!(cli["stt"], false);
            assert_eq!(cli["llm"], true);
        }
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| kind["is_cli_agent"] == true)
                .count(),
            6
        );
        let apple = kinds
            .iter()
            .find(|kind| kind["kind"] == "apple_speech")
            .unwrap();
        assert_eq!(apple["stt"], true);
        assert_eq!(apple["llm"], false);
        assert_eq!(apple["availability"]["status"], "unsupported");
        let intelligence = kinds
            .iter()
            .find(|kind| kind["kind"] == "apple_intelligence")
            .unwrap();
        assert_eq!(intelligence["stt"], false);
        assert_eq!(intelligence["llm"], true);
        assert_eq!(intelligence["requires_key"], false);
        assert_eq!(intelligence["default_llm_model"], "system");
        assert_eq!(intelligence["availability"]["status"], "unavailable");
    }

    #[test]
    fn meeting_identifiers_cannot_address_another_folder_on_either_os() {
        for id in [
            "",
            ".",
            "..",
            "../ander",
            "ander/meeting",
            "ander\\meeting",
            "/tmp/meeting",
            "C:\\meeting",
            "a:b",
            "meeting\nnaam",
            "meeting\0naam",
        ] {
            assert!(validate_meeting_id(id).is_err(), "accepted {id:?}");
        }
        assert!(validate_meeting_id("2026-10-24 0900 Klantgesprek (2)").is_ok());
    }

    #[test]
    fn missing_archives_are_explicit_in_the_meeting_contract() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("ik.m4a"), b"fixture").unwrap();
        let tracks = serde_json::to_value(audio_tracks(directory.path())).unwrap();
        assert_eq!(
            tracks,
            serde_json::json!([
                {"speaker":"me","available":true},
                {"speaker":"others","available":false}
            ])
        );
    }

    #[test]
    fn chat_history_is_data_with_a_stable_wire_contract() {
        let history: Vec<ChatTurn> = serde_json::from_value(serde_json::json!([
            {"question":"Wat is afgesproken?","answer":"Donderdag [1]."}
        ]))
        .unwrap();
        assert_eq!(history[0].question, "Wat is afgesproken?");
        assert_eq!(history[0].answer, "Donderdag [1].");
    }

    #[test]
    fn finalizing_keeps_the_meeting_and_saved_notes_visible_without_a_recorder() {
        let root = tempfile::tempdir().unwrap();
        let data_dir = root.path().join("data");
        let library = root.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        let engine =
            pennel_core::PennelEngine::new(data_dir.to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.meetings_dir = library.to_string_lossy().into_owned();
        engine.update_settings(settings).unwrap();
        let session = engine
            .clone()
            .start_meeting(
                "Klantgesprek".into(),
                pennel_core::ApiKeys {
                    by_provider: HashMap::new(),
                },
            )
            .unwrap();
        session.set_notes("Mijn eigen notities.".into()).unwrap();
        let mut lifecycle = pennel_core::RecordingLifecycle::default();
        let token = lifecycle.begin(None).unwrap();
        lifecycle.recording(&token).unwrap();
        lifecycle.stopping(&token).unwrap();
        lifecycle.finalizing(&token).unwrap();
        let (dictation, _receiver) = std::sync::mpsc::channel();
        let shared = Shared {
            engine,
            version: "0.2.0-test".into(),
            preview: true,
            migration_warning: None,
            data_dir,
            dictation,
            secrets: crate::secrets::Secrets::default(),
            hotkey: std::sync::Mutex::new(None),
            hotkey_error: std::sync::Mutex::new(None),
            meeting: std::sync::Mutex::new(None),
            recording: std::sync::Mutex::new(lifecycle),
            recording_cancel: std::sync::atomic::AtomicBool::new(false),
            library_jobs: std::sync::atomic::AtomicUsize::new(0),
            updating: std::sync::atomic::AtomicBool::new(false),
            quitting: std::sync::atomic::AtomicBool::new(false),
            dictation_active: std::sync::atomic::AtomicBool::new(false),
            dictation_jobs: std::sync::atomic::AtomicUsize::new(0),
            paste_jobs: std::sync::atomic::AtomicUsize::new(0),
            meeting_context: std::sync::Mutex::new(Some(crate::meeting_runtime::MeetingContext {
                token,
                session: session.clone(),
                title: "Klantgesprek".into(),
                started: std::time::Instant::now(),
                duration_secs: Some(42),
                system_audio: true,
                heard_system_audio: true,
                audio_warning: Some("Systeemgeluid is onderbroken.".into()),
                consent_copied: true,
            })),
        };
        let snapshot = status(&shared);
        assert!(snapshot.busy);
        assert!(!snapshot.recording);
        assert_eq!(snapshot.phase, pennel_core::RecordingPhase::Finalizing);
        assert_eq!(snapshot.id.as_deref(), Some(session.id().as_str()));
        assert_eq!(snapshot.title.as_deref(), Some("Klantgesprek"));
        assert_eq!(snapshot.notes, "Mijn eigen notities.");
        assert_eq!(snapshot.elapsed_secs, 42);
        assert_eq!(
            snapshot.audio_warning.as_deref(),
            Some("Systeemgeluid is onderbroken.")
        );
        assert!(snapshot.system_audio);
        assert!(snapshot.consent_copied);
        assert!(require_inactive_meeting(&shared, &session.id()).is_err());
        session.stop().unwrap();
    }
}
