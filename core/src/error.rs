#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum PennelError {
    #[error("no speech detected")]
    NoSpeech,
    #[error("missing api key for {task}")]
    MissingApiKey { task: String },
    #[error("http {status}: {message}")]
    Http { status: u16, message: String },
    #[error("network error: {message}")]
    Network { message: String },
    #[error("invalid response: {message}")]
    InvalidResponse { message: String },
    #[error("storage error: {message}")]
    Storage { message: String },
    #[error("not found")]
    NotFound,
    #[error("cancelled")]
    Cancelled,
    #[error("model not downloaded")]
    ModelNotDownloaded,
}

impl PennelError {
    pub(crate) fn storage(err: impl std::fmt::Display) -> Self {
        PennelError::Storage { message: err.to_string() }
    }
}
