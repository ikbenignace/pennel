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
    #[error("Sign in to Pennel Cloud under Settings → Account.")]
    NotSignedIn,
    #[error("{message}")]
    Cloud { code: String, message: String },
    #[error("Enter your recovery code to turn on sync.")]
    VaultLocked,
    #[error("This processor is too old for local models. Choose a cloud provider.")]
    UnsupportedProcessor,
}

impl PennelError {
    pub(crate) fn storage(err: impl std::fmt::Display) -> Self {
        PennelError::Storage {
            message: err.to_string(),
        }
    }

    #[cfg_attr(feature = "local-models", allow(dead_code))]
    pub(crate) fn local_models_unavailable() -> Self {
        PennelError::InvalidResponse {
            message: "Local models are unavailable on this device.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_error_displays_only_its_message() {
        for message in ["Je tegoed is op.", "Your credit has run out."] {
            let error = PennelError::Cloud {
                code: "quota".into(),
                message: message.into(),
            };
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn cloud_login_errors_are_canonical_english() {
        assert_eq!(
            PennelError::NotSignedIn.to_string(),
            "Sign in to Pennel Cloud under Settings → Account."
        );
        assert_eq!(
            PennelError::VaultLocked.to_string(),
            "Enter your recovery code to turn on sync."
        );
    }
}
