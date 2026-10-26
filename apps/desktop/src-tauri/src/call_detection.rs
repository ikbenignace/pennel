//! Native observers report activity; shared Rust rules decide whether to offer or start recording.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pennel_core::{AppRule, CallDetector, CallEvent, CallObservation, RecordAction};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::diagnostics::Log;
use crate::state::Shared;

const LOG: Log = Log::new("calls");

#[derive(Clone, Serialize)]
pub struct RecordPrompt {
    pub id: u64,
    pub app_id: String,
    pub app_name: String,
    pub title: Option<String>,
    pub expires_at_ms: u64,
    #[serde(skip)]
    attendees: Vec<String>,
}

#[derive(Default)]
pub struct DetectionState {
    pub prompt: Mutex<Option<RecordPrompt>>,
    active: Mutex<Option<String>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn snapshot() -> Result<Vec<CallObservation>, String> {
    #[cfg(target_os = "macos")]
    return crate::macos::call_snapshot(
        &pennel_core::known_call_apps(),
        &pennel_core::browser_apps(),
    )
    .map(|users| {
        users
            .into_iter()
            .map(|user| CallObservation {
                app_id: user.app_id,
                is_call: user.is_call,
                is_playing: user.is_playing,
            })
            .collect()
    });
    #[cfg(windows)]
    return crate::windows_meetings::snapshot();
    #[cfg(target_os = "linux")]
    return crate::linux_meetings::snapshot();
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    Ok(Vec::new())
}

pub fn calendar() -> serde_json::Value {
    #[cfg(target_os = "macos")]
    return crate::macos::calendar_snapshot();
    #[cfg(not(target_os = "macos"))]
    serde_json::json!({ "calendars": [], "current_event": null })
}

fn dismiss(app: &AppHandle) {
    if let Some(state) = app.try_state::<DetectionState>() {
        state.prompt.lock().unwrap().take();
    }
    let _ = app.emit("record-prompt", Option::<RecordPrompt>::None);
    if let Some(window) = app.get_webview_window("record-prompt") {
        crate::pill::hide_without_focus(&window);
    }
}

fn offer(app: &AppHandle, shared: &Arc<Shared>, app_id: String) {
    if shared.updating.load(Ordering::SeqCst)
        || shared.quitting.load(Ordering::SeqCst)
        || shared.recording.lock().unwrap().is_busy()
    {
        return;
    }
    let settings = shared.settings();
    let calendar = if settings.calendar_enabled {
        calendar()
    } else {
        serde_json::Value::Null
    };
    let event = calendar.get("current_event");
    let title = event
        .and_then(|event| event.get("title"))
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let calendar_id = event
        .and_then(|event| event.get("calendar_id"))
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let attendees = event
        .and_then(|event| event.get("attendees"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    match pennel_core::decide_recording(settings, app_id.clone(), calendar_id, title.clone()) {
        RecordAction::Never => {}
        RecordAction::Always => {
            if crate::meeting_runtime::start(
                shared,
                title.unwrap_or_else(|| "Meeting".into()),
                Some(app_id),
                attendees,
            )
            .is_ok()
            {
                let _ = app.emit("meeting-updated", ());
            } else {
                LOG.error("Automatische opname kon niet starten; open Pennel voor de opnamestatus");
            }
        }
        RecordAction::Ask => {
            static NEXT_PROMPT: AtomicU64 = AtomicU64::new(1);
            let prompt = RecordPrompt {
                id: NEXT_PROMPT.fetch_add(1, Ordering::Relaxed),
                app_name: app_name(&app_id),
                app_id,
                title,
                expires_at_ms: now_ms() + 60_000,
                attendees,
            };
            *app.state::<DetectionState>().prompt.lock().unwrap() = Some(prompt.clone());
            crate::show_window(app, "prompt", None);
            let _ = app.emit("record-prompt", Some(prompt));
        }
    }
}

fn app_name(app_id: &str) -> String {
    match app_id {
        "com.microsoft.teams2" => "Microsoft Teams",
        "us.zoom.xos" => "Zoom",
        "com.google.Chrome" => "Google Chrome",
        "com.microsoft.edgemac" => "Microsoft Edge",
        "com.apple.FaceTime" => "FaceTime",
        "com.apple.Safari" => "Safari",
        "org.mozilla.firefox" => "Firefox",
        "com.tinyspeck.slackmacgap" => "Slack",
        other => return other.to_owned(),
    }
    .into()
}

pub fn answer(
    app: &AppHandle,
    shared: &Arc<Shared>,
    answer: &str,
    prompt_id: u64,
) -> Result<(), String> {
    if !matches!(answer, "yes" | "no" | "never") {
        return Err("Onbekend antwoord".into());
    }
    let state = app.state::<DetectionState>();
    let prompt = {
        let mut pending = state.prompt.lock().unwrap();
        if !pending
            .as_ref()
            .is_some_and(|prompt| prompt.id == prompt_id && prompt.expires_at_ms > now_ms())
        {
            return Err("De opnamevraag is verlopen".into());
        }
        pending.take().expect("validated prompt")
    };
    dismiss(app);
    if state.active.lock().unwrap().as_deref() != Some(&prompt.app_id) {
        return Err("De gedetecteerde meeting is afgelopen".into());
    }
    match answer {
        "yes" => {
            let started = crate::meeting_runtime::start(
                shared,
                prompt.title.unwrap_or_else(|| "Meeting".into()),
                Some(prompt.app_id),
                prompt.attendees,
            );
            if started.is_err() {
                // The prompt is already gone; the recording view shows why it did not start.
                LOG.error("Opname na opnamevraag kon niet starten");
                crate::show_window(app, "live", None);
            }
            started?;
        }
        "never" => {
            let mut settings = shared.settings();
            settings
                .app_rules
                .retain(|rule| rule.bundle_id != prompt.app_id);
            settings.app_rules.push(AppRule {
                bundle_id: prompt.app_id,
                action: RecordAction::Never,
            });
            shared
                .engine
                .update_settings(settings)
                .map_err(|error| error.to_string())?;
        }
        _ => {}
    }
    Ok(())
}

pub fn toggle_recording(app: AppHandle, shared: Arc<Shared>) {
    if shared.recording.lock().unwrap().is_busy() {
        let _ = crate::meeting_runtime::stop(app, shared, None);
    } else {
        crate::show_window(&app, "live", None);
        if crate::meeting_runtime::start(&shared, "Meeting".into(), None, Vec::new()).is_err() {
            LOG.error("Opname via sneltoets kon niet starten");
        }
    }
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("pennel-calls".into())
        .spawn(move || {
            let mut detector = CallDetector::new(app.config().identifier.clone());
            let started = Instant::now();
            let mut was_enabled = false;
            loop {
                #[cfg(target_os = "macos")]
                crate::refresh_hotkey(&app, &shared);
                let quitting = app
                    .state::<crate::ExitState>()
                    .requested
                    .load(Ordering::SeqCst);
                let enabled = shared.settings().auto_detect && !quitting;
                if enabled {
                    if let Ok(users) = snapshot() {
                        for event in detector.update(&users, started.elapsed().as_secs_f64()) {
                            match event {
                                CallEvent::Started { app_id } => {
                                    *app.state::<DetectionState>().active.lock().unwrap() =
                                        Some(app_id.clone());
                                    offer(&app, &shared, app_id);
                                }
                                CallEvent::Ended { app_id } => {
                                    app.state::<DetectionState>().active.lock().unwrap().take();
                                    dismiss(&app);
                                    let _ = crate::meeting_runtime::stop(
                                        app.clone(),
                                        shared.clone(),
                                        Some(&app_id),
                                    );
                                }
                            }
                        }
                    }
                    let expired = app
                        .state::<DetectionState>()
                        .prompt
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|prompt| prompt.expires_at_ms <= now_ms());
                    if expired {
                        dismiss(&app);
                    }
                } else if was_enabled {
                    detector = CallDetector::new(app.config().identifier.clone());
                    app.state::<DetectionState>().active.lock().unwrap().take();
                    dismiss(&app);
                }
                was_enabled = enabled;
                std::thread::sleep(Duration::from_secs(2));
            }
        })
        .expect("call observer thread");
}
