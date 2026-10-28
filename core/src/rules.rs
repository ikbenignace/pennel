use serde::{Deserialize, Serialize};

use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum RecordAction {
    Ask,
    Always,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AppRule {
    pub bundle_id: String,
    pub action: RecordAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct CalendarRule {
    pub calendar_id: String,
    pub calendar_name: String,
    pub action: RecordAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct EventRule {
    pub title: String,
    pub action: RecordAction,
}

const CALL_APPS: &[&str] = &[
    "com.microsoft.teams2",
    "us.zoom.xos",
    "com.tinyspeck.slackmacgap",
    "com.apple.FaceTime",
    "net.whatsapp.WhatsApp",
    "Cisco-Systems.Spark",
    "com.hnc.Discord",
];

const BROWSERS: &[&str] = &[
    "com.google.Chrome",
    "com.apple.Safari",
    "company.thebrowser.Browser",
    "com.microsoft.edgemac",
    "org.mozilla.firefox",
    "com.brave.Browser",
];

#[uniffi::export]
pub fn known_call_apps() -> Vec<String> {
    CALL_APPS.iter().map(|s| (*s).to_owned()).collect()
}

#[uniffi::export]
pub fn browser_apps() -> Vec<String> {
    BROWSERS.iter().map(|s| (*s).to_owned()).collect()
}

#[uniffi::export]
pub fn consent_sentence() -> String {
    consent_sentence_for_language(crate::InterfaceLanguage::English)
}

#[uniffi::export]
pub fn consent_sentence_for_language(language: crate::InterfaceLanguage) -> String {
    crate::ui_text(language, "meetings.consent_sentence", &[])
}

/// Most specific rule wins: meeting title, then calendar, then app; otherwise ask.
#[uniffi::export]
pub fn decide_recording(
    settings: Settings,
    bundle_id: String,
    calendar_id: Option<String>,
    event_title: Option<String>,
) -> RecordAction {
    let by_event = event_title.as_deref().and_then(|title| {
        settings
            .event_rules
            .iter()
            .find(|r| r.title.trim().eq_ignore_ascii_case(title.trim()))
            .map(|r| r.action)
    });
    let by_calendar = calendar_id.as_deref().and_then(|id| {
        settings
            .calendar_rules
            .iter()
            .find(|r| r.calendar_id == id)
            .map(|r| r.action)
    });
    let by_app = settings
        .app_rules
        .iter()
        .find(|r| r.bundle_id == bundle_id)
        .map(|r| r.action);
    by_event
        .or(by_calendar)
        .or(by_app)
        .unwrap_or(RecordAction::Ask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::default_settings;

    const TEAMS: &str = "com.microsoft.teams2";

    #[test]
    fn consent_announcement_uses_english_by_default_and_localizes_explicitly() {
        assert_eq!(
            consent_sentence(),
            "I am taking notes with a transcript using Pennel."
        );
        assert_eq!(
            consent_sentence_for_language(crate::InterfaceLanguage::Dutch),
            "Ik neem notities met transcript via Pennel."
        );
    }

    #[test]
    fn default_is_ask() {
        assert_eq!(
            decide_recording(default_settings(), TEAMS.into(), None, None),
            RecordAction::Ask
        );
    }

    #[test]
    fn app_rule_applies() {
        let mut s = default_settings();
        s.app_rules.push(AppRule {
            bundle_id: TEAMS.into(),
            action: RecordAction::Never,
        });
        assert_eq!(
            decide_recording(s, TEAMS.into(), None, None),
            RecordAction::Never
        );
    }

    #[test]
    fn calendar_rule_beats_app_rule() {
        let mut s = default_settings();
        s.app_rules.push(AppRule {
            bundle_id: TEAMS.into(),
            action: RecordAction::Never,
        });
        s.calendar_rules.push(CalendarRule {
            calendar_id: "werk".into(),
            calendar_name: "Werk".into(),
            action: RecordAction::Always,
        });
        assert_eq!(
            decide_recording(s.clone(), TEAMS.into(), Some("werk".into()), None),
            RecordAction::Always
        );
        assert_eq!(
            decide_recording(s, TEAMS.into(), Some("prive".into()), None),
            RecordAction::Never
        );
    }

    #[test]
    fn event_rule_beats_calendar_and_app() {
        let mut s = default_settings();
        s.app_rules.push(AppRule {
            bundle_id: TEAMS.into(),
            action: RecordAction::Always,
        });
        s.calendar_rules.push(CalendarRule {
            calendar_id: "werk".into(),
            calendar_name: "Werk".into(),
            action: RecordAction::Always,
        });
        s.event_rules.push(EventRule {
            title: "Standup".into(),
            action: RecordAction::Never,
        });
        assert_eq!(
            decide_recording(s, TEAMS.into(), Some("werk".into()), Some("standup".into())),
            RecordAction::Never
        );
    }

    #[test]
    fn known_apps_include_teams_and_browsers_are_separate() {
        assert!(known_call_apps().contains(&TEAMS.to_owned()));
        assert!(browser_apps().contains(&"com.google.Chrome".to_owned()));
        assert!(!known_call_apps().contains(&"com.google.Chrome".to_owned()));
    }

    #[test]
    fn old_settings_get_detection_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut value = serde_json::to_value(default_settings()).unwrap();
        let map = value.as_object_mut().unwrap();
        for key in [
            "app_rules",
            "calendar_rules",
            "event_rules",
            "calendar_enabled",
            "echo_cancellation",
            "record_shortcut_enabled",
            "consent_enabled",
            "auto_detect",
        ] {
            map.remove(key);
        }
        std::fs::write(&path, value.to_string()).unwrap();
        let s = crate::settings::Settings::load(&path).unwrap();
        assert!(s.app_rules.is_empty());
        assert!(!s.calendar_enabled);
        assert!(s.echo_cancellation);
        assert!(s.record_shortcut_enabled);
        assert!(!s.consent_enabled);
        assert!(s.auto_detect);
        assert!(!dir.path().join("settings.corrupt.json").exists());
    }
}
