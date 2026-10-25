use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
#[cfg(not(target_os = "macos"))]
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

use crate::capabilities::{PermissionInfo, PermissionKind};
use crate::state::Shared;

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<tauri_plugin_updater::Update>>,
    pub installing: std::sync::atomic::AtomicBool,
}

#[tauri::command]
pub fn get_permissions() -> Vec<PermissionInfo> {
    crate::capabilities::permissions()
}

#[tauri::command]
pub async fn test_apple_intelligence() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(|| {
            crate::macos::language_model_complete(
                vec![
                    pennel_core::LanguageModelMessage {
                        role: "system".into(),
                        content: "Correct punctuation. Return only the corrected sentence.".into(),
                    },
                    pennel_core::LanguageModelMessage {
                        role: "user".into(),
                        content: "hello pennel".into(),
                    },
                ],
                30,
            )
            .map(|_| ())
        })
        .await
        .map_err(|_| "De modeltest is onderbroken.".to_owned())?
    }
    #[cfg(not(target_os = "macos"))]
    Err(
        "Apple Intelligence is alleen beschikbaar op een geschikte Mac met macOS 26 of nieuwer."
            .into(),
    )
}

#[tauri::command]
pub async fn request_permission(
    kind: PermissionKind,
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
) -> Result<Vec<PermissionInfo>, String> {
    let shared = shared.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            crate::macos::request_permission(kind.as_str())?;
            if kind == PermissionKind::Accessibility {
                crate::refresh_hotkey(&app, &shared);
            }
            Ok(crate::capabilities::permissions())
        }
        #[cfg(windows)]
        {
            let _ = shared;
            if kind != PermissionKind::Microphone {
                return Err("Deze toestemming wordt op Windows niet door Pennel gevraagd".into());
            }
            app.opener()
                .open_url("ms-settings:privacy-microphone", None::<&str>)
                .map_err(|error| error.to_string())?;
            Ok(crate::capabilities::permissions())
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = (kind, app, shared);
            Err("Beheer microfoontoegang in de instellingen van je desktopomgeving".into())
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn calendar_snapshot() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(crate::call_detection::calendar)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn play_meeting_audio(
    id: String,
    speaker: String,
    start_ms: u64,
    shared: State<'_, Arc<Shared>>,
) -> Result<(), String> {
    let shared = shared.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _job = crate::meeting_runtime::LibraryJob::new(&shared)?;
        let dir = crate::commands::checked_meeting_dir(&shared.engine, &id)?;
        let filename = match speaker.as_str() {
            "me" => "ik.m4a",
            "others" => "anderen.m4a",
            _ => return Err("Onbekend audiospoor".into()),
        };
        let path = dir
            .join(filename)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !path.starts_with(&dir) {
            return Err("Audiospoor valt buiten de meetingmap".into());
        }
        crate::platform_audio::play(&path, start_ms)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn stop_audio() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(crate::platform_audio::stop)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn copy_text(text: String, html: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::platform_clipboard::copy(&text, html.as_deref())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Copies text that must not reach clipboard history (the recovery code). Never logged.
#[tauri::command]
pub async fn copy_secret_text(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::platform_clipboard::copy_secret(&text))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn show_window(kind: String, tab: Option<String>, app: AppHandle) -> Result<(), String> {
    if !matches!(
        kind.as_str(),
        "library"
            | "workspace"
            | "dictation"
            | "meetings"
            | "settings"
            | "history"
            | "live"
            | "recording"
            | "onboarding"
    ) {
        return Err("Onbekend venster".into());
    }
    crate::show_window(&app, &kind, tab.as_deref());
    Ok(())
}

#[tauri::command]
pub fn show_workspace(
    route: crate::workspace::WorkspaceRoute,
    app: AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    if route
        .meeting_id
        .as_deref()
        .is_some_and(|id| !crate::workspace::valid_meeting_id(id))
    {
        return Err("Ongeldige meeting".into());
    }
    let kind = route.window_kind();
    crate::workspace::request(&app, crate::workspace::window_spec(kind).label, Some(route));
    crate::show_window(&app, kind, None);
    if window.label() == "onboarding" {
        crate::workspace::cancel_presentation(&app, window.label());
        crate::owned_dictation::close_owner(&app, window.label());
        let _ = window.hide();
        crate::notify_visibility(&app, window.label(), false);
    }
    Ok(())
}

#[tauri::command]
pub fn dictation_state(app: AppHandle) -> crate::pill::PillSnapshot {
    crate::pill::content_snapshot(&app)
}

#[tauri::command]
pub fn frontend_ready(app: AppHandle, window: tauri::WebviewWindow) {
    crate::frontend_ready(&app, window.label());
}

#[tauri::command]
pub fn frontend_flushed(
    request_id: u64,
    success: bool,
    error: Option<String>,
    app: AppHandle,
    window: tauri::WebviewWindow,
) {
    let _ = error;
    crate::frontend_flushed(&app, window.label(), request_id, success);
}

#[derive(Serialize)]
pub struct MarkedLine {
    text: String,
    mine: bool,
}

#[tauri::command]
pub fn mark_lines(enhanced: String, notes: String) -> Vec<MarkedLine> {
    pennel_core::mark_lines(&enhanced, &notes)
        .into_iter()
        .map(|line| MarkedLine {
            text: line.text,
            mine: line.mine,
        })
        .collect()
}

#[tauri::command]
pub async fn choose_meetings_dir(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
) -> Result<Option<String>, String> {
    if shared.preview {
        return Err("De preview gebruikt uitsluitend zijn eigen meetingmap".into());
    }
    if shared.recording.lock().unwrap().is_busy() {
        return Err("Rond eerst de opname af voordat je een andere map kiest".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Kies de meetingmap")
            .blocking_pick_folder()
            .map(|file| {
                file.into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|error| error.to_string())
            })
            .transpose()
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn toggle_dictation(copy_only: bool, shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    shared
        .dictation
        .send(crate::dictation::Msg::ManualToggle { copy_only })
        .map_err(|_| "Dicteren is niet beschikbaar".into())
}

#[tauri::command]
pub async fn share_meeting(
    id: String,
    include_transcript: bool,
    action: String,
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
) -> Result<(), String> {
    if !matches!(action.as_str(), "mail" | "share") {
        return Err("Onbekende deelactie".into());
    }
    let shared = shared.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _job = crate::meeting_runtime::LibraryJob::new(&shared)?;
        crate::commands::checked_meeting_dir(&shared.engine, &id)?;
        let export = shared
            .engine
            .export_meeting(id, include_transcript)
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        {
            let _ = app;
            crate::macos::share(&export.markdown, &export.title, &action)
        }
        #[cfg(not(target_os = "macos"))]
        {
            if action == "share" {
                return Err(
                    "Kopieer de opgemaakte notities of open de meetingmap om deze te delen".into(),
                );
            }
            let mut url = tauri::Url::parse("mailto:").map_err(|error| error.to_string())?;
            url.query_pairs_mut()
                .append_pair("subject", &export.title)
                .append_pair("body", &export.markdown);
            app.opener()
                .open_url(url.to_string(), None::<&str>)
                .map_err(|error| error.to_string())
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn record_prompt(app: AppHandle) -> Option<crate::call_detection::RecordPrompt> {
    app.state::<crate::call_detection::DetectionState>()
        .prompt
        .lock()
        .unwrap()
        .clone()
}

#[tauri::command]
pub async fn answer_record_prompt(
    answer: String,
    prompt_id: u64,
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
) -> Result<(), String> {
    let shared = shared.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::call_detection::answer(&app, &shared, &answer, prompt_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Serialize)]
pub struct UpdateInfo {
    available: bool,
    version: Option<String>,
    body: Option<String>,
}

fn updates_enabled(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|config| config.get("pubkey"))
        .and_then(|value| value.as_str())
        .is_some_and(|key| !key.is_empty())
}

pub fn spawn_update_checks(app: &AppHandle) {
    if !updates_enabled(app) {
        return;
    }
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            if let Ok(info) = tauri::async_runtime::block_on(check_for_updates(app.clone()))
                && info.available
            {
                let _ = app.emit("update-available", &info);
            }
            std::thread::sleep(std::time::Duration::from_secs(24 * 60 * 60));
        }
    });
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateInfo, String> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return Err("Werk dit pakket bij via je pakketbeheerder. Automatische updates zijn beschikbaar in de AppImage-versie.".into());
    }
    if !updates_enabled(&app) {
        return Err("Updates zijn beschikbaar in de ondertekende release. Deze preview wordt niet automatisch bijgewerkt.".into());
    }
    let update = app
        .updater_builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    let result = UpdateInfo {
        available: update.is_some(),
        version: update.as_ref().map(|update| update.version.clone()),
        body: update.as_ref().and_then(|update| update.body.clone()),
    };
    *app.state::<UpdateState>().pending.lock().unwrap() = update;
    Ok(result)
}

#[tauri::command]
pub async fn install_update(app: AppHandle, shared: State<'_, Arc<Shared>>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    struct UpdateGuard {
        shared: Arc<Shared>,
        app: AppHandle,
        request_id: Option<u64>,
    }
    impl Drop for UpdateGuard {
        fn drop(&mut self) {
            self.shared.updating.store(false, Ordering::SeqCst);
            self.app
                .state::<UpdateState>()
                .installing
                .store(false, Ordering::SeqCst);
            if let Some(request_id) = self.request_id {
                let _ = self.app.emit("update-cancelled", request_id);
            }
        }
    }
    let shared = shared.inner().clone();
    let quit_attempt = app
        .state::<crate::ExitState>()
        .quit_attempts
        .load(Ordering::SeqCst);
    let mut guard = {
        let recording = shared.recording.lock().unwrap();
        if shared.updating.swap(true, Ordering::SeqCst) {
            return Err("Pennel wordt al bijgewerkt".into());
        }
        let guard = UpdateGuard {
            shared: shared.clone(),
            app: app.clone(),
            request_id: None,
        };
        if shared.quitting.load(Ordering::SeqCst)
            || recording.is_busy()
            || shared.dictation_active.load(Ordering::SeqCst)
            || shared.dictation_jobs.load(Ordering::SeqCst) != 0
            || shared.paste_jobs.load(Ordering::SeqCst) != 0
            || shared.library_jobs.load(Ordering::SeqCst) != 0
        {
            return Err(
                "Wacht tot opnemen, verwerken en plakken klaar zijn voordat je Pennel bijwerkt"
                    .into(),
            );
        }
        guard
    };
    let update = app
        .state::<UpdateState>()
        .pending
        .lock()
        .unwrap()
        .take()
        .ok_or("Zoek eerst naar updates")?;
    let flush_app = app.clone();
    let flush = tauri::async_runtime::spawn_blocking(move || {
        crate::flush_frontends(&flush_app, "prepare-update")
    })
    .await
    .map_err(|error| error.to_string())?;
    guard.request_id = Some(flush.request_id);
    flush.result?;
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    if shared.quitting.load(Ordering::SeqCst)
        || app
            .state::<crate::ExitState>()
            .quit_attempts
            .load(Ordering::SeqCst)
            != quit_attempt
    {
        return Err("Pennel wordt afgesloten".into());
    }
    let flush_app = app.clone();
    let flush = tauri::async_runtime::spawn_blocking(move || {
        crate::flush_frontends(&flush_app, "prepare-update")
    })
    .await
    .map_err(|error| error.to_string())?;
    guard.request_id = Some(flush.request_id);
    flush.result?;
    {
        let exit = app.state::<crate::ExitState>();
        let _transition = exit.transition_gate.lock().unwrap();
        if shared.quitting.load(Ordering::SeqCst)
            || exit.quit_attempts.load(Ordering::SeqCst) != quit_attempt
        {
            return Err("Bijwerken is geannuleerd omdat Pennel wordt afgesloten".into());
        }
        app.state::<UpdateState>()
            .installing
            .store(true, Ordering::SeqCst);
    }
    update.install(bytes).map_err(|error| error.to_string())?;
    app.restart();
}
