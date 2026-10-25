use serde::{Deserialize, Serialize};

use crate::{OnboardingStep, ProviderKind, ProviderTask, Settings, requires_key, supports};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum SetupAccess {
    Available,
    PermissionRequired,
    Unavailable,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SetupFacts {
    pub microphone: SetupAccess,
    pub global_hotkey: SetupAccess,
    pub external_paste: SetupAccess,
    pub apple_speech: SetupAccess,
    pub keyed_providers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct SetupReadiness {
    pub configured: bool,
    pub can_record: bool,
    pub can_use_shortcut: bool,
    pub can_paste_externally: bool,
    pub reason: Option<String>,
    pub resume_step: OnboardingStep,
}

#[uniffi::export]
pub fn dictation_readiness(
    settings: Settings,
    facts: SetupFacts,
    downloaded_models: Vec<String>,
) -> SetupReadiness {
    readiness(
        &settings,
        &facts,
        &downloaded_models,
        ProviderTask::DictationStt,
    )
}

#[uniffi::export]
pub fn meeting_readiness(
    settings: Settings,
    facts: SetupFacts,
    downloaded_models: Vec<String>,
) -> SetupReadiness {
    readiness(
        &settings,
        &facts,
        &downloaded_models,
        ProviderTask::MeetingStt,
    )
}

fn blocked(configured: bool, reason: &str, resume_step: OnboardingStep) -> SetupReadiness {
    SetupReadiness {
        configured,
        can_record: false,
        can_use_shortcut: false,
        can_paste_externally: false,
        reason: Some(reason.into()),
        resume_step,
    }
}

fn readiness(
    settings: &Settings,
    facts: &SetupFacts,
    downloaded_models: &[String],
    task: ProviderTask,
) -> SetupReadiness {
    let choice = if task == ProviderTask::MeetingStt {
        &settings.meeting_stt
    } else {
        &settings.dictation_stt
    };
    let Some((account, model)) = settings.resolve(choice) else {
        return blocked(
            false,
            "Kies eerst een spraakherkenning.",
            OnboardingStep::Engine,
        );
    };
    if !supports(account.kind, task) {
        return blocked(
            false,
            "Deze provider ondersteunt geen spraakherkenning.",
            OnboardingStep::Engine,
        );
    }
    if model.trim().is_empty() {
        return blocked(false, "Kies een spraakmodel.", OnboardingStep::Engine);
    }
    if requires_key(account.kind) && !facts.keyed_providers.contains(&account.id) {
        return blocked(
            false,
            "Voeg de API-sleutel van je provider toe.",
            OnboardingStep::Engine,
        );
    }
    if account.kind == ProviderKind::Local && !downloaded_models.contains(&model) {
        return blocked(
            false,
            "Download eerst het gekozen spraakmodel.",
            OnboardingStep::Engine,
        );
    }
    if account.kind == ProviderKind::AppleSpeech {
        match facts.apple_speech {
            SetupAccess::Available => {}
            SetupAccess::Unsupported => {
                return blocked(
                    true,
                    "Apple-dicteren vereist een geschikte Mac met macOS 26 of nieuwer.",
                    OnboardingStep::Engine,
                );
            }
            SetupAccess::PermissionRequired => {
                return blocked(
                    true,
                    "Geef Pennel toegang tot spraakherkenning.",
                    OnboardingStep::Permissions,
                );
            }
            SetupAccess::Unavailable | SetupAccess::Unknown => {
                return blocked(
                    true,
                    "Apple-dicteren is nog niet beschikbaar. Controleer spraakherkenning in de systeeminstellingen.",
                    OnboardingStep::Permissions,
                );
            }
        }
    }
    match facts.microphone {
        // Windows and Linux cannot always query microphone permission before the first capture.
        SetupAccess::Available | SetupAccess::Unknown => {}
        SetupAccess::PermissionRequired => {
            return blocked(
                true,
                "Geef Pennel toegang tot de microfoon.",
                OnboardingStep::Permissions,
            );
        }
        SetupAccess::Unavailable | SetupAccess::Unsupported => {
            return blocked(
                true,
                "Microfoon niet beschikbaar. Controleer je invoerapparaat.",
                OnboardingStep::Permissions,
            );
        }
    }
    SetupReadiness {
        configured: true,
        can_record: true,
        can_use_shortcut: facts.global_hotkey == SetupAccess::Available,
        can_paste_externally: facts.external_paste == SetupAccess::Available,
        reason: None,
        resume_step: OnboardingStep::Practice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelChoice, default_settings, new_account};

    fn facts() -> SetupFacts {
        SetupFacts {
            microphone: SetupAccess::Available,
            global_hotkey: SetupAccess::Available,
            external_paste: SetupAccess::Available,
            apple_speech: SetupAccess::Unsupported,
            keyed_providers: Vec::new(),
        }
    }

    #[test]
    fn a_partial_local_model_requires_the_engine_step() {
        let result = dictation_readiness(default_settings(), facts(), Vec::new());
        assert!(!result.configured);
        assert!(!result.can_record);
        assert_eq!(result.resume_step, OnboardingStep::Engine);
        assert!(result.reason.unwrap().contains("Download"));
    }

    #[test]
    fn manual_capture_does_not_require_a_global_hotkey_or_external_paste() {
        let settings = default_settings();
        let mut facts = facts();
        facts.microphone = SetupAccess::Unknown;
        facts.global_hotkey = SetupAccess::Unsupported;
        facts.external_paste = SetupAccess::Unsupported;
        let result =
            dictation_readiness(settings.clone(), facts, vec![settings.dictation_stt.model]);
        assert!(result.configured);
        assert!(result.can_record);
        assert!(!result.can_use_shortcut);
        assert!(!result.can_paste_externally);
        assert_eq!(result.resume_step, OnboardingStep::Practice);
    }

    #[test]
    fn denied_microphone_access_requires_permission_recovery() {
        let settings = default_settings();
        let mut facts = facts();
        facts.microphone = SetupAccess::PermissionRequired;
        let result =
            dictation_readiness(settings.clone(), facts, vec![settings.dictation_stt.model]);
        assert!(result.configured);
        assert!(!result.can_record);
        assert_eq!(result.resume_step, OnboardingStep::Permissions);
    }

    #[test]
    fn a_remote_model_requires_its_own_key_but_not_an_llm_key() {
        let mut settings = default_settings();
        settings.dictation_stt.provider_id = "groq".into();
        let missing = dictation_readiness(settings.clone(), facts(), Vec::new());
        assert!(!missing.configured);
        let mut facts = facts();
        facts.keyed_providers.push("groq".into());
        let ready = dictation_readiness(settings, facts, Vec::new());
        assert!(ready.can_record);
    }

    #[test]
    fn apple_permission_and_platform_unavailability_are_distinct() {
        let mut settings = default_settings();
        let apple = new_account(ProviderKind::AppleSpeech, settings.providers.clone());
        settings.dictation_stt = ModelChoice {
            provider_id: apple.id.clone(),
            model: "dictation".into(),
        };
        settings.providers.push(apple);
        let unsupported = dictation_readiness(settings.clone(), facts(), Vec::new());
        assert!(!unsupported.can_record);
        assert_eq!(unsupported.resume_step, OnboardingStep::Engine);
        let mut facts = facts();
        facts.apple_speech = SetupAccess::PermissionRequired;
        let needs_permission = dictation_readiness(settings, facts, Vec::new());
        assert!(needs_permission.configured);
        assert!(!needs_permission.can_record);
        assert_eq!(needs_permission.resume_step, OnboardingStep::Permissions);
    }

    #[test]
    fn meeting_setup_uses_the_meeting_slot() {
        let mut settings = default_settings();
        settings.meeting_stt.provider_id = "groq".into();
        let downloaded = vec![settings.dictation_stt.model.clone()];
        assert!(dictation_readiness(settings.clone(), facts(), downloaded.clone()).can_record);
        assert!(!meeting_readiness(settings, facts(), downloaded).configured);
    }
}
