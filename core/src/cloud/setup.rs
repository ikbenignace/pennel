use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::cloud::crypto::{self, VaultKey};
use crate::cloud::sync::api::RemoteVault;
use crate::cloud::{BACKUP_PENDING_ACCOUNT, VAULT_KEY_ACCOUNT, store_verified};
use crate::{PennelEngine, PennelError};

const PREPARING: &str = "pennel-cloud-vault-preparing-v1";

fn restore_entry(
    store: &dyn super::SecretStore,
    name: &str,
    old: &Option<String>,
) -> Result<(), PennelError> {
    if let Some(value) = old {
        store_verified(store, name, value)
    } else {
        store.set(name.into(), None);
        if store.get(name.into()).is_some() {
            Err(PennelError::Storage {
                message: "The credential store could not restore the previous sync setup.".into(),
            })
        } else {
            Ok(())
        }
    }
}

fn recover_preparation(engine: &PennelEngine, vault: &RemoteVault) -> Result<(), PennelError> {
    let Some(store) = engine.secret_store() else {
        return Ok(());
    };
    let Some(encoded) = store.get(PREPARING.into()) else {
        return Ok(());
    };
    let encoded = Zeroizing::new(encoded);
    let candidate: serde_json::Value =
        serde_json::from_str(&encoded).map_err(PennelError::storage)?;
    if candidate["origin"].as_str() != Some(engine.cloud_session()?.base_url()) {
        return Ok(());
    }
    let Some(key) = candidate["key"].as_str().and_then(VaultKey::from_b64) else {
        return Ok(());
    };
    if candidate["generation"].as_u64() != Some(u64::from(vault.generation))
        || !matches_vault(&key, vault)
    {
        return Ok(());
    }
    let already_active = engine.settings().cloud_sync
        && engine
            .vault_key()
            .is_some_and(|current| crypto::vault_check(&current) == crypto::vault_check(&key));
    if !already_active {
        store_verified(store.as_ref(), VAULT_KEY_ACCOUNT, &key.to_b64())?;
        store_verified(
            store.as_ref(),
            BACKUP_PENDING_ACCOUNT,
            &format!("{}:{}", crypto::vault_check(&key), vault.generation),
        )?;
        engine.with_sync_state(|state| {
            state.use_vault(&crypto::vault_check(&key))?;
            state.set_generation(vault.generation)
        })?;
        let mut settings = engine.settings();
        settings.cloud_sync = false;
        engine.update_settings_locked(settings)?;
    }
    store.set(PREPARING.into(), None);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "snake_case")]
pub enum CloudSyncSetupPhase {
    SignedOut,
    NewLibrary,
    Locked,
    Ready,
    Paused,
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct CloudSyncSetup {
    pub phase: CloudSyncSetupPhase,
    pub sync_enabled: bool,
    pub generation: Option<u32>,
    pub can_approve_devices: bool,
    pub uploaded: u32,
    pub downloaded: u32,
    pub conflicts: u32,
    pub last_success_ms: Option<i64>,
}

pub(crate) fn matches_vault(key: &VaultKey, vault: &RemoteVault) -> bool {
    vault.wrapped.is_some()
        && vault.kdf.as_ref().and_then(|kdf| kdf.key_check.as_deref())
            == Some(crypto::vault_check(key).as_str())
}

pub(crate) fn validation_marker(key: &VaultKey, vault: &RemoteVault) -> String {
    let bytes = serde_json::to_vec(&(vault.generation, &vault.wrapped, &vault.kdf))
        .expect("serializable vault metadata");
    format!(
        "{}:{}",
        crypto::vault_check(key),
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn matches_stored_vault(engine: &PennelEngine, key: &VaultKey, vault: &RemoteVault) -> bool {
    matches_vault(key, vault)
        || (vault.wrapped.is_some()
            && vault.kdf.as_ref().is_some_and(|k| k.key_check.is_none())
            && engine
                .secret_store()
                .and_then(|store| store.get(super::VAULT_VALIDATION_ACCOUNT.into()))
                .as_deref()
                == Some(validation_marker(key, vault).as_str()))
}

#[uniffi::export]
impl PennelEngine {
    /// Probes access separately from the sync switch, including a locked library while paused.
    pub fn cloud_sync_setup(&self) -> Result<CloudSyncSetup, PennelError> {
        let _gate = self.sync_gate.lock().unwrap();
        let report = self.cloud_sync_last();
        let enabled = self.settings().cloud_sync;
        let mut result = CloudSyncSetup {
            phase: CloudSyncSetupPhase::SignedOut,
            sync_enabled: enabled,
            generation: None,
            can_approve_devices: false,
            uploaded: report.uploaded,
            downloaded: report.downloaded,
            conflicts: report.conflicts,
            last_success_ms: report.last_success_ms,
        };
        if !self.cloud_signed_in() {
            return Ok(result);
        }
        if super::link::has_pending(self) {
            result.phase = CloudSyncSetupPhase::Paused;
            result.sync_enabled = false;
            return Ok(result);
        }
        let session = self.cloud_session()?;
        let identity_epoch = session.generation();
        let vault = self.signed_in_sync_api()?.vault()?;
        let _identity = self.inference_capture_gate();
        if session.generation() != identity_epoch {
            return Err(PennelError::Cancelled);
        }
        recover_preparation(self, &vault)?;
        let enabled = self.settings().cloud_sync;
        result.sync_enabled = enabled;
        result.generation = Some(vault.generation);
        if vault.wrapped.is_none() {
            result.phase = CloudSyncSetupPhase::NewLibrary;
            return Ok(result);
        }
        let valid = self
            .vault_key()
            .is_some_and(|key| matches_stored_vault(self, &key, &vault))
            && self.with_sync_state(|state| state.generation())? == Some(vault.generation);
        if !valid {
            result.phase = CloudSyncSetupPhase::Locked;
            return Ok(result);
        }
        result.can_approve_devices = true;
        result.phase = if enabled {
            CloudSyncSetupPhase::Active
        } else if self
            .secret_store()
            .and_then(|store| store.get(BACKUP_PENDING_ACCOUNT.into()))
            == self
                .vault_key()
                .map(|key| format!("{}:{}", crypto::vault_check(&key), vault.generation))
        {
            CloudSyncSetupPhase::Ready
        } else {
            CloudSyncSetupPhase::Paused
        };
        Ok(result)
    }

    /// Creates access without starting uploads. The returned recovery code is shown once.
    pub fn cloud_sync_prepare(&self) -> Result<Option<String>, PennelError> {
        let _gate = self.sync_gate.lock().unwrap();
        if super::link::has_pending(self) {
            return Err(PennelError::Cloud {
                code: "pairing_pending".into(),
                message: "Finish the current device connection first.".into(),
            });
        }
        let api = self.signed_in_sync_api()?;
        let session = self.cloud_session()?;
        let identity_epoch = session.generation();
        let vault = api.vault()?;
        let _identity = self.inference_capture_gate();
        if session.generation() != identity_epoch {
            return Err(PennelError::Cancelled);
        }
        recover_preparation(self, &vault)?;
        if vault.wrapped.is_some() {
            if self
                .vault_key()
                .is_some_and(|key| matches_stored_vault(self, &key, &vault))
                && self.with_sync_state(|state| state.generation())? == Some(vault.generation)
            {
                return Ok(None);
            }
            return Err(PennelError::VaultLocked);
        }
        let key = VaultKey::generate();
        let code = crypto::new_recovery_code();
        let (wrapped, kdf) = crypto::wrap(&key, &code)?;
        let store = self.secret_store().ok_or(PennelError::NotSignedIn)?;
        let previous_key = Zeroizing::new(store.get(VAULT_KEY_ACCOUNT.into()));
        let previous_pending = Zeroizing::new(store.get(BACKUP_PENDING_ACCOUNT.into()));
        let candidate = Zeroizing::new(serde_json::json!({"origin":self.cloud_session()?.base_url(),"key":key.to_b64(),"generation":vault.generation}).to_string());
        store_verified(store.as_ref(), PREPARING, &candidate)?;
        let prepared = (|| {
            store_verified(store.as_ref(), VAULT_KEY_ACCOUNT, &key.to_b64())?;
            store_verified(
                store.as_ref(),
                BACKUP_PENDING_ACCOUNT,
                &format!("{}:{}", crypto::vault_check(&key), vault.generation),
            )?;
            api.put_vault(vault.generation, &wrapped, &kdf, true)?;
            self.with_sync_state(|state| {
                state.use_vault(&crypto::vault_check(&key))?;
                state.set_generation(vault.generation)
            })?;
            let mut settings = self.settings();
            settings.cloud_sync = false;
            self.update_settings_locked(settings)
        })();
        if let Err(error) = prepared {
            restore_entry(store.as_ref(), VAULT_KEY_ACCOUNT, &previous_key)?;
            restore_entry(store.as_ref(), BACKUP_PENDING_ACCOUNT, &previous_pending)?;
            return Err(error);
        }
        store.set(PREPARING.into(), None);
        Ok(Some(code))
    }

    pub fn cloud_sync_activate(&self) -> Result<(), PennelError> {
        self.cloud_sync_resume()
    }

    pub fn cloud_recovery_code_context(&self, code: String) -> Result<String, PennelError> {
        let _gate = self.sync_gate.lock().unwrap();
        self.recovery_context_locked(&code)
    }

    pub fn cloud_recovery_confirm(&self, code: String, context: String) -> Result<(), PennelError> {
        let _speaker = crate::speaker_jobs::speaker_identity_guard();
        let _gate = self.sync_gate.lock().unwrap();
        let _identity = self.inference_capture_gate();
        if super::link::has_pending(self) {
            return Err(PennelError::Cloud {
                code: "pairing_pending".into(),
                message: "Finish the current device connection first.".into(),
            });
        }
        if self.recovery_context_locked(&code)? != context {
            return Err(PennelError::Cloud {
                code: "recovery_changed".into(),
                message: "This recovery code changed. Save the current code before continuing."
                    .into(),
            });
        }
        let mut settings = self.settings();
        settings.cloud_sync = true;
        self.update_settings_locked(settings)?;
        if let Some(store) = self.secret_store() {
            store.set(BACKUP_PENDING_ACCOUNT.into(), None);
        }
        Ok(())
    }

    /// Resumes only a key that matches the current library and generation.
    pub fn cloud_sync_resume(&self) -> Result<(), PennelError> {
        let _gate = self.sync_gate.lock().unwrap();
        if super::link::has_pending(self) {
            return Err(PennelError::Cloud {
                code: "pairing_pending".into(),
                message: "Finish the current device connection first.".into(),
            });
        }
        let session = self.cloud_session()?;
        let identity_epoch = session.generation();
        let vault = self.signed_in_sync_api()?.vault()?;
        let _identity = self.inference_capture_gate();
        if session.generation() != identity_epoch {
            return Err(PennelError::Cancelled);
        }
        let key = self.vault_key().ok_or(PennelError::VaultLocked)?;
        if !matches_stored_vault(self, &key, &vault)
            || self.with_sync_state(|state| state.generation())? != Some(vault.generation)
        {
            return Err(PennelError::VaultLocked);
        }
        let mut settings = self.settings();
        settings.cloud_sync = true;
        self.update_settings_locked(settings)?;
        if let Some(store) = self.secret_store() {
            store.set(BACKUP_PENDING_ACCOUNT.into(), None);
        }
        Ok(())
    }
}

impl PennelEngine {
    fn recovery_context_locked(&self, code: &str) -> Result<String, PennelError> {
        let (account, _) = self.cloud_session()?.identity()?;
        let vault = self.signed_in_sync_api()?.vault()?;
        let (Some(wrapped), Some(kdf)) = (vault.wrapped, vault.kdf) else {
            return Err(PennelError::VaultLocked);
        };
        let key = crypto::unwrap(&wrapped, &kdf, code)?;
        if self
            .vault_key()
            .is_none_or(|local| crypto::vault_check(&local) != crypto::vault_check(&key))
            || self.with_sync_state(|state| state.generation())? != Some(vault.generation)
        {
            return Err(PennelError::VaultLocked);
        }
        let canonical = serde_json::to_vec(&(
            "pennel-recovery-confirm-v1",
            account,
            vault.generation,
            &wrapped,
            &kdf,
        ))
        .map_err(PennelError::storage)?;
        Ok(Sha256::digest(canonical)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}
