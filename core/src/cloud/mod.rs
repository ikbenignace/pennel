// Consumed by the sync and hosted-model modules that follow.
#![allow(dead_code)]

pub mod account;
pub mod crypto;
pub mod diarize;
pub mod http;
pub mod link;
pub mod relay;
pub mod setup;
pub mod sync;

const RELEASE_URL: &str = "https://api.pennel.app";
const STAGING_URL: &str = "https://api-staging.pennel.app";

pub const REFRESH_ACCOUNT: &str = "pennel-cloud-refresh";
pub const VAULT_KEY_ACCOUNT: &str = "pennel-cloud-vault-key";
pub(crate) const VAULT_OWNER_ACCOUNT: &str = "pennel-cloud-vault-owner";
pub(crate) const BACKUP_PENDING_ACCOUNT: &str = "pennel-cloud-backup-pending";
pub(crate) const VAULT_VALIDATION_ACCOUNT: &str = "pennel-cloud-vault-validation";

pub(crate) fn store_verified(
    store: &dyn SecretStore,
    account: &str,
    value: &str,
) -> Result<(), crate::PennelError> {
    let old = zeroize::Zeroizing::new(store.get(account.into()));
    store.set(account.into(), Some(value.into()));
    if store.get(account.into()).as_deref() != Some(value) {
        store.set(account.into(), (*old).clone());
        return Err(crate::PennelError::Storage { message: "The device credential store could not save sync access. Try again after unlocking this device.".into() });
    }
    Ok(())
}

/// Host credential store. `set` with `None` deletes the entry.
#[uniffi::export(with_foreign)]
pub trait SecretStore: Send + Sync {
    fn get(&self, account: String) -> Option<String>;
    fn set(&self, account: String, value: Option<String>);
}

pub fn base_url(preview: bool) -> String {
    match std::env::var("PENNEL_CLOUD_URL") {
        Ok(url) if !url.trim().is_empty() => url.trim().trim_end_matches('/').to_string(),
        _ if preview => STAGING_URL.to_string(),
        _ => RELEASE_URL.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::engine::PennelEngine;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub(crate) struct MemorySecrets(Mutex<HashMap<String, String>>);

    impl SecretStore for MemorySecrets {
        fn get(&self, account: String) -> Option<String> {
            self.0.lock().unwrap().get(&account).cloned()
        }
        fn set(&self, account: String, value: Option<String>) {
            let mut map = self.0.lock().unwrap();
            match value {
                Some(v) => map.insert(account, v),
                None => map.remove(&account),
            };
        }
    }

    // Env vars are process-wide; one test owns every assertion that touches them.
    #[test]
    fn base_url_respects_env_and_preview() {
        unsafe { std::env::remove_var("PENNEL_CLOUD_URL") };
        assert_eq!(base_url(false), "https://api.pennel.app");
        assert_eq!(base_url(true), "https://api-staging.pennel.app");
        unsafe { std::env::set_var("PENNEL_CLOUD_URL", "http://127.0.0.1:9/") };
        assert_eq!(base_url(false), "http://127.0.0.1:9");
        assert_eq!(base_url(true), "http://127.0.0.1:9");
        unsafe { std::env::remove_var("PENNEL_CLOUD_URL") };
    }

    #[test]
    fn secret_store_roundtrip_through_engine() {
        let dir = tempfile::tempdir().unwrap();
        let engine = PennelEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(engine.secret_store().is_none());
        engine
            .set_secret_store(Arc::new(MemorySecrets::default()))
            .unwrap();
        let store = engine.secret_store().unwrap();
        store.set(REFRESH_ACCOUNT.into(), Some("token".into()));
        assert_eq!(store.get(REFRESH_ACCOUNT.into()).as_deref(), Some("token"));
        store.set(REFRESH_ACCOUNT.into(), None);
        assert_eq!(store.get(REFRESH_ACCOUNT.into()), None);
    }
}
