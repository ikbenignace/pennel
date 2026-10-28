//! API keys in the OS credential store: Windows Credential Manager, Secret Service on Linux,
//! the Keychain on macOS. Normal lookups are cached for the launch.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use pennel_core::{ApiKeys, ProviderAccount, SecretStore};

use crate::diagnostics::Log;

const LOG: Log = Log::new("secrets");

pub struct Secrets {
    service: &'static str,
    cache: Mutex<HashMap<String, Option<String>>>,
    revision: AtomicU64,
    #[cfg(test)]
    in_memory: bool,
    #[cfg(test)]
    fail_writes: std::sync::atomic::AtomicBool,
}

impl Default for Secrets {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Secrets {
    pub fn new(preview: bool) -> Self {
        let service = match (cfg!(target_os = "macos"), preview) {
            (true, false) => "dev.pennel.Pennel",
            (true, true) => "dev.pennel.Pennel.preview",
            (false, false) => "app.pennel.desktop",
            (false, true) => "app.pennel.desktop.preview",
        };
        Self {
            service,
            cache: Mutex::new(HashMap::new()),
            revision: AtomicU64::new(0),
            #[cfg(test)]
            in_memory: false,
            #[cfg(test)]
            fail_writes: std::sync::atomic::AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            in_memory: true,
            ..Self::new(true)
        }
    }

    fn entry(&self, account: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(self.service, account).map_err(|error| error.to_string())
    }

    pub fn get(&self, account: &str) -> Option<String> {
        if account.is_empty() {
            return None;
        }
        if let Some(cached) = self.cache.lock().unwrap().get(account) {
            return cached.clone();
        }
        #[cfg(test)]
        if self.in_memory {
            return None;
        }
        let value = self
            .entry(account)
            .ok()
            .and_then(|e| e.get_password().ok())
            .filter(|v| !v.is_empty());
        self.cache
            .lock()
            .unwrap()
            .insert(account.to_owned(), value.clone());
        value
    }

    pub fn set(&self, account: &str, value: &str) -> Result<(), String> {
        if account.is_empty() {
            return Err("no account".into());
        }
        let value = value.trim();
        #[cfg(test)]
        if self.in_memory {
            if self.fail_writes.load(std::sync::atomic::Ordering::SeqCst) {
                return Err("simulated".into());
            }
            self.cache.lock().unwrap().insert(
                account.to_owned(),
                (!value.is_empty()).then(|| value.to_owned()),
            );
            self.revision.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        let entry = self.entry(account)?;
        if value.is_empty() {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(e.to_string()),
            }
        } else {
            entry.set_password(value).map_err(|e| e.to_string())?;
        }
        self.cache.lock().unwrap().insert(
            account.to_owned(),
            (!value.is_empty()).then(|| value.to_owned()),
        );
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    pub fn keys(&self, providers: &[ProviderAccount]) -> ApiKeys {
        ApiKeys {
            by_provider: providers
                .iter()
                .filter_map(|p| self.get(&p.id).map(|key| (p.id.clone(), key)))
                .collect(),
        }
    }

    #[cfg(any(target_os = "macos", test))]
    pub fn migrate_legacy(&self, providers: &[ProviderAccount]) -> Result<(), String> {
        if self.service.ends_with(".preview") {
            return Err("The preview does not import saved API keys.".into());
        }
        #[cfg(target_os = "macos")]
        for provider in providers {
            crate::macos::migrate_legacy_key(&provider.id)?;
            self.cache.lock().unwrap().remove(&provider.id);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = providers;
        Ok(())
    }
}

impl SecretStore for Secrets {
    fn get(&self, account: String) -> Option<String> {
        Secrets::get(self, &account)
    }

    fn set(&self, account: String, value: Option<String>) {
        if Secrets::set(self, &account, value.as_deref().unwrap_or("")).is_err() {
            LOG.error("credential store write failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn verification_revision_changes_only_after_successful_credential_writes() {
        let secrets = Secrets::in_memory();
        let initial = secrets.revision();
        secrets.set("provider", "first synthetic key").unwrap();
        let first = secrets.revision();
        assert!(first > initial);
        secrets
            .set("provider", "replacement synthetic key")
            .unwrap();
        let replacement = secrets.revision();
        assert!(replacement > first);
        secrets.fail_writes.store(true, Ordering::SeqCst);
        assert!(secrets.set("provider", "rejected synthetic key").is_err());
        assert_eq!(secrets.revision(), replacement);
    }

    #[test]
    fn secrets_impl_roundtrip() {
        let secrets = Secrets::in_memory();
        let store: &dyn SecretStore = &secrets;
        store.set("pennel-cloud-refresh".into(), Some("token".into()));
        assert_eq!(
            store.get("pennel-cloud-refresh".into()).as_deref(),
            Some("token")
        );
        store.set("pennel-cloud-refresh".into(), None);
        assert_eq!(store.get("pennel-cloud-refresh".into()), None);
    }

    #[test]
    fn shared_testing_secrets_are_a_store() {
        let dir = tempfile::tempdir().unwrap();
        let shared = crate::state::Shared::testing(dir.path().to_path_buf());
        let store: Arc<dyn SecretStore> = shared.secrets.clone();
        store.set("a".into(), Some("b".into()));
        assert_eq!(shared.secrets.get("a").as_deref(), Some("b"));
        store.set("a".into(), None);
        assert_eq!(shared.secrets.get("a"), None);
    }

    #[test]
    fn failed_write_keeps_the_old_value_and_does_not_panic() {
        let secrets = Secrets::in_memory();
        secrets.set("acct", "old").unwrap();
        secrets
            .fail_writes
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(secrets.set("acct", "new").is_err());
        let store: &dyn SecretStore = &secrets;
        store.set("acct".into(), Some("new".into()));
        store.set("acct".into(), None);
        assert_eq!(secrets.get("acct").as_deref(), Some("old"));
    }
}
