use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PennelError;
use crate::provider::{
    ProviderAccount, ProviderKind, ProviderTask, default_base_url, default_model, new_account,
    supports,
};
use crate::rules::{AppRule, CalendarRule, EventRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    Fast,
    Clean,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum AppearancePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStatus {
    Pending,
    Skipped,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    Welcome,
    Engine,
    Permissions,
    Practice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct OnboardingProgress {
    pub version: u32,
    pub status: OnboardingStatus,
    pub resume_step: OnboardingStep,
}

impl Default for OnboardingProgress {
    fn default() -> Self {
        // Missing fields belong to existing installations, not to a new first run.
        Self {
            version: 1,
            status: OnboardingStatus::Skipped,
            resume_step: OnboardingStep::Welcome,
        }
    }
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
    /// Windows/Linux: there is no fn key there, so Right Ctrl is the default.
    RightControl,
    /// Windows/Linux: Ctrl + Windows/Super held together.
    ControlSuper,
}

#[cfg(target_os = "macos")]
const DEFAULT_HOTKEY: HotkeyTrigger = HotkeyTrigger::Fn;
#[cfg(not(target_os = "macos"))]
const DEFAULT_HOTKEY: HotkeyTrigger = HotkeyTrigger::RightControl;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ModelChoice {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct Settings {
    pub providers: Vec<ProviderAccount>,
    pub dictation_stt: ModelChoice,
    pub meeting_stt: ModelChoice,
    pub llm: ModelChoice,
    pub mode: DictationMode,
    pub language: Option<String>,
    pub dictionary: Vec<String>,
    pub style_rules: Vec<StyleRule>,
    pub history_retention_days: u32,
    pub hotkey: HotkeyTrigger,
    pub local_unload_minutes: u32,
    /// Empty means `~/Pennel`.
    #[serde(default)]
    pub meetings_dir: String,
    #[serde(default)]
    pub meeting_audio_retention_days: u32,
    #[serde(default = "default_template_id")]
    pub default_template: String,
    #[serde(default)]
    pub app_rules: Vec<AppRule>,
    #[serde(default)]
    pub calendar_rules: Vec<CalendarRule>,
    #[serde(default)]
    pub event_rules: Vec<EventRule>,
    #[serde(default)]
    pub calendar_enabled: bool,
    #[serde(default = "enabled")]
    pub echo_cancellation: bool,
    #[serde(default = "enabled")]
    pub record_shortcut_enabled: bool,
    #[serde(default)]
    pub consent_enabled: bool,
    #[serde(default = "enabled")]
    pub auto_detect: bool,
    #[serde(default)]
    pub diarization: bool,
    #[serde(default)]
    pub cloud_sync: bool,
    #[serde(default)]
    pub appearance: AppearancePreference,
    #[serde(default)]
    pub onboarding: OnboardingProgress,
    #[serde(default)]
    pub dictation_pad_auto_paste: bool,
}

fn enabled() -> bool {
    true
}

fn default_template_id() -> String {
    "algemeen".into()
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

fn choice(account: &ProviderAccount, task: ProviderTask) -> ModelChoice {
    ModelChoice {
        provider_id: account.id.clone(),
        model: default_model(account.kind, task),
    }
}

#[uniffi::export]
pub fn default_settings() -> Settings {
    let groq = new_account(ProviderKind::Groq, vec![]);
    let local = new_account(ProviderKind::Local, vec![groq.clone()]);
    Settings {
        dictation_stt: choice(&local, ProviderTask::DictationStt),
        meeting_stt: choice(&local, ProviderTask::MeetingStt),
        llm: choice(&groq, ProviderTask::Llm),
        providers: vec![groq, local],
        mode: DictationMode::Fast,
        language: None,
        dictionary: Vec::new(),
        style_rules: DEFAULT_STYLES
            .iter()
            .map(|(bundle_id, style)| StyleRule {
                bundle_id: (*bundle_id).into(),
                style: *style,
            })
            .collect(),
        history_retention_days: 30,
        hotkey: DEFAULT_HOTKEY,
        local_unload_minutes: 5,
        meetings_dir: String::new(),
        meeting_audio_retention_days: 0,
        default_template: default_template_id(),
        app_rules: Vec::new(),
        calendar_rules: Vec::new(),
        event_rules: Vec::new(),
        calendar_enabled: false,
        echo_cancellation: true,
        record_shortcut_enabled: true,
        consent_enabled: false,
        auto_detect: true,
        diarization: false,
        cloud_sync: false,
        appearance: AppearancePreference::System,
        onboarding: OnboardingProgress {
            status: OnboardingStatus::Pending,
            ..OnboardingProgress::default()
        },
        dictation_pad_auto_paste: false,
    }
}

impl Settings {
    /// Startup migrations must never rename a user's settings or reset their providers.
    pub(crate) fn load_strict(path: &Path) -> Result<Settings, PennelError> {
        let json = match std::fs::read_to_string(path) {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(default_settings());
            }
            Err(error) => return Err(PennelError::storage(error)),
        };
        let value: serde_json::Value = serde_json::from_str(&json).map_err(PennelError::storage)?;
        if value.get("providers").is_some() {
            serde_json::from_value(value).map_err(PennelError::storage)
        } else {
            serde_json::from_value::<LegacySettings>(value)
                .map(LegacySettings::migrate)
                .map_err(PennelError::storage)
        }
    }

    pub fn load(path: &Path) -> Result<Settings, PennelError> {
        let json = match std::fs::read_to_string(path) {
            Ok(json) => json,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(default_settings()),
            Err(e) => return Err(PennelError::storage(e)),
        };
        let parsed = serde_json::from_str::<serde_json::Value>(&json)
            .ok()
            .and_then(|value| {
                if value.get("providers").is_some() {
                    serde_json::from_value::<Settings>(value).ok()
                } else {
                    serde_json::from_value::<LegacySettings>(value)
                        .ok()
                        .map(LegacySettings::migrate)
                }
            });
        match parsed {
            Some(settings) => Ok(settings.normalized()),
            None => {
                std::fs::rename(path, path.with_file_name("settings.corrupt.json"))
                    .map_err(PennelError::storage)?;
                Ok(default_settings())
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), PennelError> {
        let json = serde_json::to_string_pretty(self).map_err(PennelError::storage)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(PennelError::storage)?;
        std::fs::rename(&tmp, path).map_err(PennelError::storage)
    }

    pub fn resolve(&self, choice: &ModelChoice) -> Option<(ProviderAccount, String)> {
        self.providers
            .iter()
            .find(|p| p.id == choice.provider_id)
            .map(|p| (p.clone(), choice.model.clone()))
    }

    pub fn normalized(self) -> Settings {
        self.normalized_with(&[])
    }

    /// Points every task whose provider no longer exists at the first provider that can do it,
    /// and keeps local choices on a model that is actually downloaded.
    pub fn normalized_with(mut self, downloaded_local: &[String]) -> Settings {
        for (task, slot) in [
            (ProviderTask::DictationStt, &mut self.dictation_stt),
            (ProviderTask::MeetingStt, &mut self.meeting_stt),
            (ProviderTask::Llm, &mut self.llm),
        ] {
            let current = self.providers.iter().find(|p| p.id == slot.provider_id);
            let account = match current {
                Some(account) => account,
                None => match self.providers.iter().find(|p| supports(p.kind, task)) {
                    Some(fallback) => {
                        *slot = choice(fallback, task);
                        fallback
                    }
                    None => continue,
                },
            };
            if account.kind == ProviderKind::Local
                && !downloaded_local.contains(&slot.model)
                && let Some(first) = downloaded_local.first()
            {
                slot.model = first.clone();
            }
        }
        self
    }
}

#[derive(Deserialize)]
struct LegacyProvider {
    kind: ProviderKind,
    base_url: String,
    model: String,
    keychain_account: String,
}

#[derive(Deserialize)]
struct LegacySettings {
    dictation_stt: LegacyProvider,
    meeting_stt: LegacyProvider,
    llm: LegacyProvider,
    mode: DictationMode,
    language: Option<String>,
    dictionary: Vec<String>,
    style_rules: Vec<StyleRule>,
    history_retention_days: u32,
    hotkey: HotkeyTrigger,
}

impl LegacySettings {
    fn migrate(self) -> Settings {
        let mut providers: Vec<ProviderAccount> = Vec::new();
        let mut adopt = |legacy: &LegacyProvider| -> ModelChoice {
            let id = if legacy.keychain_account.is_empty() {
                legacy.kind.slug().to_owned()
            } else {
                legacy.keychain_account.clone()
            };
            if !providers.iter().any(|p| p.id == id) {
                let base_url = if legacy.base_url.is_empty() {
                    default_base_url(legacy.kind)
                } else {
                    legacy.base_url.clone()
                };
                providers.push(ProviderAccount {
                    id: id.clone(),
                    kind: legacy.kind,
                    name: legacy.kind.label().into(),
                    base_url,
                });
            }
            ModelChoice {
                provider_id: id,
                model: legacy.model.clone(),
            }
        };
        let dictation_stt = adopt(&self.dictation_stt);
        let meeting_stt = adopt(&self.meeting_stt);
        let llm = adopt(&self.llm);
        if !providers.iter().any(|p| p.kind == ProviderKind::Local) {
            providers.push(new_account(ProviderKind::Local, providers.clone()));
        }
        Settings {
            providers,
            dictation_stt,
            meeting_stt,
            llm,
            mode: self.mode,
            language: self.language,
            dictionary: self.dictionary,
            style_rules: self.style_rules,
            history_retention_days: self.history_retention_days,
            hotkey: self.hotkey,
            local_unload_minutes: 5,
            meetings_dir: String::new(),
            meeting_audio_retention_days: 0,
            default_template: default_template_id(),
            onboarding: OnboardingProgress::default(),
            ..default_settings()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1_GROQ: &str = r#"{
      "dictation_stt": {"kind":"groq","base_url":"https://api.groq.com/openai/v1","model":"whisper-large-v3-turbo","keychain_account":"groq"},
      "meeting_stt": {"kind":"groq","base_url":"https://api.groq.com/openai/v1","model":"whisper-large-v3-turbo","keychain_account":"groq"},
      "llm": {"kind":"groq","base_url":"https://api.groq.com/openai/v1","model":"llama-3.3-70b-versatile","keychain_account":"groq"},
      "mode":"clean","language":null,"dictionary":["Pennel"],
      "style_rules":[{"bundle_id":"com.tinyspeck.slackmacgap","style":"casual"}],
      "history_retention_days":30,"hotkey":"fn"
    }"#;

    fn ids(s: &Settings) -> Vec<&str> {
        s.providers.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn fresh_profiles_start_local_fast_and_need_onboarding() {
        let settings = default_settings();
        assert_eq!(settings.dictation_stt.provider_id, "local");
        assert_eq!(settings.meeting_stt.provider_id, "local");
        assert_eq!(settings.mode, DictationMode::Fast);
        assert_eq!(settings.appearance, AppearancePreference::System);
        assert!(!settings.dictation_pad_auto_paste);
        assert_eq!(settings.onboarding.version, 1);
        assert_eq!(settings.onboarding.status, OnboardingStatus::Pending);
        assert_eq!(settings.onboarding.resume_step, OnboardingStep::Welcome);
    }

    #[test]
    fn existing_profiles_without_ui_preferences_keep_their_configuration() {
        let mut previous = default_settings();
        previous.dictation_stt = choice(&previous.providers[0], ProviderTask::DictationStt);
        previous.meeting_stt = choice(&previous.providers[0], ProviderTask::MeetingStt);
        previous.mode = DictationMode::Clean;
        let mut json = serde_json::to_value(&previous).unwrap();
        let fields = json.as_object_mut().unwrap();
        fields.remove("appearance");
        fields.remove("onboarding");
        fields.remove("dictation_pad_auto_paste");
        let restored: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(restored.providers, previous.providers);
        assert_eq!(restored.dictation_stt, previous.dictation_stt);
        assert_eq!(restored.meeting_stt, previous.meeting_stt);
        assert_eq!(restored.llm, previous.llm);
        assert_eq!(restored.mode, DictationMode::Clean);
        assert_eq!(restored.appearance, AppearancePreference::System);
        assert_eq!(restored.onboarding.status, OnboardingStatus::Skipped);
        assert!(!restored.dictation_pad_auto_paste);
    }

    #[test]
    fn ui_preferences_roundtrip_without_resetting_onboarding() {
        let mut settings = default_settings();
        settings.appearance = AppearancePreference::Dark;
        settings.dictation_pad_auto_paste = true;
        settings.onboarding = OnboardingProgress {
            version: 1,
            status: OnboardingStatus::Pending,
            resume_step: OnboardingStep::Practice,
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored, settings);
    }

    #[test]
    fn defaults_have_groq_and_local() {
        let s = default_settings();
        assert_eq!(ids(&s), ["groq", "local"]);
        assert_eq!(
            s.dictation_stt,
            ModelChoice {
                provider_id: "local".into(),
                model: "whisper-large-v3-turbo".into()
            }
        );
        assert_eq!(s.llm.provider_id, "groq");
        assert_eq!(s.mode, DictationMode::Fast);
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
        assert_eq!(s.local_unload_minutes, 5);
        let slack = s
            .style_rules
            .iter()
            .find(|r| r.bundle_id == "com.tinyspeck.slackmacgap")
            .unwrap();
        assert_eq!(slack.style, Style::Casual);
    }

    #[test]
    fn migrates_v1_groq_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, V1_GROQ).unwrap();
        let s = Settings::load(&path).unwrap();
        assert_eq!(ids(&s), ["groq", "local"]);
        assert_eq!(s.onboarding.status, OnboardingStatus::Skipped);
        assert_eq!(s.mode, DictationMode::Clean);
        assert_eq!(
            s.dictation_stt,
            ModelChoice {
                provider_id: "groq".into(),
                model: "whisper-large-v3-turbo".into()
            }
        );
        assert_eq!(
            s.llm,
            ModelChoice {
                provider_id: "groq".into(),
                model: "llama-3.3-70b-versatile".into()
            }
        );
        assert_eq!(s.dictionary, ["Pennel"]);
        assert_eq!(s.style_rules.len(), 1);
        assert!(!dir.path().join("settings.corrupt.json").exists());
    }

    #[test]
    fn migrates_v1_mixed_providers() {
        let v1 = V1_GROQ.replacen(
            r#""llm": {"kind":"groq","base_url":"https://api.groq.com/openai/v1","model":"llama-3.3-70b-versatile","keychain_account":"groq"}"#,
            r#""llm": {"kind":"open_router","base_url":"https://openrouter.ai/api/v1","model":"m","keychain_account":"openrouter"}"#,
            1,
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, v1).unwrap();
        let s = Settings::load(&path).unwrap();
        assert_eq!(ids(&s), ["groq", "openrouter", "local"]);
        assert_eq!(
            s.llm,
            ModelChoice {
                provider_id: "openrouter".into(),
                model: "m".into()
            }
        );
    }

    #[test]
    fn migrates_v1_with_keyless_lm_studio() {
        let v1 = V1_GROQ
            .replacen(
                r#""dictation_stt": {"kind":"groq","base_url":"https://api.groq.com/openai/v1","model":"whisper-large-v3-turbo","keychain_account":"groq"}"#,
                r#""dictation_stt": {"kind":"lm_studio","base_url":"http://localhost:1234/v1","model":"w","keychain_account":""}"#,
                1,
            )
            .replacen(
                r#""meeting_stt": {"kind":"groq","base_url":"https://api.groq.com/openai/v1","model":"whisper-large-v3-turbo","keychain_account":"groq"}"#,
                r#""meeting_stt": {"kind":"open_router","base_url":"https://openrouter.ai/api/v1","model":"openai/whisper-large-v3-turbo","keychain_account":"openrouter"}"#,
                1,
            );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, v1).unwrap();
        let s = Settings::load(&path).unwrap();
        assert_eq!(ids(&s), ["lmstudio", "openrouter", "groq", "local"]);
        assert_eq!(
            s.dictation_stt,
            ModelChoice {
                provider_id: "lmstudio".into(),
                model: "w".into()
            }
        );
        assert_eq!(s.providers[0].base_url, "http://localhost:1234/v1");
        assert_eq!(s.llm.provider_id, "groq");
    }

    #[test]
    fn removed_provider_falls_back() {
        let mut s = default_settings();
        s.providers.retain(|p| p.id != "groq");
        let s = s.normalized();
        assert_eq!(
            s.dictation_stt,
            ModelChoice {
                provider_id: "local".into(),
                model: "whisper-large-v3-turbo".into()
            }
        );
        assert_eq!(s.llm.provider_id, "groq");
    }

    #[test]
    fn fallback_to_local_prefers_downloaded_model() {
        let mut s = default_settings();
        s.providers.retain(|p| p.id != "groq");
        let s = s.normalized_with(&["whisper-small-q5_1".to_owned()]);
        assert_eq!(
            s.dictation_stt,
            ModelChoice {
                provider_id: "local".into(),
                model: "whisper-small-q5_1".into()
            }
        );
    }

    #[test]
    fn local_choice_with_deleted_model_moves_to_downloaded_one() {
        let mut s = default_settings();
        s.dictation_stt = ModelChoice {
            provider_id: "local".into(),
            model: "whisper-large-v3-turbo-q5_0".into(),
        };
        let s = s.normalized_with(&["whisper-small-q5_1".to_owned()]);
        assert_eq!(s.dictation_stt.model, "whisper-small-q5_1");
    }

    #[test]
    fn v2_settings_without_meeting_fields_load_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut value = serde_json::to_value(default_settings()).unwrap();
        let map = value.as_object_mut().unwrap();
        map.remove("meetings_dir");
        map.remove("meeting_audio_retention_days");
        map.remove("default_template");
        std::fs::write(&path, value.to_string()).unwrap();
        let s = Settings::load(&path).unwrap();
        assert_eq!(s.meetings_dir, "");
        assert_eq!(s.meeting_audio_retention_days, 0);
        assert_eq!(s.default_template, "algemeen");
        assert!(!dir.path().join("settings.corrupt.json").exists());
    }

    #[test]
    fn resolve_finds_account() {
        let s = default_settings();
        let (account, model) = s.resolve(&s.llm).unwrap();
        assert_eq!(account.kind, ProviderKind::Groq);
        assert_eq!(model, "llama-3.3-70b-versatile");
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Settings::load(&dir.path().join("settings.json")).unwrap(),
            default_settings()
        );
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = default_settings();
        s.dictionary = vec!["Pennel".into(), "Q8".into()];
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
