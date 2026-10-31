//! Opt-in loopback-only interoperability driver. Requires an isolated test account's
//! refresh token in PENNEL_LINK_FIXTURE_REFRESH; it never prints credentials or codes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pennel_core::{CloudLinkPhase, CloudLinkRole, PennelEngine, SecretStore, SyncStatus};

#[derive(Default)]
struct Secrets(Mutex<HashMap<String, String>>);
impl SecretStore for Secrets {
    fn get(&self, account: String) -> Option<String> {
        self.0.lock().unwrap().get(&account).cloned()
    }
    fn set(&self, account: String, value: Option<String>) {
        let mut values = self.0.lock().unwrap();
        if let Some(value) = value {
            values.insert(account, value);
        } else {
            values.remove(&account);
        }
    }
}

struct Host {
    _directory: tempfile::TempDir,
    engine: Arc<PennelEngine>,
    secrets: Arc<Secrets>,
}
fn host() -> Host {
    let directory = tempfile::tempdir().expect("isolated fixture directory");
    let engine = PennelEngine::new(directory.path().join("data").to_string_lossy().into_owned())
        .expect("fixture engine");
    let mut settings = engine.settings();
    settings.meetings_dir = directory
        .path()
        .join("Meetings")
        .to_string_lossy()
        .into_owned();
    engine.update_settings(settings).expect("isolated library");
    let secrets = Arc::new(Secrets::default());
    engine
        .set_secret_store(secrets.clone())
        .expect("fixture secret store");
    Host {
        _directory: directory,
        engine,
        secrets,
    }
}

fn connect(new: &Host, trusted: &Host, trusted_creates: bool, use_qr: bool) {
    let view = if trusted_creates {
        trusted
            .engine
            .cloud_link_start(
                CloudLinkRole::TrustedDevice,
                "Fixture computer".into(),
                "macos".into(),
            )
            .expect("trusted start")
    } else {
        new.engine
            .cloud_link_start(
                CloudLinkRole::NewDevice,
                "Fixture phone".into(),
                "ios".into(),
            )
            .expect("new start")
    };
    let code = if use_qr {
        view.qr_payload.expect("QR payload")
    } else {
        view.connection_code.expect("manual code")
    };
    if trusted_creates {
        new.engine
            .cloud_link_join(
                code,
                CloudLinkRole::NewDevice,
                "Fixture phone".into(),
                "ios".into(),
            )
            .expect("new joins");
    } else {
        trusted
            .engine
            .cloud_link_join(
                code,
                CloudLinkRole::TrustedDevice,
                "Fixture computer".into(),
                "macos".into(),
            )
            .expect("trusted joins");
    }
    let mut approved = false;
    for _ in 0..40 {
        let new_view = new.engine.cloud_link_poll().expect("new poll");
        let trusted_view = trusted.engine.cloud_link_poll().expect("trusted poll");
        assert!(
            !matches!(
                new_view.phase,
                CloudLinkPhase::Failed | CloudLinkPhase::Expired
            ),
            "new connection failed: {:?}",
            new_view.error_code
        );
        assert!(
            !matches!(
                trusted_view.phase,
                CloudLinkPhase::Failed | CloudLinkPhase::Expired
            ),
            "trusted connection failed: {:?}",
            trusted_view.error_code
        );
        if trusted_view.phase == CloudLinkPhase::AwaitingApproval && !approved {
            assert_eq!(new_view.verification_code, trusted_view.verification_code);
            trusted
                .engine
                .cloud_link_approve()
                .expect("explicit fixture approval");
            approved = true;
        }
        if new_view.phase == CloudLinkPhase::Complete
            && trusted_view.phase == CloudLinkPhase::Complete
        {
            assert!(new.engine.cloud_signed_in());
            assert!(new.engine.settings().cloud_sync);
            assert_eq!(
                new.secrets.get("pennel-cloud-vault-key".into()),
                trusted.secrets.get("pennel-cloud-vault-key".into())
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("connection did not complete");
}

fn main() {
    let base = std::env::var("PENNEL_CLOUD_URL").expect("loopback PENNEL_CLOUD_URL required");
    assert!(
        base.starts_with("http://127.0.0.1:") || base.starts_with("http://localhost:"),
        "fixture driver only accepts loopback HTTP"
    );
    let refresh = std::env::var("PENNEL_LINK_FIXTURE_REFRESH")
        .expect("synthetic fixture refresh token required");
    let trusted = host();
    trusted
        .secrets
        .set("pennel-cloud-refresh".into(), Some(refresh));
    let code = trusted
        .engine
        .cloud_sync_prepare()
        .expect("prepare fixture library");
    assert!(code.is_some());
    assert!(!trusted.engine.settings().cloud_sync);
    trusted
        .engine
        .cloud_sync_activate()
        .expect("fixture backup acknowledged");
    let folder = std::path::PathBuf::from(trusted.engine.settings().meetings_dir)
        .join("2026-10-30 1200 Device connection fixture");
    std::fs::create_dir_all(&folder).expect("fixture meeting directory");
    std::fs::write(folder.join("meeting.md"),format!("---\ntitle: Device connection fixture\nstart: 2026-10-30T12:00:00Z\nduration_min: 1\npennel_id: {}\n---\n\nSynthetic connection fixture.\n",uuid::Uuid::now_v7())).expect("fixture meeting");
    std::fs::write(folder.join("notes.md"), "Synthetic cross-device note.\n")
        .expect("fixture note");
    assert_eq!(
        trusted.engine.cloud_sync_run(vec![]).status,
        SyncStatus::Synced
    );
    let first = host();
    connect(&first, &trusted, false, false);
    let report = first.engine.cloud_sync_run(vec![]);
    assert_eq!(report.status, SyncStatus::Synced);
    assert!(report.downloaded >= 2);
    let second = host();
    connect(&second, &trusted, true, true);
    assert_eq!(
        second.engine.cloud_sync_run(vec![]).status,
        SyncStatus::Synced
    );
    let device_count = first.engine.cloud_devices().expect("fixture devices").len();
    first.engine.cloud_sync_disable().expect("pause");
    first.secrets.set("pennel-cloud-vault-key".into(), None);
    connect(&first, &trusted, true, false);
    assert_eq!(
        first
            .engine
            .cloud_devices()
            .expect("fixture devices after reconnect")
            .len(),
        device_count
    );
    println!(
        "{{\"connections\":3,\"new_creator_manual\":true,\"trusted_creator_qr\":true,\"existing_device_reused\":true,\"synthetic_meeting_synced\":true}}"
    );
}
