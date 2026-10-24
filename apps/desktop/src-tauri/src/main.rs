#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod clipboard;
mod clipboard_system;
mod commands;
mod diagnostics;
mod dictation;
mod foreground;
mod gesture;
mod hotkey;
mod keys;
mod meeting;
mod paste_queue;
mod pill;
mod resample;
mod secrets;
mod state;
mod status;

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pennel_core::{PennelEngine, ProviderKind};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WindowEvent};

use crate::diagnostics::Log;
use crate::hotkey::Listener;
use crate::state::Shared;

const LOG: Log = Log::new("app");

/// Models are large, so everything lives in the local (non-roaming) data folder.
/// On a Mac (development only) it stays apart from the native app's "Pennel" folder.
fn data_dir(app: &AppHandle) -> PathBuf {
    let name = if cfg!(target_os = "macos") {
        "Pennel Desktop"
    } else {
        "Pennel"
    };
    app.path()
        .local_data_dir()
        .map(|dir| dir.join(name))
        .unwrap_or_else(|_| PathBuf::from(name))
}

pub fn show_main(app: &AppHandle, tab: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(tab) = tab {
        commands::emit_navigate(app, tab);
    }
}

fn hotkey_label(trigger: pennel_core::HotkeyTrigger) -> &'static str {
    use pennel_core::HotkeyTrigger::*;
    match trigger {
        RightControl | Fn => "rechter Ctrl",
        RightOption => "rechter Alt",
        RightCommand => "rechter Windows-toets",
        ControlSuper => "Ctrl + Windows-toets",
    }
}

pub struct TrayHint(pub MenuItem<tauri::Wry>);

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
    if account.kind == ProviderKind::Local {
        return shared.engine.model_state(model) != pennel_core::ModelState::Downloaded;
    }
    pennel_core::requires_key(account.kind) && shared.secrets.get(&account.id).is_none()
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let dir = data_dir(&handle);
    diagnostics::init(dir.join("logs").join("pennel.log"));
    LOG.info(format!(
        "Pennel {} gestart op {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    ));
    let engine = PennelEngine::new(dir.to_string_lossy().into_owned())?;
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Shared {
        engine,
        secrets: secrets::Secrets::default(),
        data_dir: dir,
        dictation: tx.clone(),
        hotkey: Mutex::new(None),
        hotkey_error: Mutex::new(None),
        meeting: Mutex::new(None),
    });
    app.manage(shared.clone());

    pill::create(&handle)?;
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
    if needs_setup(&shared) {
        show_main(&handle, Some("providers"));
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app, None);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(setup)
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
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
            commands::meetings,
            commands::meeting_detail,
            commands::meeting_templates,
            commands::summarize_meeting,
            commands::retranscribe_meeting,
            commands::rename_meeting,
            commands::delete_meeting,
            commands::show_main,
            commands::hide_window,
        ])
        .build(tauri::generate_context!())
        .expect("Pennel kon niet starten")
        .run(|_, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
