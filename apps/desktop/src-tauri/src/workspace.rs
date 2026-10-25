use std::collections::HashSet;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

pub const WINDOW_MARGIN: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FittedWindow {
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

/// Monitor work areas are physical pixels; builder sizes and minima are logical points.
pub fn fit_window_size(
    requested: (f64, f64),
    minimum: (f64, f64),
    physical_area: (u32, u32),
    scale: f64,
) -> FittedWindow {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let available_width = (f64::from(physical_area.0) / scale - WINDOW_MARGIN * 2.0).max(1.0);
    let available_height = (f64::from(physical_area.1) / scale - WINDOW_MARGIN * 2.0).max(1.0);
    let width = requested.0.min(available_width).max(1.0);
    let height = requested.1.min(available_height).max(1.0);
    FittedWindow {
        width,
        height,
        min_width: minimum.0.min(width),
        min_height: minimum.1.min(height),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSection {
    Dictation,
    Meetings,
    History,
    Settings,
    Recording,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRoute {
    pub section: WorkspaceSection,
    pub tab: Option<String>,
    pub meeting_id: Option<String>,
    pub jump_ms: Option<u64>,
}

impl WorkspaceRoute {
    fn section(section: WorkspaceSection) -> Self {
        Self {
            section,
            tab: None,
            meeting_id: None,
            jump_ms: None,
        }
    }
}

#[derive(Default)]
pub struct Navigation {
    pending: Mutex<Option<WorkspaceRoute>>,
    onboarding: Mutex<Option<String>>,
    requested: Mutex<HashSet<String>>,
}

pub fn request_onboarding(app: &AppHandle, tab: Option<&str>) {
    if let Some(tab @ ("welcome" | "engine" | "permissions" | "practice" | "resume")) = tab {
        *app.state::<Navigation>().onboarding.lock().unwrap() = Some(tab.into());
    }
}

pub fn deliver_onboarding(app: &AppHandle) {
    if let Some(tab) = app.state::<Navigation>().onboarding.lock().unwrap().take() {
        let _ = app.emit_to("onboarding", "navigate", tab);
    }
}

pub fn valid_meeting_id(value: &str) -> bool {
    !value.trim().is_empty()
        && !matches!(value, "." | "..")
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

pub fn legacy_route(kind: &str, tab: Option<&str>) -> Option<WorkspaceRoute> {
    use WorkspaceSection::*;
    let mut route = WorkspaceRoute::section(match kind {
        "dictation" => Dictation,
        "history" => History,
        "settings" => Settings,
        "live" | "recording" => Recording,
        "library" | "meetings" => Meetings,
        _ => return None,
    });
    match route.section {
        Settings => {
            route.tab = Some(
                match tab.unwrap_or("providers") {
                    "meetings" => "meeting-settings",
                    "local" => "local",
                    known @ ("providers" | "models" | "dictation" | "meeting-settings"
                    | "recording" | "dictionary" | "styles" | "appearance" | "account") => known,
                    _ => "providers",
                }
                .into(),
            );
        }
        Meetings => match tab {
            Some("chat" | "__chat__") => route.tab = Some("chat".into()),
            Some("library" | "meetings") | None => {}
            Some(id)
                if valid_meeting_id(id)
                    && !matches!(
                        id,
                        "providers"
                            | "models"
                            | "dictation"
                            | "meeting-settings"
                            | "recording"
                            | "dictionary"
                            | "styles"
                            | "appearance"
                            | "account"
                            | "settings"
                            | "history"
                            | "onboarding"
                            | "live"
                            | "pill"
                            | "prompt"
                    ) =>
            {
                route.meeting_id = Some(id.into())
            }
            _ => {}
        },
        _ => {}
    }
    Some(route)
}

pub fn request(app: &AppHandle, label: &str, route: Option<WorkspaceRoute>) {
    let navigation = app.state::<Navigation>();
    navigation.requested.lock().unwrap().insert(label.into());
    if route.is_some() {
        *navigation.pending.lock().unwrap() = route;
    }
}

pub fn deliver(app: &AppHandle) {
    if let Some(route) = app.state::<Navigation>().pending.lock().unwrap().take() {
        let _ = app.emit_to("main", "workspace-navigate", route);
    }
}

pub fn requested(app: &AppHandle, label: &str) -> bool {
    app.state::<Navigation>()
        .requested
        .lock()
        .unwrap()
        .contains(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_work_area_preserves_the_approved_wizard_size() {
        assert_eq!(
            fit_window_size((850.0, 690.0), (520.0, 520.0), (1920, 1040), 1.0),
            FittedWindow {
                width: 850.0,
                height: 690.0,
                min_width: 520.0,
                min_height: 520.0
            }
        );
    }

    #[test]
    fn high_dpi_short_work_area_clamps_requested_height_and_minimum() {
        assert_eq!(
            fit_window_size((850.0, 690.0), (520.0, 520.0), (1920, 1080), 2.0),
            FittedWindow {
                width: 850.0,
                height: 492.0,
                min_width: 520.0,
                min_height: 492.0
            }
        );
    }

    #[test]
    fn tiny_work_area_never_retains_an_unreachable_minimum() {
        assert_eq!(
            fit_window_size((850.0, 690.0), (520.0, 520.0), (800, 600), 2.0),
            FittedWindow {
                width: 352.0,
                height: 252.0,
                min_width: 352.0,
                min_height: 252.0
            }
        );
    }

    #[test]
    fn fractional_scale_and_main_window_minimum_share_the_same_fitting_rules() {
        assert_eq!(
            fit_window_size((1040.0, 740.0), (760.0, 520.0), (1536, 864), 1.5),
            FittedWindow {
                width: 976.0,
                height: 528.0,
                min_width: 760.0,
                min_height: 520.0
            }
        );
        assert_eq!(
            fit_window_size((850.0, 690.0), (520.0, 520.0), (1200, 900), 0.0),
            fit_window_size((850.0, 690.0), (520.0, 520.0), (1200, 900), 1.0)
        );
    }

    #[test]
    fn old_window_routes_land_in_one_workspace() {
        assert_eq!(
            legacy_route("history", None).unwrap().section,
            WorkspaceSection::History
        );
        assert_eq!(
            legacy_route("live", None).unwrap().section,
            WorkspaceSection::Recording
        );
        assert_eq!(
            legacy_route("settings", Some("meetings"))
                .unwrap()
                .tab
                .as_deref(),
            Some("meeting-settings")
        );
        assert_eq!(
            legacy_route("library", Some("chat"))
                .unwrap()
                .tab
                .as_deref(),
            Some("chat")
        );
        assert_eq!(
            legacy_route("library", Some("2026-10-25 Productoverleg"))
                .unwrap()
                .meeting_id
                .as_deref(),
            Some("2026-10-25 Productoverleg")
        );
    }

    #[test]
    fn meeting_routes_never_accept_paths_or_controls() {
        for bad in ["../meeting", ".", "..", "a/b", "a\\b", "a:b", "a\n"] {
            assert!(!valid_meeting_id(bad));
            assert!(
                legacy_route("library", Some(bad))
                    .unwrap()
                    .meeting_id
                    .is_none()
            );
        }
    }
}
