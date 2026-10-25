use std::collections::{HashMap, HashSet};
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

/// Each product window, as in the original Pennel: the library is the main window and settings,
/// history and dictation open in their own titled windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSpec {
    pub label: &'static str,
    pub mode: &'static str,
    pub title: &'static str,
    pub size: (f64, f64),
    pub minimum: (f64, f64),
}

pub fn window_spec(kind: &str) -> WindowSpec {
    let (label, mode, title, size, minimum) = match kind {
        "onboarding" => (
            "onboarding",
            "onboarding",
            "Welkom bij Pennel",
            (640.0, 660.0),
            (520.0, 520.0),
        ),
        "prompt" => (
            "record-prompt",
            "prompt",
            "Pennel",
            (420.0, 210.0),
            (420.0, 210.0),
        ),
        "settings" => (
            "settings",
            "settings",
            "Instellingen",
            (760.0, 600.0),
            (640.0, 460.0),
        ),
        "history" => (
            "history",
            "history",
            "Geschiedenis",
            (560.0, 560.0),
            (440.0, 360.0),
        ),
        "dictation" => (
            "dictation",
            "dictation",
            "Dicteren",
            (640.0, 560.0),
            (480.0, 420.0),
        ),
        _ => (
            "main",
            "workspace",
            "Pennel",
            (980.0, 640.0),
            (760.0, 520.0),
        ),
    };
    WindowSpec {
        label,
        mode,
        title,
        size,
        minimum,
    }
}

pub fn label_spec(label: &str) -> Option<WindowSpec> {
    let kind = match label {
        "main" => "workspace",
        "record-prompt" => "prompt",
        other => other,
    };
    let spec = window_spec(kind);
    (spec.label == label).then_some(spec)
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
    /// The window kind that shows this route.
    pub fn window_kind(&self) -> &'static str {
        match self.section {
            WorkspaceSection::Dictation => "dictation",
            WorkspaceSection::History => "history",
            WorkspaceSection::Settings => "settings",
            WorkspaceSection::Meetings | WorkspaceSection::Recording => "workspace",
        }
    }

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
    pending: Mutex<HashMap<String, WorkspaceRoute>>,
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
        Dictation if tab == Some("pad") => route.tab = Some("pad".into()),
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
    if let Some(route) = route {
        navigation
            .pending
            .lock()
            .unwrap()
            .insert(label.into(), route);
    }
}

pub fn deliver(app: &AppHandle, label: &str) {
    let route = app
        .state::<Navigation>()
        .pending
        .lock()
        .unwrap()
        .remove(label);
    if let Some(route) = route {
        let _ = app.emit_to(label, "workspace-navigate", route);
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
    fn settings_history_and_dictation_open_their_own_titled_windows() {
        for (kind, label, title) in [
            ("settings", "settings", "Instellingen"),
            ("history", "history", "Geschiedenis"),
            ("dictation", "dictation", "Dicteren"),
            ("onboarding", "onboarding", "Welkom bij Pennel"),
            ("library", "main", "Pennel"),
            ("live", "main", "Pennel"),
            ("workspace", "main", "Pennel"),
        ] {
            let spec = window_spec(kind);
            assert_eq!((spec.label, spec.title), (label, title));
            assert_eq!(label_spec(label), Some(spec));
        }
        assert_eq!(label_spec("pill"), None);
        assert_eq!(
            legacy_route("settings", Some("styles"))
                .unwrap()
                .window_kind(),
            "settings"
        );
        assert_eq!(
            legacy_route("live", None).unwrap().window_kind(),
            "workspace"
        );
        assert_eq!(
            legacy_route("dictation", Some("pad"))
                .unwrap()
                .tab
                .as_deref(),
            Some("pad")
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
