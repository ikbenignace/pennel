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
    #[error("Log in bij Pennel Cloud in Instellingen → Account.")]
    NotSignedIn,
    #[error("{message}")]
    Cloud { code: String, message: String },
    #[error("Voer je herstelcode in om sync aan te zetten.")]
    VaultLocked,
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
            message: "Lokale modellen zijn niet beschikbaar op dit toestel.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_error_displays_only_its_message() {
        let error = PennelError::Cloud {
            code: "quota".into(),
            message: "Je tegoed is op.".into(),
        };
        assert_eq!(error.to_string(), "Je tegoed is op.");
    }

    #[test]
    fn cloud_login_errors_display_in_dutch() {
        assert_eq!(
            PennelError::NotSignedIn.to_string(),
            "Log in bij Pennel Cloud in Instellingen → Account."
        );
        assert_eq!(
            PennelError::VaultLocked.to_string(),
            "Voer je herstelcode in om sync aan te zetten."
        );
    }
}
