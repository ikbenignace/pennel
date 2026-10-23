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
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ProviderTask {
    DictationStt,
    MeetingStt,
    Llm,
}

/// A configured provider. Its `id` doubles as the Keychain account for its API key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ProviderAccount {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: String,
}

/// Resolved endpoint for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
}

impl ProviderKind {
    pub(crate) fn slug(self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "openai",
            ProviderKind::OpenRouter => "openrouter",
            ProviderKind::Groq => "groq",
            ProviderKind::Mistral => "mistral",
            ProviderKind::Ollama => "ollama",
            ProviderKind::LmStudio => "lmstudio",
            ProviderKind::Custom => "custom",
            ProviderKind::Local => "local",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "OpenAI",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::Groq => "Groq",
            ProviderKind::Mistral => "Mistral",
            ProviderKind::Ollama => "Ollama",
            ProviderKind::LmStudio => "LM Studio",
            ProviderKind::Custom => "Eigen server",
            ProviderKind::Local => "Lokaal",
        }
    }
}

#[uniffi::export]
pub fn requires_key(kind: ProviderKind) -> bool {
    matches!(kind, ProviderKind::OpenAi | ProviderKind::OpenRouter | ProviderKind::Groq | ProviderKind::Mistral)
}

#[uniffi::export]
pub fn supports(kind: ProviderKind, task: ProviderTask) -> bool {
    !(kind == ProviderKind::Local && task == ProviderTask::Llm)
}

#[uniffi::export]
pub fn default_base_url(kind: ProviderKind) -> String {
    match kind {
        ProviderKind::OpenAi => "https://api.openai.com/v1",
        ProviderKind::OpenRouter => "https://openrouter.ai/api/v1",
        ProviderKind::Groq => "https://api.groq.com/openai/v1",
        ProviderKind::Mistral => "https://api.mistral.ai/v1",
        ProviderKind::Ollama => "http://localhost:11434/v1",
        ProviderKind::LmStudio => "http://localhost:1234/v1",
        ProviderKind::Custom | ProviderKind::Local => "",
    }
    .into()
}

#[uniffi::export]
pub fn default_model(kind: ProviderKind, task: ProviderTask) -> String {
    let is_stt = task != ProviderTask::Llm;
    let (stt, llm) = match kind {
        ProviderKind::OpenAi => ("gpt-4o-mini-transcribe", "gpt-4.1-mini"),
        ProviderKind::OpenRouter => ("openai/whisper-large-v3-turbo", "google/gemini-3.5-flash-lite"),
        ProviderKind::Groq => ("whisper-large-v3-turbo", "llama-3.3-70b-versatile"),
        ProviderKind::Mistral => ("voxtral-mini-latest", "mistral-small-latest"),
        ProviderKind::Ollama => ("", "gemma3:4b"),
        ProviderKind::LmStudio | ProviderKind::Custom => ("", ""),
        ProviderKind::Local => ("whisper-large-v3-turbo-q5_0", ""),
    };
    if is_stt { stt } else { llm }.into()
}

#[uniffi::export]
pub fn new_account(kind: ProviderKind, existing: Vec<ProviderAccount>) -> ProviderAccount {
    let base = kind.slug();
    let id = std::iter::once(base.to_owned())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(|candidate| !existing.iter().any(|a| &a.id == candidate))
        .expect("unbounded iterator");
    ProviderAccount { id, kind, name: kind.label().into(), base_url: default_base_url(kind) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groq_defaults() {
        assert_eq!(default_base_url(ProviderKind::Groq), "https://api.groq.com/openai/v1");
        assert_eq!(default_model(ProviderKind::Groq, ProviderTask::DictationStt), "whisper-large-v3-turbo");
        assert_eq!(default_model(ProviderKind::Local, ProviderTask::DictationStt), "whisper-large-v3-turbo-q5_0");
    }

    #[test]
    fn requires_key_only_for_cloud() {
        assert!(requires_key(ProviderKind::Groq));
        assert!(requires_key(ProviderKind::OpenAi));
        for kind in [ProviderKind::Custom, ProviderKind::Ollama, ProviderKind::LmStudio, ProviderKind::Local] {
            assert!(!requires_key(kind), "{kind:?}");
        }
    }

    #[test]
    fn new_account_ids_are_unique() {
        let first = new_account(ProviderKind::Groq, vec![]);
        assert_eq!(first.id, "groq");
        assert_eq!(first.name, "Groq");
        assert_eq!(first.base_url, "https://api.groq.com/openai/v1");
        let second = new_account(ProviderKind::Groq, vec![first.clone()]);
        assert_eq!(second.id, "groq-2");
        let third = new_account(ProviderKind::Groq, vec![first, second]);
        assert_eq!(third.id, "groq-3");
    }

    #[test]
    fn local_cannot_do_llm() {
        assert!(supports(ProviderKind::Local, ProviderTask::DictationStt));
        assert!(!supports(ProviderKind::Local, ProviderTask::Llm));
        assert!(supports(ProviderKind::Groq, ProviderTask::Llm));
    }
}
