use std::sync::Arc;

use super::api::fake::FakeApi;
use super::*;
use crate::cloud::account::Session;
use crate::cloud::tests::MemorySecrets;
use crate::cloud::{REFRESH_ACCOUNT, SecretStore, VAULT_KEY_ACCOUNT};
use crate::engine::PennelEngine;

struct Host {
    _dir: tempfile::TempDir,
    engine: Arc<PennelEngine>,
    secrets: Arc<MemorySecrets>,
    root: PathBuf,
}

fn host(api: &Arc<FakeApi>, device: &str) -> Host {
    let dir = tempfile::tempdir().unwrap();
    let engine = PennelEngine::new(dir.path().join("data").to_string_lossy().into_owned()).unwrap();
    let root = dir.path().join("Meetings");
    let mut settings = engine.settings();
    settings.meetings_dir = root.to_string_lossy().into_owned();
    engine.update_settings(settings).unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    secrets.set(REFRESH_ACCOUNT.into(), Some("refresh".into()));
    engine.set_secret_store(secrets.clone()).unwrap();
    // Unreachable on purpose: only logout talks to the account API in these tests.
    engine.use_cloud_session(Session::new("http://127.0.0.1:1".into(), secrets.clone()));
    engine.use_sync_api(api.clone());
    engine.remember_device_name(device);
    Host {
        _dir: dir,
        engine,
        secrets,
        root,
    }
}

fn store(host: &Host) -> MeetingStore {
    MeetingStore::new(&host.root)
}

fn start() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 25, 9, 30, 0).unwrap()
}

#[test]
fn enable_unlock_and_sync_between_two_engines() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Laptop");
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Off);
    let code = a.engine.cloud_sync_enable().unwrap();
    assert!(a.engine.settings().cloud_sync);
    assert!(a.secrets.get(VAULT_KEY_ACCOUNT.into()).is_some());

    assert_eq!(b.engine.cloud_sync_enable(), Err(PennelError::VaultLocked));
    assert!(!b.engine.settings().cloud_sync);
    let mut settings = b.engine.settings();
    settings.cloud_sync = true;
    b.engine.update_settings(settings).unwrap();
    assert_eq!(
        b.engine.cloud_sync_run(vec![]).status,
        SyncStatus::NeedsRecoveryCode
    );
    assert!(
        b.engine
            .cloud_sync_unlock("0000-0000-0000-0000-0000-0000".into())
            .is_err()
    );
    b.engine.cloud_sync_unlock(code.to_lowercase()).unwrap();

    let id = store(&a).create("Overleg", start()).unwrap();
    std::fs::write(a.root.join(&id).join("notes.md"), "a\n").unwrap();
    let report = a.engine.cloud_sync_run(vec![]);
    assert_eq!(report.status, SyncStatus::Synced);
    assert_eq!(report.uploaded, 2);
    assert!(report.last_success_ms.is_some());
    assert_eq!(a.engine.cloud_sync_last(), report);

    assert_eq!(b.engine.cloud_sync_run(vec![]).downloaded, 2);
    assert_eq!(
        b.engine.meeting_sync_states(),
        vec![MeetingSyncState {
            id: id.clone(),
            state: SyncItemState::Synced
        }]
    );
    std::fs::write(b.root.join(&id).join("notes.md"), "b\n").unwrap();
    assert_eq!(
        b.engine.meeting_sync_states()[0].state,
        SyncItemState::Pending
    );
    std::fs::write(
        b.root
            .join(&id)
            .join("notes (conflict Mac 2026-10-25 0930).md"),
        "a\n",
    )
    .unwrap();
    assert_eq!(
        b.engine.meeting_sync_states()[0].state,
        SyncItemState::Conflict
    );

    b.engine.cloud_sync_disable().unwrap();
    assert!(!b.engine.settings().cloud_sync);
    assert!(b.secrets.get(VAULT_KEY_ACCOUNT.into()).is_some());
    assert_eq!(b.engine.cloud_sync_run(vec![]).status, SyncStatus::Off);
}

#[test]
fn setup_reports_a_locked_existing_library_even_while_sync_is_off() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Phone");
    assert_eq!(
        a.engine.cloud_sync_setup().unwrap().phase,
        crate::CloudSyncSetupPhase::NewLibrary
    );
    a.engine.cloud_sync_enable().unwrap();
    assert_eq!(
        b.engine.cloud_sync_setup().unwrap().phase,
        crate::CloudSyncSetupPhase::Locked
    );
    assert!(!b.engine.settings().cloud_sync);
}

#[test]
fn a_prepared_library_waits_for_activation_and_a_paused_device_can_resume() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let code = a.engine.cloud_sync_prepare().unwrap().unwrap();
    assert!(!code.is_empty());
    assert!(!a.engine.settings().cloud_sync);
    assert_eq!(
        a.engine.cloud_sync_setup().unwrap().phase,
        crate::CloudSyncSetupPhase::Ready
    );
    a.engine.cloud_sync_activate().unwrap();
    assert!(a.engine.settings().cloud_sync);
    a.engine.cloud_sync_disable().unwrap();
    assert_eq!(
        a.engine.cloud_sync_setup().unwrap().phase,
        crate::CloudSyncSetupPhase::Paused
    );
    a.engine.cloud_sync_resume().unwrap();
    assert!(a.engine.settings().cloud_sync);
}

#[test]
fn a_paused_device_cannot_resume_after_another_device_reset_the_library() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Phone");
    let code = a.engine.cloud_sync_enable().unwrap();
    b.engine.cloud_sync_unlock(code).unwrap();
    b.engine.cloud_sync_disable().unwrap();
    a.engine.cloud_sync_reset().unwrap();
    assert_eq!(b.engine.cloud_sync_resume(), Err(PennelError::VaultLocked));
    assert!(!b.engine.settings().cloud_sync);
}

#[test]
fn failed_credential_write_does_not_create_a_vault_or_enable_sync() {
    #[derive(Default)]
    struct UnwritableKey(MemorySecrets);
    impl SecretStore for UnwritableKey {
        fn get(&self, account: String) -> Option<String> {
            self.0.get(account)
        }
        fn set(&self, account: String, value: Option<String>) {
            if account != VAULT_KEY_ACCOUNT {
                self.0.set(account, value);
            }
        }
    }
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Phone");
    let secrets = Arc::new(UnwritableKey::default());
    secrets.set(REFRESH_ACCOUNT.into(), Some("fixture-refresh".into()));
    a.engine.set_secret_store(secrets.clone()).unwrap();
    a.engine
        .use_cloud_session(Session::new("http://127.0.0.1:1".into(), secrets));
    assert!(matches!(
        a.engine.cloud_sync_prepare(),
        Err(PennelError::Storage { .. })
    ));
    assert!(!a.engine.settings().cloud_sync);
    assert!(api.vault().unwrap().wrapped.is_none());
}

#[test]
fn preparation_restores_both_previous_entries_after_the_second_write_fails() {
    struct MarkerFailure(MemorySecrets);
    impl SecretStore for MarkerFailure {
        fn get(&self, account: String) -> Option<String> {
            self.0.get(account)
        }
        fn set(&self, account: String, value: Option<String>) {
            if account != crate::cloud::BACKUP_PENDING_ACCOUNT {
                self.0.set(account, value);
            }
        }
    }
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Phone");
    let inner = MemorySecrets::default();
    inner.set(REFRESH_ACCOUNT.into(), Some("fixture-refresh".into()));
    inner.set(
        VAULT_KEY_ACCOUNT.into(),
        Some("previous retained key".into()),
    );
    inner.set(
        crate::cloud::BACKUP_PENDING_ACCOUNT.into(),
        Some("previous backup marker".into()),
    );
    let secrets = Arc::new(MarkerFailure(inner));
    a.engine.set_secret_store(secrets.clone()).unwrap();
    a.engine
        .use_cloud_session(Session::new("http://127.0.0.1:1".into(), secrets.clone()));
    assert!(matches!(
        a.engine.cloud_sync_prepare(),
        Err(PennelError::Storage { .. })
    ));
    assert_eq!(
        secrets.get(VAULT_KEY_ACCOUNT.into()).as_deref(),
        Some("previous retained key")
    );
    assert_eq!(
        secrets
            .get(crate::cloud::BACKUP_PENDING_ACCOUNT.into())
            .as_deref(),
        Some("previous backup marker")
    );
    assert!(api.vault().unwrap().wrapped.is_none());
}

#[test]
fn preparation_restores_previous_entries_after_a_failed_vault_request() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Phone");
    a.secrets.set(
        VAULT_KEY_ACCOUNT.into(),
        Some("previous retained key".into()),
    );
    a.secrets.set(
        crate::cloud::BACKUP_PENDING_ACCOUNT.into(),
        Some("previous backup marker".into()),
    );
    api.fail_vault_puts(true);
    assert!(matches!(
        a.engine.cloud_sync_prepare(),
        Err(PennelError::Network { .. })
    ));
    assert_eq!(
        a.secrets.get(VAULT_KEY_ACCOUNT.into()).as_deref(),
        Some("previous retained key")
    );
    assert_eq!(
        a.secrets
            .get(crate::cloud::BACKUP_PENDING_ACCOUNT.into())
            .as_deref(),
        Some("previous backup marker")
    );
    assert!(!a.engine.settings().cloud_sync);
}

#[test]
fn legacy_vault_without_a_fingerprint_can_resume_after_verified_unlock() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Phone");
    let code = a.engine.cloud_sync_enable().unwrap();
    let vault = api.vault().unwrap();
    let mut kdf = vault.kdf.unwrap();
    kdf.key_check = None;
    api.put_vault(vault.generation, &vault.wrapped.unwrap(), &kdf, false)
        .unwrap();
    b.engine.cloud_sync_unlock(code).unwrap();
    b.engine.cloud_sync_disable().unwrap();
    assert_eq!(
        b.engine.cloud_sync_setup().unwrap().phase,
        crate::CloudSyncSetupPhase::Paused
    );
    b.engine.cloud_sync_resume().unwrap();
    assert!(b.engine.settings().cloud_sync);
}

#[test]
fn recovery_acknowledgment_is_bound_to_account_generation_and_current_wrapping() {
    let mut server = mockito::Server::new();
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Phone");
    a.secrets.set(REFRESH_ACCOUNT.into(), None);
    let session = Session::new(server.url(), a.secrets.clone());
    session.install_link_tokens(session.generation(),&serde_json::json!({"access_token":"fixture-access","refresh_token":"fixture-refresh","expires_in":900})).unwrap();
    a.engine.use_cloud_session(session);
    let account_a = "00000000-0000-4000-8000-000000000001";
    let account_b = "00000000-0000-4000-8000-000000000002";
    let identity = server
        .mock("GET", "/v1/whoami")
        .with_body(
            serde_json::json!({"user_id":account_a,"device_id":"fixture-device"}).to_string(),
        )
        .expect(1)
        .create();
    let code = a.engine.cloud_sync_prepare().unwrap().unwrap();
    let context = a.engine.cloud_recovery_code_context(code.clone()).unwrap();
    identity.assert();
    identity.remove();
    let changed = server
        .mock("GET", "/v1/whoami")
        .with_body(
            serde_json::json!({"user_id":account_b,"device_id":"fixture-device"}).to_string(),
        )
        .expect(1)
        .create();
    assert!(
        a.engine
            .cloud_recovery_confirm(code.clone(), context.clone())
            .is_err()
    );
    assert!(!a.engine.settings().cloud_sync);
    changed.assert();
    changed.remove();
    let identity = server
        .mock("GET", "/v1/whoami")
        .with_body(
            serde_json::json!({"user_id":account_a,"device_id":"fixture-device"}).to_string(),
        )
        .expect(4)
        .create();
    let replacement = a.engine.cloud_sync_new_recovery_code().unwrap();
    assert!(
        a.engine
            .cloud_recovery_confirm(code, context.clone())
            .is_err()
    );
    let current = a
        .engine
        .cloud_recovery_code_context(replacement.clone())
        .unwrap();
    assert_ne!(context, current);
    assert!(
        a.engine
            .cloud_recovery_confirm(replacement.clone(), context)
            .is_err()
    );
    assert!(!a.engine.settings().cloud_sync);
    a.engine
        .cloud_recovery_confirm(replacement, current)
        .unwrap();
    assert!(a.engine.settings().cloud_sync);
    identity.assert();
}

#[test]
fn reset_on_one_engine_makes_the_other_ask_for_the_new_code() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Laptop");
    let code = a.engine.cloud_sync_enable().unwrap();
    b.engine.cloud_sync_unlock(code.clone()).unwrap();
    let id = store(&b).create("Lokaal", start()).unwrap();
    assert_eq!(b.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);

    let new_code = a.engine.cloud_sync_reset().unwrap();
    assert_ne!(new_code, code);
    assert_eq!(api.live_objects(), 0);
    assert_eq!(
        b.engine.cloud_sync_run(vec![]).status,
        SyncStatus::VaultReset
    );
    assert_eq!(b.secrets.get(VAULT_KEY_ACCOUNT.into()), None);
    assert_eq!(
        b.engine.cloud_sync_run(vec![]).status,
        SyncStatus::NeedsRecoveryCode
    );
    assert!(b.engine.cloud_sync_unlock(code).is_err());
    b.engine.cloud_sync_unlock(new_code.clone()).unwrap();
    let report = b.engine.cloud_sync_run(vec![]);
    assert_eq!(report.status, SyncStatus::Synced);
    assert_eq!(report.uploaded, 1);
    assert!(b.root.join(&id).join("meeting.md").exists());

    let newest = b.engine.cloud_sync_new_recovery_code().unwrap();
    assert!(a.engine.cloud_sync_unlock(new_code).is_err());
    a.engine.cloud_sync_unlock(newest).unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).downloaded, 1);
}

#[test]
fn logout_clears_sync_state_and_keeps_meetings() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    a.engine.cloud_sync_enable().unwrap();
    let id = store(&a).create("Blijft", start()).unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).uploaded, 1);
    a.engine.cloud_logout().unwrap();
    assert!(a.root.join(&id).join("meeting.md").exists());
    assert_eq!(
        a.engine.cloud_sync_run(vec![]).status,
        SyncStatus::SignedOut
    );
    let state = SyncState::open(&a.engine.data_dir()).unwrap();
    assert!(state.entries().unwrap().is_empty());
    assert_eq!(state.generation().unwrap(), None);
}

#[test]
fn signed_out_engine_reports_signed_out() {
    let dir = tempfile::tempdir().unwrap();
    let engine = PennelEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
    let mut settings = engine.settings();
    settings.cloud_sync = true;
    settings.meetings_dir = dir.path().join("M").to_string_lossy().into_owned();
    engine.update_settings(settings).unwrap();
    assert_eq!(engine.cloud_sync_run(vec![]).status, SyncStatus::SignedOut);
    assert_eq!(engine.cloud_sync_enable(), Err(PennelError::NotSignedIn));
}

fn keys() -> crate::engine::ApiKeys {
    crate::engine::ApiKeys {
        by_provider: std::collections::HashMap::new(),
    }
}

#[test]
fn engine_deletes_bypass_the_brake_and_confirm_releases_held_deletes() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    a.engine.cloud_sync_enable().unwrap();
    let ids: Vec<String> = (0..2)
        .map(|n| store(&a).create(&format!("Weg {n}"), start()).unwrap())
        .collect();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    for id in &ids {
        a.engine.delete_meeting(id.clone()).unwrap();
    }
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(api.live_objects(), 0);

    let ids: Vec<String> = (0..2)
        .map(|n| store(&a).create(&format!("Extern {n}"), start()).unwrap())
        .collect();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    for id in &ids {
        std::fs::remove_dir_all(a.root.join(id)).unwrap();
    }
    assert!(matches!(
        a.engine.cloud_sync_run(vec![]).status,
        SyncStatus::Error { .. }
    ));
    assert_eq!(api.live_objects(), 2);
    a.engine.cloud_sync_confirm_deletes().unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(api.live_objects(), 0);
}

#[test]
fn own_recording_session_is_never_synced() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    a.engine.cloud_sync_enable().unwrap();
    let session = a
        .engine
        .clone()
        .start_meeting("Loopt nog".into(), keys())
        .unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(api.live_objects(), 0);
    drop(session);
}

#[test]
fn status_reads_do_not_wait_for_a_running_pass() {
    use std::sync::mpsc;
    use std::time::Duration;
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Laptop");
    let code = a.engine.cloud_sync_enable().unwrap();
    b.engine.cloud_sync_unlock(code).unwrap();
    store(&a).create("Onderweg", start()).unwrap();
    a.engine.cloud_sync_run(vec![]);
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let release_rx = std::sync::Mutex::new(release_rx);
    api.on_get(move |_| {
        let _ = entered.send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10));
    });
    let engine = b.engine.clone();
    let pass = std::thread::spawn(move || engine.cloud_sync_run(vec![]));
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let engine = b.engine.clone();
    let (done, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let states = engine.meeting_sync_states();
        let last = engine.cloud_sync_last();
        let _ = done.send((states, last));
    });
    let answered = done_rx.recv_timeout(Duration::from_secs(2));
    release.send(()).unwrap();
    assert!(answered.is_ok(), "status reads blocked on the running pass");
    assert_eq!(pass.join().unwrap().status, SyncStatus::Synced);
}

#[test]
fn delete_meeting_does_not_wait_for_a_running_pass_and_still_bypasses_the_brake() {
    use std::sync::mpsc;
    use std::time::Duration;
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    a.engine.cloud_sync_enable().unwrap();
    let ids: Vec<String> = (0..2)
        .map(|n| store(&a).create(&format!("Bezig {n}"), start()).unwrap())
        .collect();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    std::fs::write(a.root.join(&ids[0]).join("notes.md"), "upload\n").unwrap();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let release_rx = std::sync::Mutex::new(release_rx);
    let mut fired = false;
    api.on_put(move |_| {
        if !fired {
            fired = true;
            let _ = entered.send(());
            let _ = release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10));
        }
    });
    let engine = a.engine.clone();
    let pass = std::thread::spawn(move || engine.cloud_sync_run(vec![]));
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let engine = a.engine.clone();
    let doomed = ids.clone();
    let (done, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for id in doomed {
            engine.delete_meeting(id).unwrap();
        }
        let _ = done.send(());
    });
    let answered = done_rx.recv_timeout(Duration::from_secs(2));
    release.send(()).unwrap();
    assert!(
        answered.is_ok(),
        "delete_meeting waited for the running pass"
    );
    pass.join().unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(api.live_objects(), 0);
}

#[test]
fn delete_meeting_without_sync_creates_no_sync_state() {
    let dir = tempfile::tempdir().unwrap();
    let engine = PennelEngine::new(dir.path().join("data").to_string_lossy().into_owned()).unwrap();
    let mut settings = engine.settings();
    settings.meetings_dir = dir.path().join("M").to_string_lossy().into_owned();
    engine.update_settings(settings).unwrap();
    let id = MeetingStore::new(&dir.path().join("M"))
        .create("Lokaal", start())
        .unwrap();
    engine.delete_meeting(id).unwrap();
    assert!(!dir.path().join("data").join("sync.sqlite").exists());
}

/// The server ends this device's session the way a revoked device sees it: the refresh is refused.
fn revoke_session(host: &Host) {
    let mut server = mockito::Server::new();
    let _refresh = server
        .mock("POST", "/v1/auth/refresh")
        .with_status(401)
        .with_body(r#"{"error":"session_revoked","message":"Afgemeld."}"#)
        .create();
    host.engine
        .use_cloud_session(Session::new(server.url(), host.secrets.clone()));
    assert_eq!(host.engine.cloud_account(), Err(PennelError::NotSignedIn));
    host.engine.use_cloud_session(Session::new(
        "http://127.0.0.1:1".into(),
        host.secrets.clone(),
    ));
}

fn sign_in(host: &Host, api: &Arc<FakeApi>) {
    host.secrets
        .set(REFRESH_ACCOUNT.into(), Some("refresh-again".into()));
    host.engine.use_sync_api(api.clone());
}

#[test]
fn another_account_after_revocation_gets_a_full_compare() {
    let first = Arc::new(FakeApi::new());
    let a = host(&first, "Mac");
    a.engine.cloud_sync_enable().unwrap();
    for n in 0..2 {
        store(&a).create(&format!("Eigen {n}"), start()).unwrap();
    }
    assert_eq!(a.engine.cloud_sync_run(vec![]).uploaded, 2);
    revoke_session(&a);
    assert_eq!(
        a.engine.cloud_sync_run(vec![]).status,
        SyncStatus::SignedOut
    );

    let second = Arc::new(FakeApi::new());
    let other = host(&second, "Pc");
    let code = other.engine.cloud_sync_enable().unwrap();
    store(&other).create("Van de ander", start()).unwrap();
    assert_eq!(other.engine.cloud_sync_run(vec![]).uploaded, 1);

    sign_in(&a, &second);
    assert_eq!(
        a.engine.cloud_sync_new_recovery_code(),
        Err(PennelError::VaultLocked)
    );
    assert_eq!(
        a.engine.cloud_sync_run(vec![]).status,
        SyncStatus::NeedsRecoveryCode
    );
    assert_eq!(a.secrets.get(VAULT_KEY_ACCOUNT.into()), None);
    assert_eq!(second.live_objects(), 1);
    a.engine.cloud_sync_unlock(code).unwrap();
    let report = a.engine.cloud_sync_run(vec![]);
    assert_eq!(report.status, SyncStatus::Synced);
    assert_eq!(report.uploaded, 2);
    assert_eq!(report.downloaded, 1);
    assert_eq!(second.live_objects(), 3);
}

#[test]
fn revoked_device_keeps_its_key_and_resumes_without_the_code() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    a.engine.cloud_sync_enable().unwrap();
    let id = store(&a).create("Blijft", start()).unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).uploaded, 1);
    revoke_session(&a);
    assert!(a.secrets.get(VAULT_KEY_ACCOUNT.into()).is_some());

    sign_in(&a, &api);
    std::fs::write(a.root.join(&id).join("notes.md"), "nieuw\n").unwrap();
    let report = a.engine.cloud_sync_run(vec![]);
    assert_eq!(report.status, SyncStatus::Synced);
    assert_eq!(report.uploaded, 1);
    assert_eq!(api.live_objects(), 2);
}

#[test]
fn confirming_deletes_and_naming_the_device_do_not_wait_for_a_running_pass() {
    use std::sync::mpsc;
    use std::time::Duration;
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Laptop");
    let code = a.engine.cloud_sync_enable().unwrap();
    b.engine.cloud_sync_unlock(code).unwrap();
    let ids: Vec<String> = (0..2)
        .map(|n| store(&a).create(&format!("Weg {n}"), start()).unwrap())
        .collect();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    for id in &ids {
        std::fs::remove_dir_all(a.root.join(id)).unwrap();
    }
    assert!(matches!(
        a.engine.cloud_sync_run(vec![]).status,
        SyncStatus::Error { .. }
    ));
    b.engine.cloud_sync_run(vec![]);
    store(&b).create("Nieuw", start()).unwrap();
    assert_eq!(b.engine.cloud_sync_run(vec![]).uploaded, 1);

    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let release_rx = std::sync::Mutex::new(release_rx);
    api.on_get(move |_| {
        let _ = entered.send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10));
    });
    let engine = a.engine.clone();
    let pass = std::thread::spawn(move || engine.cloud_sync_run(vec![]));
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let engine = a.engine.clone();
    let (done, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let confirmed = engine.cloud_sync_confirm_deletes();
        engine.remember_device_name("Werkplek");
        let _ = done.send(confirmed);
    });
    let answered = done_rx.recv_timeout(Duration::from_secs(2));
    release.send(()).unwrap();
    assert_eq!(answered.expect("waited for the running pass"), Ok(()));
    pass.join().unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(api.live_objects(), 1);
    let state = SyncState::open(&a.engine.data_dir()).unwrap();
    assert_eq!(state.device().unwrap().as_deref(), Some("Werkplek"));
}

#[test]
fn last_status_before_any_pass_is_not_synced() {
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    assert_eq!(a.engine.cloud_sync_last().status, SyncStatus::Off);
    let mut settings = a.engine.settings();
    settings.cloud_sync = true;
    a.engine.update_settings(settings).unwrap();
    assert_eq!(a.engine.cloud_sync_last().status, SyncStatus::NeedsSetup);
    a.engine.cloud_sync_enable().unwrap();
    assert_eq!(a.engine.cloud_sync_last().status, SyncStatus::Pending);
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(a.engine.cloud_sync_last().status, SyncStatus::Synced);
    revoke_session(&a);
    assert_eq!(a.engine.cloud_sync_last().status, SyncStatus::SignedOut);
}

#[test]
fn disabling_reports_off_at_once_even_during_a_pass() {
    use std::sync::mpsc;
    use std::time::Duration;
    let api = Arc::new(FakeApi::new());
    let a = host(&api, "Mac");
    let b = host(&api, "Laptop");
    let code = a.engine.cloud_sync_enable().unwrap();
    b.engine.cloud_sync_unlock(code).unwrap();
    store(&a).create("Onderweg", start()).unwrap();
    assert_eq!(a.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    assert_eq!(b.engine.cloud_sync_run(vec![]).status, SyncStatus::Synced);
    b.engine.cloud_sync_disable().unwrap();
    assert_eq!(b.engine.cloud_sync_last().status, SyncStatus::Off);
    let mut settings = b.engine.settings();
    settings.cloud_sync = true;
    b.engine.update_settings(settings).unwrap();
    store(&a).create("Tweede", start()).unwrap();
    a.engine.cloud_sync_run(vec![]);

    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let release_rx = std::sync::Mutex::new(release_rx);
    api.on_get(move |_| {
        let _ = entered.send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10));
    });
    let engine = b.engine.clone();
    let pass = std::thread::spawn(move || engine.cloud_sync_run(vec![]));
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let engine = b.engine.clone();
    let (done, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let disabled = engine.cloud_sync_disable();
        let _ = done.send((disabled, engine.cloud_sync_last().status));
    });
    let answered = done_rx.recv_timeout(Duration::from_secs(2));
    release.send(()).unwrap();
    assert_eq!(
        answered.expect("disable waited for the running pass"),
        (Ok(()), SyncStatus::Off)
    );
    pass.join().unwrap();
    assert_eq!(b.engine.cloud_sync_last().status, SyncStatus::Off);
}
