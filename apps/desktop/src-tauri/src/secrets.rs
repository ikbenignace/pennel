//! API keys in the OS credential store: Windows Credential Manager, Secret Service on Linux,
//! the Keychain on macOS. Normal lookups are cached for the launch.

use std::collections::HashMap;
use std::sync::Mutex;

use pennel_core::{ApiKeys, ProviderAccount};

pub struct Secrets {
    service: &'static str,
    cache: Mutex<HashMap<String, Option<String>>>,
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
            return Err("geen account".into());
        }
        let entry = self.entry(account)?;
        let value = value.trim();
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
        Ok(())
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
            return Err("De testversie neemt geen opgeslagen API-sleutels over.".into());
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
