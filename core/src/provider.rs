use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAi,
    OpenRouter,
    Groq,
    Mistral,
    Ollama,
    LmStudio,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ProviderTask {
    DictationStt,
    MeetingStt,
    Llm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// Keychain account the Swift shell stores this provider's key under; empty means no key needed.
    pub keychain_account: String,
}

#[uniffi::export]
pub fn preset(kind: ProviderKind, task: ProviderTask) -> ProviderConfig {
    let is_stt = task != ProviderTask::Llm;
    let (base_url, stt_model, llm_model, account) = match kind {
        ProviderKind::OpenAi => ("https://api.openai.com/v1", "gpt-4o-mini-transcribe", "gpt-4.1-mini", "openai"),
        ProviderKind::OpenRouter => (
            "https://openrouter.ai/api/v1",
            "openai/whisper-large-v3-turbo",
            "google/gemini-3.5-flash-lite",
            "openrouter",
        ),
        ProviderKind::Groq => (
            "https://api.groq.com/openai/v1",
            "whisper-large-v3-turbo",
            "llama-3.3-70b-versatile",
            "groq",
        ),
        ProviderKind::Mistral => ("https://api.mistral.ai/v1", "voxtral-mini-latest", "mistral-small-latest", "mistral"),
        ProviderKind::Ollama => ("http://localhost:11434/v1", "", "gemma3:4b", ""),
        ProviderKind::LmStudio => ("http://localhost:1234/v1", "", "", ""),
        ProviderKind::Custom => ("", "", "", "custom"),
    };
    ProviderConfig {
        kind,
        base_url: base_url.into(),
        model: if is_stt { stt_model } else { llm_model }.into(),
        keychain_account: account.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_groq_dictation() {
        let p = preset(ProviderKind::Groq, ProviderTask::DictationStt);
        assert_eq!(p.base_url, "https://api.groq.com/openai/v1");
        assert_eq!(p.model, "whisper-large-v3-turbo");
        assert_eq!(p.keychain_account, "groq");
    }

    #[test]
    fn preset_ollama_needs_no_key() {
        let p = preset(ProviderKind::Ollama, ProviderTask::Llm);
        assert_eq!(p.base_url, "http://localhost:11434/v1");
        assert_eq!(p.keychain_account, "");
    }
}
