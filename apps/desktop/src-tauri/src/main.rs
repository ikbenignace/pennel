#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod call_detection;
mod capabilities;
#[cfg(not(target_os = "macos"))]
mod clipboard;
#[cfg(not(target_os = "macos"))]
mod clipboard_system;
mod cloud;
mod commands;
mod diagnostics;
mod dictation;
mod foreground;
mod gesture;
mod hotkey;
mod keys;
#[cfg(any(target_os = "macos", test))]
mod legacy_migration;
#[cfg(target_os = "linux")]
mod linux_input;
#[cfg(target_os = "linux")]
mod linux_meetings;
#[cfg(target_os = "macos")]
mod macos;
mod meeting;
mod meeting_runtime;
mod paste_queue;
mod pill;
mod platform_audio;
mod platform_clipboard;
mod platform_commands;
mod record_shortcut;
mod resample;
mod secrets;
mod state;
mod status;
#[cfg(windows)]
mod windows_meetings;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pennel_core::{PennelEngine, ProviderKind};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use crate::diagnostics::Log;
use crate::hotkey::Listener;
use crate::state::Shared;

const LOG: Log = Log::new("app");

/// Models are large, so everything lives in the local (non-roaming) data folder.
/// On a Mac (development only) it stays apart from the native app's "Pennel" folder.
fn is_preview(app: &AppHandle) -> bool {
    cfg!(debug_assertions) || app.config().identifier.ends_with(".preview")
}

fn data_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    let name = if is_preview(app) {
        "Pennel Preview"
    } else {
        "Pennel"
    };
    app.path().local_data_dir().map(|dir| dir.join(name))
}

pub fn show_main(app: &AppHandle, tab: Option<&str>) {
    let kind = match tab {
        Some("meeting") => "live",
        Some("history") => "history",
        Some("providers" | "models" | "dictation" | "dictionary" | "styles") => "settings",
        _ => "library",
    };
    show_window(app, kind, tab);
}

pub fn show_window(app: &AppHandle, kind: &str, tab: Option<&str>) {
    let (label, mode, width, height) = match kind {
        "settings" => ("settings", "settings", 720.0, 660.0),
        "history" => ("history", "history", 760.0, 580.0),
        "live" => ("live", "live", 920.0, 700.0),
        "onboarding" => ("onboarding", "onboarding", 620.0, 560.0),
        "prompt" => ("record-prompt", "prompt", 420.0, 210.0),
        _ => ("main", "library", 920.0, 660.0),
    };
    let app = app.clone();
    let handle = app.clone();
    let tab = tab.map(str::to_owned);
    let _ = app.run_on_main_thread(move || {
        let window = handle.get_webview_window(label).or_else(|| {
            let url = format!(
                "index.html?window={mode}{}",
                tab.as_ref()
                    .map(|tab| format!("&tab={tab}"))
                    .unwrap_or_default()
            );
            let mut builder = tauri::WebviewWindowBuilder::new(
                &handle,
                label,
                tauri::WebviewUrl::App(url.into()),
            )
            .title("Pennel")
            .inner_size(width, height)
            .visible(false);
            if mode == "prompt" {
                builder = builder
                    .decorations(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .focused(false)
                    .focusable(false)
                    .resizable(false);
            }
            builder.build().ok()
        });
        if let Some(window) = window {
            if let Some(tab) = tab {
                let _ = window.emit_to(label, "navigate", tab);
            }
            let _ = window.unminimize();
            if mode == "prompt" {
                #[cfg(target_os = "macos")]
                if let Ok(pointer) = window.ns_window() {
                    unsafe {
                        macos::configure_panel(pointer, true);
                        macos::show_panel(pointer);
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    crate::pill::show_without_focus(&window);
                }
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

fn hotkey_label(trigger: pennel_core::HotkeyTrigger) -> &'static str {
    use pennel_core::HotkeyTrigger::*;
    match trigger {
        Fn if cfg!(target_os = "macos") => "fn",
        RightControl | Fn => "rechter Ctrl",
        RightOption if cfg!(target_os = "macos") => "rechter Option",
        RightCommand if cfg!(target_os = "macos") => "rechter Command",
        RightOption => "rechter Alt",
        RightCommand => "rechter Windows-toets",
        ControlSuper => "Ctrl + Windows-toets",
    }
}

pub struct TrayHint(pub MenuItem<tauri::Wry>);

#[derive(Default)]
struct ExitSync {
    sequence: u64,
    ready: std::collections::HashSet<String>,
    pending: std::collections::HashSet<String>,
    failed: bool,
}

#[derive(Default)]
pub struct ExitState {
    pub requested: AtomicBool,
    pub quit_attempts: std::sync::atomic::AtomicU64,
    pub transition_gate: Mutex<()>,
    approved: AtomicBool,
    sync: Mutex<ExitSync>,
    flush_gate: Mutex<()>,
    changed: std::sync::Condvar,
}

pub fn frontend_ready(app: &AppHandle, label: &str) {
    app.state::<ExitState>()
        .sync
        .lock()
        .unwrap()
        .ready
        .insert(label.to_owned());
}

pub fn frontend_flushed(app: &AppHandle, label: &str, sequence: u64, success: bool) {
    let state = app.state::<ExitState>();
    let mut sync = state.sync.lock().unwrap();
    if sync.sequence == sequence && sync.pending.remove(label) {
        sync.failed |= !success;
        state.changed.notify_all();
    }
}

pub struct FrontendFlush {
    pub request_id: u64,
    pub result: Result<(), String>,
}

pub fn flush_frontends(app: &AppHandle, event: &str) -> FrontendFlush {
    let state = app.state::<ExitState>();
    let _operation = state.flush_gate.lock().unwrap();
    let windows = app.webview_windows();
    let (sequence, labels) = {
        let mut sync = state.sync.lock().unwrap();
        sync.sequence = sync.sequence.wrapping_add(1);
        sync.failed = false;
        sync.pending = sync
            .ready
            .iter()
            .filter(|label| windows.contains_key(*label))
            .cloned()
            .collect();
        (sync.sequence, sync.pending.clone())
    };
    for label in &labels {
        if app.emit_to(label, event, sequence).is_err() {
            return FrontendFlush {
                request_id: sequence,
                result: Err("Een venster kon de laatste wijzigingen niet bewaren".into()),
            };
        }
    }
    let sync = state.sync.lock().unwrap();
    let (sync, _) = state
        .changed
        .wait_timeout_while(sync, Duration::from_secs(5), |sync| {
            !sync.pending.is_empty() && !sync.failed
        })
        .unwrap();
    let result = if sync.failed || !sync.pending.is_empty() {
        Err(
            "Bewaren is nog niet klaar. Pennel blijft open zodat je wijzigingen bewaard blijven."
                .into(),
        )
    } else {
        Ok(())
    };
    FrontendFlush {
        request_id: sequence,
        result,
    }
}

pub fn clipboard_mode() -> bool {
    #[cfg(target_os = "linux")]
    return linux_input::is_wayland();
    #[cfg(not(target_os = "linux"))]
    false
}

fn dispatch_cli(app: &AppHandle, arguments: &[String]) -> bool {
    let Some(shared) = app.try_state::<Arc<Shared>>() else {
        return false;
    };
    if arguments
        .iter()
        .any(|argument| argument == "--toggle-dictation")
    {
        let _ = shared.dictation.send(dictation::Msg::ManualToggle {
            copy_only: clipboard_mode(),
        });
        true
    } else if arguments
        .iter()
        .any(|argument| argument == "--toggle-meeting")
    {
        let app = app.clone();
        let shared = shared.inner().clone();
        std::thread::spawn(move || call_detection::toggle_recording(app, shared));
        true
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
pub fn refresh_hotkey(app: &AppHandle, shared: &Arc<Shared>) {
    let active = shared.hotkey.lock().unwrap().is_some();
    if active
        || macos::permissions()
            .get("accessibility")
            .and_then(|value| value.as_str())
            != Some("granted")
    {
        return;
    }
    static RETRYING: AtomicBool = AtomicBool::new(false);
    if RETRYING.swap(true, Ordering::SeqCst) {
        return;
    }
    let (sender, receiver) = mpsc::channel();
    match Listener::start(shared.settings().hotkey, sender) {
        Ok(listener) => {
            let _ = listener.set_record_shortcut_enabled(shared.settings().record_shortcut_enabled);
            let previous = shared.hotkey.lock().unwrap().replace(listener);
            drop(previous);
            record_shortcut::refresh(app, shared);
            *shared.hotkey_error.lock().unwrap() = None;
            let tx = shared.dictation.clone();
            std::thread::spawn(move || {
                for event in receiver {
                    if tx.send(dictation::Msg::Hotkey(event)).is_err() {
                        break;
                    }
                }
            });
            if let Some(hint) = app.try_state::<TrayHint>() {
                let _ = hint.0.set_text(hotkey_hint(shared));
            }
        }
        Err(error) => *shared.hotkey_error.lock().unwrap() = Some(error),
    }
    RETRYING.store(false, Ordering::SeqCst);
}

pub fn hotkey_hint(shared: &Shared) -> String {
    match shared.hotkey_error.lock().unwrap().as_ref() {
        None => format!(
            "Dicteren — houd {} ingedrukt",
            hotkey_label(shared.settings().hotkey)
        ),
        Some(_) => "Sneltoets niet beschikbaar (zie logboek)".into(),
    }
}

fn build_tray(app: &AppHandle, shared: &Shared) -> tauri::Result<()> {
    let hint = MenuItem::with_id(app, "hint", hotkey_hint(shared), false, None::<&str>)?;
    app.manage(TrayHint(hint.clone()));
    let menu = Menu::with_items(
        app,
        &[
            &hint,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "open", "Open Pennel", true, None::<&str>)?,
            &MenuItem::with_id(
                app,
                "dictation",
                "Dicteren starten/stoppen",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(app, "meeting", "Meeting opnemen…", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Instellingen…", true, None::<&str>)?,
            &MenuItem::with_id(app, "history", "Geschiedenis…", true, None::<&str>)?,
            &MenuItem::with_id(app, "log", "Logboek openen", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Stop Pennel", true, None::<&str>)?,
        ],
    )?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id("pennel")
        .icon(icon)
        .tooltip("Pennel")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "dictation" => {
                if let Some(shared) = app.try_state::<Arc<Shared>>() {
                    let _ = shared.dictation.send(dictation::Msg::ManualToggle {
                        copy_only: clipboard_mode(),
                    });
                }
            }
            "open" => show_main(app, Some("meetings")),
            "meeting" => show_main(app, Some("meeting")),
            "settings" => show_main(app, Some("providers")),
            "history" => show_main(app, Some("history")),
            "log" => {
                let _ = commands::open_log_file(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn needs_setup(shared: &Shared) -> bool {
    let settings = shared.settings();
    let Some((account, model)) = settings.resolve(&settings.dictation_stt) else {
        return true;
    };
    if model.trim().is_empty()
        || !pennel_core::supports(account.kind, pennel_core::ProviderTask::DictationStt)
    {
        return true;
    }
    if account.kind == ProviderKind::AppleSpeech {
        #[cfg(target_os = "macos")]
        {
            let permissions = macos::permissions();
            if !permissions
                .get("apple_speech")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
                || permissions.get("speech").and_then(|value| value.as_str()) != Some("granted")
            {
                return true;
            }
        }
        #[cfg(not(target_os = "macos"))]
        return true;
    }
    if account.kind == ProviderKind::Local {
        return shared.engine.model_state(model) != pennel_core::ModelState::Downloaded;
    }
    pennel_core::requires_key(account.kind) && shared.secrets.get(&account.id).is_none()
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let dir = data_dir(&handle)?;
    let preview = is_preview(&handle);
    app.manage(ExitState::default());
    app.manage(call_detection::DetectionState::default());
    #[cfg(target_os = "macos")]
    {
        macos::set_accessory();
        if !preview && !macos::other_instances().is_empty() {
            return Err("Sluit de vorige Pennel-app voordat je deze versie opent. Je gegevens zijn niet gewijzigd.".into());
        }
        if !preview {
            legacy_migration::prepare(&dir, &handle.path().home_dir()?)?;
        }
    }
    let log = if cfg!(target_os = "macos") && !preview {
        handle
            .path()
            .home_dir()?
            .join("Library/Logs/Pennel/pennel.log")
    } else {
        dir.join("logs/pennel.log")
    };
    diagnostics::init(log);
    LOG.info(format!(
        "Pennel {} gestart op {}",
        handle.package_info().version,
        std::env::consts::OS
    ));
    let engine = if preview {
        PennelEngine::new_preview(dir.to_string_lossy().into_owned())?
    } else {
        PennelEngine::new(dir.to_string_lossy().into_owned())?
    };
    #[cfg(target_os = "macos")]
    {
        engine.set_system_transcriber(Arc::new(AppleTranscriber));
        engine.set_system_language_model(Arc::new(AppleLanguageModel));
    }
    let (tx, rx) = mpsc::channel();
    let secrets = Arc::new(secrets::Secrets::new(preview));
    engine.set_secret_store(secrets.clone());
    #[cfg(target_os = "macos")]
    let migration_warning = if preview {
        None
    } else {
        match legacy_migration::finish(&engine, &secrets, &dir, &handle.path().home_dir()?) {
            Ok(()) => None,
            Err(legacy_migration::MigrationError::Credentials(message)) => Some(message),
            Err(error) => return Err(error.into()),
        }
    };
    #[cfg(not(target_os = "macos"))]
    let migration_warning = None;
    let shared = Arc::new(Shared {
        engine,
        secrets,
        data_dir: dir,
        version: handle.package_info().version.to_string(),
        migration_warning,
        preview,
        dictation: tx.clone(),
        hotkey: Mutex::new(None),
        hotkey_error: Mutex::new(None),
        meeting: Mutex::new(None),
        recording: Mutex::new(pennel_core::RecordingLifecycle::default()),
        meeting_context: Mutex::new(None),
        recording_cancel: AtomicBool::new(false),
        library_jobs: std::sync::atomic::AtomicUsize::new(0),
        updating: AtomicBool::new(false),
        quitting: AtomicBool::new(false),
        dictation_active: AtomicBool::new(false),
        dictation_jobs: std::sync::atomic::AtomicUsize::new(0),
        paste_jobs: std::sync::atomic::AtomicUsize::new(0),
    });
    app.manage(shared.clone());
    app.manage(cloud::SyncRunner::start(shared.clone(), handle.clone()));
    app.manage(platform_commands::UpdateState::default());
    if handle
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|value| value.get("pubkey"))
        .and_then(|value| value.as_str())
        .is_some_and(|key| !key.is_empty())
    {
        handle.plugin(tauri_plugin_updater::Builder::new().build())?;
    }

    app.manage(pill::Snapshot::default());
    dictation::spawn(handle.clone(), shared.clone(), rx, tx.clone());

    let (hotkey_tx, hotkey_rx) = mpsc::channel();
    let settings = shared.settings();
    match Listener::start(settings.hotkey, hotkey_tx) {
        Ok(listener) => *shared.hotkey.lock().unwrap() = Some(listener),
        Err(e) => {
            LOG.error(format!("sneltoets niet actief: {e}"));
            *shared.hotkey_error.lock().unwrap() = Some(e);
        }
    }
    std::thread::spawn(move || {
        for event in hotkey_rx {
            if tx.send(dictation::Msg::Hotkey(event)).is_err() {
                return;
            }
        }
    });
    LOG.info(format!(
        "sneltoets actief={} · dicteren {}/{} · opkuis {}/{} · taal {} · modus {:?}",
        shared.hotkey_error.lock().unwrap().is_none(),
        settings.dictation_stt.provider_id,
        settings.dictation_stt.model,
        settings.llm.provider_id,
        settings.llm.model,
        settings.language.as_deref().unwrap_or("automatisch"),
        settings.mode
    ));

    let idle_engine = shared.engine.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            if idle_engine.unload_idle_models() {
                Log::new("app").info("lokaal model uit geheugen gehaald");
            }
        }
    });

    build_tray(&handle, &shared)?;
    record_shortcut::install(&handle)?;
    record_shortcut::refresh(&handle, &shared);
    call_detection::spawn(handle.clone(), shared.clone());
    capabilities::spawn_probe(&handle);
    platform_commands::spawn_update_checks(&handle);
    if shared.migration_warning.is_none() {
        let retained = shared.clone();
        std::thread::spawn(move || {
            if let Ok(_job) = meeting_runtime::LibraryJob::new(&retained) {
                let count = retained.engine.prune_meeting_audio();
                if count > 0 {
                    LOG.info(format!("Audio-retentie toegepast: {count} bestanden"));
                }
            }
        });
    }
    dispatch_cli(&handle, &std::env::args().collect::<Vec<_>>());
    if needs_setup(&shared) {
        show_window(&handle, "onboarding", None);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
struct AppleTranscriber;

#[cfg(target_os = "macos")]
struct AppleLanguageModel;
#[cfg(target_os = "macos")]
impl pennel_core::SystemLanguageModel for AppleLanguageModel {
    fn complete(
        &self,
        messages: Vec<pennel_core::LanguageModelMessage>,
        timeout_secs: u64,
    ) -> Result<String, pennel_core::PennelError> {
        macos::language_model_complete(messages, timeout_secs)
            .map_err(|message| pennel_core::PennelError::InvalidResponse { message })
    }
}

#[cfg(target_os = "macos")]
impl pennel_core::SystemTranscriber for AppleTranscriber {
    fn transcribe(
        &self,
        samples: Vec<f32>,
        sample_rate: u32,
        language: Option<String>,
    ) -> Result<String, pennel_core::PennelError> {
        macos::transcribe(&samples, sample_rate, language.as_deref())
            .map_err(|message| pennel_core::PennelError::InvalidResponse { message })
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !dispatch_cli(app, &args) {
                show_main(app, None);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && matches!(
                    window.label(),
                    "main" | "live" | "settings" | "history" | "onboarding"
                )
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::save_settings,
            commands::add_provider,
            commands::remove_provider,
            commands::set_key,
            commands::provider_models,
            commands::local_models,
            commands::download_model,
            commands::cancel_download,
            commands::delete_model,
            commands::history,
            commands::retry_history,
            commands::delete_history,
            commands::open_log,
            commands::open_path,
            commands::meeting_status,
            commands::start_meeting,
            commands::stop_meeting,
            commands::set_meeting_notes,
            commands::set_meeting_title,
            commands::meetings,
            commands::meeting_detail,
            commands::meeting_templates,
            commands::summarize_meeting,
            commands::retranscribe_meeting,
            commands::rename_meeting,
            commands::delete_meeting,
            commands::meeting_conflict_files,
            commands::delete_conflict_copy,
            commands::search_library,
            commands::ask_meetings,
            commands::template_source,
            commands::save_template,
            commands::delete_template,
            commands::rename_speaker,
            commands::diarize_meeting,
            commands::export_meeting,
            commands::save_meeting_notes,
            commands::set_meeting_attendees,
            commands::detect_cli_agents,
            commands::check_cli_agent,
            platform_commands::get_permissions,
            platform_commands::test_apple_intelligence,
            platform_commands::request_permission,
            platform_commands::calendar_snapshot,
            platform_commands::play_meeting_audio,
            platform_commands::stop_audio,
            platform_commands::copy_text,
            platform_commands::copy_secret_text,
            platform_commands::show_window,
            platform_commands::record_prompt,
            platform_commands::answer_record_prompt,
            platform_commands::check_for_updates,
            platform_commands::install_update,
            platform_commands::toggle_dictation,
            platform_commands::dictation_state,
            platform_commands::frontend_ready,
            platform_commands::frontend_flushed,
            platform_commands::mark_lines,
            platform_commands::share_meeting,
            platform_commands::choose_meetings_dir,
            commands::model_state,
            commands::show_main,
            commands::hide_window,
            cloud::cloud_login_start,
            cloud::cloud_login_poll,
            cloud::cloud_logout,
            cloud::cloud_account,
            cloud::cloud_devices,
            cloud::cloud_rename_device,
            cloud::cloud_revoke_device,
            cloud::cloud_delete_account,
            cloud::cloud_sync_enable,
            cloud::cloud_sync_unlock,
            cloud::cloud_sync_new_recovery_code,
            cloud::cloud_sync_reset,
            cloud::cloud_sync_disable,
            cloud::cloud_sync_now,
            cloud::cloud_sync_last,
            cloud::cloud_sync_confirm_deletes,
            cloud::meeting_sync_states,
            cloud::cloud_folder_warning,
            cloud::open_url,
        ])
        .build(tauri::generate_context!())
        .expect("Pennel kon niet starten")
        .run(|_app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                if _app.state::<ExitState>().approved.load(Ordering::SeqCst) {
                    return;
                }
                api.prevent_exit();
                let exit_state = _app.state::<ExitState>();
                let transition = exit_state.transition_gate.lock().unwrap();
                if _app
                    .try_state::<platform_commands::UpdateState>()
                    .is_some_and(|update| update.installing.load(Ordering::SeqCst))
                {
                    return;
                }
                if _app
                    .state::<ExitState>()
                    .requested
                    .swap(true, Ordering::SeqCst)
                {
                    return;
                }
                _app.state::<ExitState>()
                    .quit_attempts
                    .fetch_add(1, Ordering::SeqCst);
                drop(transition);
                let app = _app.clone();
                std::thread::spawn(move || {
                    let shared = app.state::<Arc<Shared>>().inner().clone();
                    {
                        let _recording = shared.recording.lock().unwrap();
                        shared.quitting.store(true, Ordering::SeqCst);
                    }
                    let flush = flush_frontends(&app, "prepare-exit");
                    if flush.result.is_err() {
                        shared.quitting.store(false, Ordering::SeqCst);
                        app.state::<ExitState>()
                            .requested
                            .store(false, Ordering::SeqCst);
                        let _ = app.emit("exit-cancelled", flush.request_id);
                        show_window(&app, "live", None);
                        return;
                    }
                    let (prepared, captured) = mpsc::channel();
                    if shared
                        .dictation
                        .send(dictation::Msg::PrepareShutdown(prepared))
                        .is_ok()
                    {
                        let _ = captured.recv();
                    }
                    match meeting_runtime::stop_and_wait(&shared) {
                        Ok(_) => {
                            while shared.library_jobs.load(Ordering::SeqCst) != 0 {
                                std::thread::sleep(Duration::from_millis(20));
                            }
                            let (reply, done) = mpsc::channel();
                            if shared
                                .dictation
                                .send(dictation::Msg::Shutdown(reply))
                                .is_ok()
                            {
                                let _ = done.recv();
                            }
                            app.state::<cloud::SyncRunner>().stop();
                            platform_audio::stop();
                            let listener = shared.hotkey.lock().unwrap().take();
                            drop(listener);
                            app.state::<ExitState>()
                                .approved
                                .store(true, Ordering::SeqCst);
                            app.exit(0);
                        }
                        Err(error) => {
                            let _ = error;
                            LOG.error("Stoppen uitgesteld na een fout bij het afronden");
                            app.state::<ExitState>()
                                .requested
                                .store(false, Ordering::SeqCst);
                            shared.quitting.store(false, Ordering::SeqCst);
                            let _ = app.emit("exit-cancelled", flush.request_id);
                            show_window(&app, "live", None);
                        }
                    }
                });
            }
            // Same as the Mac app: opening Pennel again from Finder or Spotlight shows the main window.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => show_main(_app, None),
            _ => {}
        });
}
