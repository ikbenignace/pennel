use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::MusliError;
use crate::provider::{preset, ProviderConfig, ProviderKind, ProviderTask};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    Fast,
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Casual,
    Formal,
    Literal,
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct StyleRule {
    pub bundle_id: String,
    pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyTrigger {
    Fn,
    RightOption,
    RightCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Settings {
    pub dictation_stt: ProviderConfig,
    pub meeting_stt: ProviderConfig,
    pub llm: ProviderConfig,
    pub mode: DictationMode,
    pub language: Option<String>,
    pub dictionary: Vec<String>,
    pub style_rules: Vec<StyleRule>,
    pub history_retention_days: u32,
    pub hotkey: HotkeyTrigger,
}

const DEFAULT_STYLES: &[(&str, Style)] = &[
    ("com.tinyspeck.slackmacgap", Style::Casual),
    ("net.whatsapp.WhatsApp", Style::Casual),
    ("com.apple.MobileSMS", Style::Casual),
    ("ru.keepcoder.Telegram", Style::Casual),
    ("com.apple.mail", Style::Formal),
    ("com.microsoft.Outlook", Style::Formal),
    ("com.microsoft.Word", Style::Formal),
    ("com.anthropic.claudefordesktop", Style::Literal),
    ("com.openai.chat", Style::Literal),
    ("com.apple.Terminal", Style::Literal),
    ("com.googlecode.iterm2", Style::Literal),
    ("com.mitchellh.ghostty", Style::Literal),
    ("com.microsoft.VSCode", Style::Literal),
    ("dev.zed.Zed", Style::Literal),
    ("com.todesktop.230313mzl4w4u92", Style::Literal),
];

#[uniffi::export]
pub fn default_settings() -> Settings {
    Settings {
        dictation_stt: preset(ProviderKind::Groq, ProviderTask::DictationStt),
        meeting_stt: preset(ProviderKind::Groq, ProviderTask::MeetingStt),
        llm: preset(ProviderKind::Groq, ProviderTask::Llm),
        mode: DictationMode::Clean,
        language: None,
        dictionary: Vec::new(),
        style_rules: DEFAULT_STYLES
            .iter()
            .map(|(bundle_id, style)| StyleRule { bundle_id: (*bundle_id).into(), style: *style })
            .collect(),
        history_retention_days: 30,
        hotkey: HotkeyTrigger::Fn,
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Settings, MusliError> {
        let json = match std::fs::read_to_string(path) {
            Ok(json) => json,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(default_settings()),
            Err(e) => return Err(MusliError::storage(e)),
        };
        match serde_json::from_str(&json) {
            Ok(settings) => Ok(settings),
            Err(_) => {
                std::fs::rename(path, path.with_file_name("settings.corrupt.json")).map_err(MusliError::storage)?;
                Ok(default_settings())
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), MusliError> {
        let json = serde_json::to_string_pretty(self).map_err(MusliError::storage)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(MusliError::storage)?;
        std::fs::rename(&tmp, path).map_err(MusliError::storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_clean_mode_groq_fn() {
        let s = default_settings();
        assert_eq!(s.mode, DictationMode::Clean);
        assert_eq!(s.hotkey, HotkeyTrigger::Fn);
        assert_eq!(s.history_retention_days, 30);
        assert_eq!(s.language, None);
        assert_eq!(s.dictation_stt.kind, ProviderKind::Groq);
        let slack = s.style_rules.iter().find(|r| r.bundle_id == "com.tinyspeck.slackmacgap").unwrap();
        assert_eq!(slack.style, Style::Casual);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings::load(&dir.path().join("settings.json")).unwrap();
        assert_eq!(s, default_settings());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = default_settings();
        s.dictionary = vec!["Musli".into(), "Q8".into()];
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
    }

    #[test]
    fn load_corrupt_file_returns_defaults_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path).unwrap(), default_settings());
        assert!(dir.path().join("settings.corrupt.json").exists());
    }

    #[test]
    fn settings_json_never_contains_api_key_fields() {
        let json = serde_json::to_string(&default_settings()).unwrap();
        assert!(!json.contains("api_key"));
    }
}
