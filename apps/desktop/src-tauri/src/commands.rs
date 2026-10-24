//! The webview's API. Blocking work (network, model loading) runs off the main thread.

use std::collections::HashMap;
use std::sync::Arc;

use pennel_core::{
    HistoryEntry, LocalModel, MeetingInfo, MeetingTemplate, ModelState, ProviderKind, ProviderTask,
    RemoteModel, Segment, Settings, SpeakerName,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::diagnostics::{self, Log};
use crate::dictation::Msg;
use crate::meeting::MeetingRecorder;
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
}

const KINDS: [(ProviderKind, &str); 8] = [
    (ProviderKind::OpenAi, "OpenAI"),
    (ProviderKind::OpenRouter, "OpenRouter"),
    (ProviderKind::Groq, "Groq"),
    (ProviderKind::Mistral, "Mistral"),
    (ProviderKind::Ollama, "Ollama (lokaal)"),
    (ProviderKind::LmStudio, "LM Studio (lokaal)"),
    (ProviderKind::Custom, "Eigen server"),
    (ProviderKind::Local, "Lokaal op deze computer"),
];

#[derive(Serialize)]
pub struct AppInfo {
    settings: Settings,
    keys: HashMap<String, bool>,
    kinds: Vec<KindInfo>,
    platform: &'static str,
    version: &'static str,
    hotkey_error: Option<String>,
    log_path: Option<String>,
    data_dir: String,
}

fn info(shared: &Shared) -> AppInfo {
    let settings = shared.settings();
    let keys = settings
        .providers
        .iter()
        .map(|p| (p.id.clone(), shared.secrets.get(&p.id).is_some()))
        .collect();
    AppInfo {
        keys,
        settings,
        kinds: KINDS
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
            })
            .collect(),
        platform: std::env::consts::OS,
        version: env!("CARGO_PKG_VERSION"),
        hotkey_error: shared.hotkey_error.lock().unwrap().clone(),
        log_path: diagnostics::file_path().map(|p| p.to_string_lossy().into_owned()),
        data_dir: shared.data_dir.to_string_lossy().into_owned(),
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
    let previous = shared.settings().hotkey;
    shared.engine.update_settings(settings).map_err(err)?;
    let hotkey = shared.settings().hotkey;
    if hotkey != previous {
        if let Some(listener) = shared.hotkey.lock().unwrap().as_ref() {
            listener.set_trigger(hotkey);
        }
        let _ = shared.dictation.send(Msg::TriggerChanged);
        if let Some(hint) = app.try_state::<crate::TrayHint>() {
            let _ = hint.0.set_text(crate::hotkey_hint(&shared));
        }
        LOG.info(format!("sneltoets gewijzigd naar {hotkey:?}"));
    }
    Ok(info(&shared))
}

#[tauri::command]
pub fn add_provider(kind: ProviderKind, shared: State<'_, Arc<Shared>>) -> Result<AppInfo> {
    let mut settings = shared.settings();
    let account = pennel_core::new_account(kind, settings.providers.clone());
    settings.providers.push(account);
    shared.engine.update_settings(settings).map_err(err)?;
    Ok(info(&shared))
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
pub fn open_path(path: String, app: AppHandle) -> Result<()> {
    app.opener().open_path(path, None::<&str>).map_err(err)
}

#[derive(Serialize)]
pub struct MeetingStatus {
    recording: bool,
    id: Option<String>,
    title: Option<String>,
    elapsed_secs: u64,
    system_audio: bool,
    heard_system_audio: bool,
    pending: u32,
    transcript: Vec<Segment>,
}

#[tauri::command]
pub fn meeting_status(shared: State<'_, Arc<Shared>>) -> MeetingStatus {
    let meeting = shared.meeting.lock().unwrap();
    match meeting.as_ref() {
        Some(m) => MeetingStatus {
            recording: true,
            id: Some(m.session.id()),
            title: Some(m.title.clone()),
            elapsed_secs: m.elapsed_secs(),
            system_audio: m.has_system_audio(),
            heard_system_audio: m.heard_system_audio(),
            pending: m.session.pending(),
            transcript: m.session.transcript(),
        },
        None => MeetingStatus {
            recording: false,
            id: None,
            title: None,
            elapsed_secs: 0,
            system_audio: false,
            heard_system_audio: false,
            pending: 0,
            transcript: Vec::new(),
        },
    }
}

#[tauri::command]
pub fn start_meeting(title: String, shared: State<'_, Arc<Shared>>) -> Result<String> {
    let mut meeting = shared.meeting.lock().unwrap();
    if let Some(m) = meeting.as_ref() {
        return Ok(m.session.id());
    }
    let title = if title.trim().is_empty() {
        "Meeting".to_owned()
    } else {
        title.trim().to_owned()
    };
    let session = shared
        .engine
        .clone()
        .start_meeting(title.clone(), shared.keys())
        .map_err(err)?;
    let id = session.id();
    *meeting = Some(MeetingRecorder::start(session, title)?);
    Ok(id)
}

#[tauri::command]
pub fn stop_meeting(app: AppHandle, shared: State<'_, Arc<Shared>>) -> Result<()> {
    let Some(recorder) = shared.meeting.lock().unwrap().take() else {
        return Ok(());
    };
    std::thread::spawn(move || {
        let result = recorder.stop();
        if let Err(e) = &result {
            LOG.error(format!("meeting stoppen: {e}"));
        }
        let _ = app.emit("meeting-stopped", result.ok());
    });
    Ok(())
}

#[tauri::command]
pub fn set_meeting_notes(text: String, shared: State<'_, Arc<Shared>>) -> Result<()> {
    match shared.meeting.lock().unwrap().as_ref() {
        Some(m) => m.session.set_notes(text).map_err(err),
        None => Err("geen meeting bezig".into()),
    }
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
}

#[tauri::command]
pub fn meeting_detail(id: String, shared: State<'_, Arc<Shared>>) -> MeetingDetail {
    let engine = &shared.engine;
    MeetingDetail {
        info: engine.meeting_info(id.clone()),
        notes: engine.meeting_notes(id.clone()),
        summary: engine.meeting_markdown(id.clone()),
        transcript: engine.meeting_transcript(id.clone()),
        speakers: engine.meeting_speakers(id.clone()),
        dir: engine.meeting_dir(id),
    }
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
        let keys = shared.keys();
        shared.engine.retranscribe_failed(id, keys).map_err(err)
    })
    .await
}

#[tauri::command]
pub fn rename_meeting(id: String, title: String, shared: State<'_, Arc<Shared>>) -> Result<String> {
    shared.engine.rename_meeting(id, title).map_err(err)
}

#[tauri::command]
pub fn delete_meeting(id: String, shared: State<'_, Arc<Shared>>) -> Result<()> {
    shared.engine.delete_meeting(id).map_err(err)
}

#[tauri::command]
pub fn show_main(app: AppHandle) {
    crate::show_main(&app, None);
}

#[tauri::command]
pub fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

pub fn emit_navigate(app: &AppHandle, tab: &str) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit_to("main", "navigate", tab);
    }
}
