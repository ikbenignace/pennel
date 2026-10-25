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
    #[error("cloud error {code}: {message}")]
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
