//! API keys in the OS credential store: Windows Credential Manager, Secret Service on Linux,
//! the Keychain on macOS. Each key is read at most once per launch.

use std::collections::HashMap;
use std::sync::Mutex;

use pennel_core::{ApiKeys, ProviderAccount};

const SERVICE: &str = "app.pennel.desktop";

#[derive(Default)]
pub struct Secrets {
    cache: Mutex<HashMap<String, Option<String>>>,
}

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())
}

impl Secrets {
    pub fn get(&self, account: &str) -> Option<String> {
        if account.is_empty() {
            return None;
        }
        if let Some(cached) = self.cache.lock().unwrap().get(account) {
            return cached.clone();
        }
        let value = entry(account)
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
        let entry = entry(account)?;
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
}
