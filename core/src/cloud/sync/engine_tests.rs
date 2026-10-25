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
    engine.set_secret_store(secrets.clone());
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
    a.engine.cloud_logout();
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
