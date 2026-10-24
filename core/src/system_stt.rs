use crate::error::PennelError;

/// Speech recognition the host OS offers on-device (Apple's dictation model on macOS), implemented
/// by the app because only the platform UI layer can reach it.
#[uniffi::export(with_foreign)]
pub trait SystemTranscriber: Send + Sync {
    /// `language` is an ISO code like "nl", or `None` for the user's preferred language.
    fn transcribe(
        &self,
        samples: Vec<f32>,
        sample_rate: u32,
        language: Option<String>,
    ) -> Result<String, PennelError>;
}
