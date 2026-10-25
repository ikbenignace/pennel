#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod call_detection;
mod capabilities;
#[cfg(not(target_os = "macos"))]
mod clipboard;
#[cfg(not(target_os = "macos"))]
mod clipboard_system;
mod commands;
mod diagnostics;
mod dictation;
mod external_paste;
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
mod owned_dictation;
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
mod style_application;
#[cfg(windows)]
mod windows_meetings;
mod workspace;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pennel_core::PennelEngine;
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
        Some("meetings" | "library" | "chat") => "library",
        _ => "workspace",
    };
    show_window(app, kind, tab);
}

pub fn show_window(app: &AppHandle, kind: &str, tab: Option<&str>) {
    let (label, mode, width, height) = match kind {
        "onboarding" => ("onboarding", "onboarding", 850.0, 690.0),
        "prompt" => ("record-prompt", "prompt", 420.0, 210.0),
        _ => ("main", "workspace", 1040.0, 740.0),
    };
    workspace::request(app, label, workspace::legacy_route(kind, tab));
    if kind == "onboarding" {
        workspace::request_onboarding(app, tab);
    }
    let app = app.clone();
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let window = handle.get_webview_window(label).or_else(|| {
            let minimum = if mode == "onboarding" {
                (520.0, 520.0)
            } else {
                (760.0, 520.0)
            };
            let fit = if mode == "prompt" {
                None
            } else {
                preferred_monitor(&handle).map(|monitor| {
                    let area = monitor.work_area();
                    workspace::fit_window_size(
                        (width, height),
                        minimum,
                        (area.size.width, area.size.height),
                        monitor.scale_factor(),
                    )
                })
            };
            let url = format!("index.html?window={mode}");
            let mut builder = tauri::WebviewWindowBuilder::new(
                &handle,
                label,
                tauri::WebviewUrl::App(url.into()),
            )
            .title("Pennel")
            .inner_size(
                fit.map_or(width, |size| size.width),
                fit.map_or(height, |size| size.height),
            )
            .visible(false);
            if mode == "onboarding" {
                builder = builder
                    .decorations(false)
                    .min_inner_size(
                        fit.map_or(520.0, |size| size.min_width),
                        fit.map_or(520.0, |size| size.min_height),
                    )
                    .center();
            }
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
            apply_appearance(&handle);
            fit_window_to_work_area(&handle, &window);
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
                let ready = handle
                    .state::<ExitState>()
                    .sync
                    .lock()
                    .unwrap()
                    .ready
                    .contains(label);
                if ready {
                    if label == "main" {
                        workspace::deliver(&handle);
                    }
                    if label == "onboarding" {
                        workspace::deliver_onboarding(&handle);
                    }
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    });
}

fn preferred_monitor(app: &AppHandle) -> Option<tauri::Monitor> {
    let cursor = app.cursor_position().ok();
    let monitors = app.available_monitors().unwrap_or_default();
    cursor
        .and_then(|cursor| {
            monitors.into_iter().find(|monitor| {
                let (position, size) = (monitor.position(), monitor.size());
                cursor.x >= f64::from(position.x)
                    && cursor.x < f64::from(position.x) + f64::from(size.width)
                    && cursor.y >= f64::from(position.y)
                    && cursor.y < f64::from(position.y) + f64::from(size.height)
            })
        })
        .or_else(|| app.primary_monitor().ok().flatten())
}

fn fit_window_to_work_area(app: &AppHandle, window: &tauri::WebviewWindow) {
    if !matches!(window.label(), "main" | "onboarding")
        || window.is_maximized().unwrap_or(false)
        || window.is_fullscreen().unwrap_or(false)
    {
        return;
    }
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| preferred_monitor(app))
    else {
        return;
    };
    let (Ok(inner), Ok(outer)) = (window.inner_size(), window.outer_size()) else {
        return;
    };
    let area = monitor.work_area();
    if area.size.width == 0 || area.size.height == 0 {
        return;
    }
    let scale = monitor.scale_factor();
    let window_scale = window.scale_factor().unwrap_or(scale);
    let window_scale = if window_scale.is_finite() && window_scale > 0.0 {
        window_scale
    } else {
        1.0
    };
    let frame_width = outer.width.saturating_sub(inner.width);
    let frame_height = outer.height.saturating_sub(inner.height);
    let minimum = if window.label() == "onboarding" {
        (520.0, 520.0)
    } else {
        (760.0, 520.0)
    };
    let fit = workspace::fit_window_size(
        (
            f64::from(inner.width) / window_scale,
            f64::from(inner.height) / window_scale,
        ),
        minimum,
        (
            area.size.width.saturating_sub(frame_width),
            area.size.height.saturating_sub(frame_height),
        ),
        scale,
    );
    let _ = window.set_min_size(Some(tauri::LogicalSize::new(fit.min_width, fit.min_height)));
    if f64::from(inner.width) / window_scale > fit.width + 0.5
        || f64::from(inner.height) / window_scale > fit.height + 0.5
    {
        let _ = window.set_size(tauri::LogicalSize::new(fit.width, fit.height));
    }
    let (Ok(position), Ok(outer)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let margin = (workspace::WINDOW_MARGIN * scale) as i64;
    let left = i64::from(area.position.x) + margin;
    let top = i64::from(area.position.y) + margin;
    let right =
        (i64::from(area.position.x) + i64::from(area.size.width) - i64::from(outer.width) - margin)
            .max(left);
    let bottom = (i64::from(area.position.y) + i64::from(area.size.height)
        - i64::from(outer.height)
        - margin)
        .max(top);
    let x = i64::from(position.x).clamp(left, right) as i32;
    let y = i64::from(position.y).clamp(top, bottom) as i32;
    if x != position.x || y != position.y {
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

pub fn apply_appearance(app: &AppHandle) {
    let Some(shared) = app.try_state::<Arc<Shared>>() else {
        return;
    };
    let theme = match shared.settings().appearance {
        pennel_core::AppearancePreference::System => None,
        pennel_core::AppearancePreference::Light => Some(tauri::Theme::Light),
        pennel_core::AppearancePreference::Dark => Some(tauri::Theme::Dark),
    };
    for window in app.webview_windows().into_values() {
        let _ = window.set_theme(theme);
    }
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
    if label == "main" {
        workspace::deliver(app);
    }
    if label == "onboarding" {
        workspace::deliver_onboarding(app);
    }
    if workspace::requested(app, label) {
        apply_appearance(app);
        if let Some(window) = app.get_webview_window(label) {
            fit_window_to_work_area(app, &window);
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
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
            "open" => show_main(app, None),
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

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let dir = data_dir(&handle)?;
    let preview = is_preview(&handle);
    app.manage(ExitState::default());
    app.manage(workspace::Navigation::default());
    app.manage(owned_dictation::Sessions::default());
    external_paste::start_tracking();
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
    let secrets = secrets::Secrets::new(preview);
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
    let handled_cli = dispatch_cli(&handle, &std::env::args().collect::<Vec<_>>());
    if !handled_cli {
        if shared.settings().onboarding.status == pennel_core::OnboardingStatus::Pending {
            show_window(&handle, "onboarding", None);
        } else {
            show_main(&handle, None);
        }
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
            if let WindowEvent::ScaleFactorChanged { .. } = event
                && let Some(webview) = window.app_handle().get_webview_window(window.label())
            {
                fit_window_to_work_area(window.app_handle(), &webview);
            }
            if let WindowEvent::Focused(false) = event {
                owned_dictation::focus_lost(window.app_handle(), window.label());
            }
            if let WindowEvent::CloseRequested { api, .. } = event
                && matches!(
                    window.label(),
                    "main" | "live" | "settings" | "history" | "onboarding"
                )
            {
                api.prevent_close();
                owned_dictation::close_owner(window.app_handle(), window.label());
                if window.label() == "onboarding"
                    && let Some(shared) = window.app_handle().try_state::<Arc<Shared>>()
                {
                    let mut settings = shared.settings();
                    if settings.onboarding.status != pennel_core::OnboardingStatus::Completed {
                        settings.onboarding.status = pennel_core::OnboardingStatus::Skipped;
                        let _ = shared.engine.update_settings(settings);
                    }
                }
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
            platform_commands::show_window,
            platform_commands::show_workspace,
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
            style_application::pick_style_application,
            commands::model_state,
            commands::show_main,
            commands::hide_window,
            owned_dictation::onboarding_readiness,
            owned_dictation::pad_text,
            owned_dictation::save_pad_text,
            owned_dictation::open_dictation_session,
            owned_dictation::get_dictation_session,
            owned_dictation::toggle_dictation_session,
            owned_dictation::cancel_dictation_session,
            owned_dictation::close_dictation_session,
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
