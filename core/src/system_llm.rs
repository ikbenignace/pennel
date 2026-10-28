use crate::error::PennelError;
use crate::llm::ChatMessage;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
pub struct LanguageModelMessage {
    pub role: String,
    pub content: String,
}

/// On-device text generation supplied by the host. No endpoint or API key crosses this boundary.
#[uniffi::export(with_foreign)]
pub trait SystemLanguageModel: Send + Sync {
    fn complete(
        &self,
        messages: Vec<LanguageModelMessage>,
        timeout_secs: u64,
    ) -> Result<String, PennelError>;
}

// A conservative planning budget for the 4,096-token macOS 26 model. Byte counts are
// a heuristic, not token counts; the native adapter also checks the actual context limit.
pub(crate) const APPLE_INPUT_BYTES: usize = 6_000;

pub(crate) fn input_bytes(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|message| message.content.len() + 64)
        .sum()
}

pub(crate) fn length_error() -> PennelError {
    PennelError::InvalidResponse {
        message: "This text is too long for Apple Intelligence. Shorten it or choose a text model with a larger context window.".into(),
    }
}

pub(crate) fn missing_model() -> PennelError {
    PennelError::InvalidResponse {
        message: "Apple Intelligence is unavailable on this computer. Choose another text model or check Apple Intelligence in System Settings.".into(),
    }
}

pub(crate) fn complete(
    model: Option<&dyn SystemLanguageModel>,
    messages: &[ChatMessage],
    timeout_secs: u64,
) -> Result<String, PennelError> {
    let model = model.ok_or_else(missing_model)?;
    if input_bytes(messages) > APPLE_INPUT_BYTES {
        return Err(length_error());
    }
    let text = model.complete(
        messages
            .iter()
            .map(|message| LanguageModelMessage {
                role: message.role.into(),
                content: message.content.clone(),
            })
            .collect(),
        timeout_secs,
    )?;
    let text = text.trim();
    if text.is_empty() {
        return Err(PennelError::InvalidResponse {
            message: "Apple Intelligence returned no text. Try again.".into(),
        });
    }
    Ok(text.to_owned())
}
