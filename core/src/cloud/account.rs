use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::http::{agent, error_from, send_json};
use super::{REFRESH_ACCOUNT, SecretStore};
use crate::error::PennelError;

const REFRESH_MARGIN: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CloudLoginStart {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval_secs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CloudLoginPoll {
    Pending,
    SlowDown,
    Expired,
    Done { email: String },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CloudAccount {
    pub email: String,
    pub audio_seconds: u64,
    pub tokens: u64,
    pub storage_bytes: u64,
    pub limit_audio_seconds: u64,
    pub limit_tokens: u64,
    pub limit_storage_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CloudDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub last_seen_iso: String,
    pub current: bool,
}

struct AccessToken {
    value: String,
    expires_at: Instant,
}

/// Holding `access` across the refresh request makes refreshes single-flight: the server
/// revokes a device when a refresh token is presented twice.
pub struct Session {
    base_url: String,
    secrets: Arc<dyn SecretStore>,
    http: ureq::Agent,
    access: Mutex<Option<AccessToken>>,
}

fn invalid(what: &str) -> PennelError {
    PennelError::InvalidResponse {
        message: format!("unexpected {what} response"),
    }
}

fn text(value: &Value, key: &str, what: &str) -> Result<String, PennelError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| invalid(what))
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn is_revoked(code: &str) -> bool {
    matches!(code, "unauthorized" | "session_revoked")
}

impl Session {
    pub fn new(base_url: String, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            base_url,
            secrets,
            http: agent(REQUEST_TIMEOUT_SECS),
            access: Mutex::new(None),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    pub fn signed_in(&self) -> bool {
        self.secrets.get(REFRESH_ACCOUNT.into()).is_some()
    }

    /// Signing out on purpose also forgets the vault key.
    fn clear_secrets(&self) {
        let mut access = self.access.lock().unwrap();
        self.end_session_locked(&mut access);
        self.secrets.set(super::VAULT_KEY_ACCOUNT.into(), None);
    }

    // Callers hold `access`, so an in-flight refresh cannot write a rotated token afterwards.
    // The vault key stays: a revoked device keeps what it had, and sync checks the key against
    // the account's vault before using it again.
    fn end_session_locked(&self, access: &mut Option<AccessToken>) {
        self.secrets.set(REFRESH_ACCOUNT.into(), None);
        *access = None;
    }

    pub fn access_token(&self) -> Result<String, PennelError> {
        self.token(false)
    }

    /// For a request that got 401 although its access token looked valid.
    pub fn refreshed_token(&self) -> Result<String, PennelError> {
        self.token(true)
    }

    fn token(&self, force: bool) -> Result<String, PennelError> {
        let mut access = self.access.lock().unwrap();
        if !force
            && let Some(current) = access.as_ref()
            && current.expires_at > Instant::now() + REFRESH_MARGIN
        {
            return Ok(current.value.clone());
        }
        let refresh_token = self
            .secrets
            .get(REFRESH_ACCOUNT.into())
            .ok_or(PennelError::NotSignedIn)?;
        let (status, body) = send_json(
            &self.http,
            "POST",
            &self.url("/v1/auth/refresh"),
            None,
            Some(&json!({ "refresh_token": refresh_token })),
        )?;
        if status == 401 {
            let code = body.get("error").and_then(Value::as_str).unwrap_or("");
            if code.is_empty() || is_revoked(code) {
                self.end_session_locked(&mut access);
                return Err(PennelError::NotSignedIn);
            }
        }
        if status != 200 {
            return Err(error_from(status, &body));
        }
        let value = text(&body, "access_token", "refresh")?;
        let rotated = text(&body, "refresh_token", "refresh")?;
        let lifetime = body
            .get("expires_in")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("refresh"))?;
        // The old token is spent on the server; losing the new one would end the session.
        self.secrets.set(REFRESH_ACCOUNT.into(), Some(rotated));
        *access = Some(AccessToken {
            value: value.clone(),
            expires_at: Instant::now() + Duration::from_secs(lifetime),
        });
        Ok(value)
    }

    /// Authenticated request; a 401 triggers one forced refresh in case the access token was revoked early.
    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), PennelError> {
        let url = self.url(path);
        let mut response = send_json(&self.http, method, &url, Some(&self.access_token()?), body)?;
        if response.0 == 401 {
            let token = self.refreshed_token()?;
            response = send_json(&self.http, method, &url, Some(&token), body)?;
        }
        if !(200..300).contains(&response.0) {
            return Err(error_from(response.0, &response.1));
        }
        Ok(response)
    }

    pub fn login_start(
        &self,
        device_name: &str,
        platform: &str,
    ) -> Result<CloudLoginStart, PennelError> {
        let (status, body) = send_json(
            &self.http,
            "POST",
            &self.url("/v1/auth/device"),
            None,
            Some(&json!({ "device_name": device_name, "platform": platform })),
        )?;
        if status != 200 {
            return Err(error_from(status, &body));
        }
        Ok(CloudLoginStart {
            user_code: text(&body, "user_code", "login")?,
            verification_uri: text(&body, "verification_uri", "login")?,
            device_code: text(&body, "device_code", "login")?,
            interval_secs: body
                .get("interval")
                .and_then(Value::as_u64)
                .map_or(5, |secs| secs as u32),
        })
    }

    pub fn login_poll(&self, device_code: &str) -> Result<CloudLoginPoll, PennelError> {
        let (status, body) = send_json(
            &self.http,
            "POST",
            &self.url("/v1/auth/token"),
            None,
            Some(&json!({ "device_code": device_code })),
        )?;
        if status == 400 {
            match body.get("error").and_then(Value::as_str) {
                Some("authorization_pending") => return Ok(CloudLoginPoll::Pending),
                Some("slow_down") => return Ok(CloudLoginPoll::SlowDown),
                Some("expired_token") => return Ok(CloudLoginPoll::Expired),
                _ => {}
            }
        }
        if status != 200 {
            return Err(error_from(status, &body));
        }
        let access = text(&body, "access_token", "login")?;
        let refresh = text(&body, "refresh_token", "login")?;
        let lifetime = body
            .get("expires_in")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("login"))?;
        self.secrets.set(REFRESH_ACCOUNT.into(), Some(refresh));
        *self.access.lock().unwrap() = Some(AccessToken {
            value: access,
            expires_at: Instant::now() + Duration::from_secs(lifetime),
        });
        // The tokens are already stored; the UI fetches the account again if the e-mail is missing.
        let email = self
            .account()
            .map(|account| account.email)
            .unwrap_or_default();
        Ok(CloudLoginPoll::Done { email })
    }

    pub fn account(&self) -> Result<CloudAccount, PennelError> {
        let (_, body) = self.call("GET", "/v1/account", None)?;
        let usage = body.get("usage").unwrap_or(&Value::Null);
        let limits = body.get("limits").unwrap_or(&Value::Null);
        Ok(CloudAccount {
            email: text(&body, "email", "account")?,
            audio_seconds: number(usage, "audio_seconds"),
            tokens: number(usage, "tokens"),
            storage_bytes: number(usage, "storage_bytes"),
            limit_audio_seconds: number(limits, "audio_seconds_per_month"),
            limit_tokens: number(limits, "tokens_per_month"),
            limit_storage_bytes: number(limits, "storage_bytes"),
        })
    }

    pub fn devices(&self) -> Result<Vec<CloudDevice>, PennelError> {
        let (_, body) = self.call("GET", "/v1/devices", None)?;
        body.as_array()
            .ok_or_else(|| invalid("devices"))?
            .iter()
            .map(|device| {
                Ok(CloudDevice {
                    id: text(device, "id", "devices")?,
                    name: text(device, "name", "devices")?,
                    platform: text(device, "platform", "devices")?,
                    last_seen_iso: text(device, "last_seen_at", "devices")?,
                    current: device
                        .get("current")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                })
            })
            .collect()
    }

    pub fn rename_device(&self, id: &str, name: &str) -> Result<(), PennelError> {
        self.call(
            "PATCH",
            &format!("/v1/devices/{id}"),
            Some(&json!({ "name": name })),
        )
        .map(drop)
    }

    pub fn revoke_device(&self, id: &str) -> Result<(), PennelError> {
        self.call("DELETE", &format!("/v1/devices/{id}"), None)
            .map(drop)
    }

    pub fn logout(&self) {
        if self.signed_in() {
            let own_id = self
                .devices()
                .ok()
                .and_then(|devices| devices.into_iter().find(|device| device.current));
            if let Some(own) = own_id {
                let _ = self.revoke_device(&own.id);
            }
        }
        self.clear_secrets();
    }

    pub fn delete_account(&self) -> Result<(), PennelError> {
        self.call("DELETE", "/v1/account", None)?;
        self.clear_secrets();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::tests::MemorySecrets;
    use crate::engine::PennelEngine;
    use mockito::Matcher;

    fn fixture(server: &mockito::ServerGuard) -> (Arc<MemorySecrets>, Session) {
        let secrets = Arc::new(MemorySecrets::default());
        let session = Session::new(server.url(), secrets.clone());
        (secrets, session)
    }

    fn stored(secrets: &MemorySecrets) -> Option<String> {
        secrets.get(REFRESH_ACCOUNT.into())
    }

    const ACCOUNT: &str = r#"{"email":"a@b.nl","created_at":"2026-01-01T00:00:00Z","usage":{"audio_seconds":5,"tokens":6,"storage_bytes":7},"limits":{"storage_bytes":70,"object_bytes":1,"devices":3,"audio_seconds_per_month":50,"tokens_per_month":60}}"#;

    #[test]
    fn login_poll_pending_then_done_stores_refresh_token() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        let start = server
            .mock("POST", "/v1/auth/device")
            .match_body(Matcher::Json(
                json!({"device_name": "Mac", "platform": "macos"}),
            ))
            .with_body(r#"{"device_code":"dc","user_code":"ABCD-1234","verification_uri":"https://x/device?code=ABCD-1234","interval":5,"expires_in":600}"#)
            .create();
        let started = session.login_start("Mac", "macos").unwrap();
        start.assert();
        assert_eq!(
            started,
            CloudLoginStart {
                user_code: "ABCD-1234".into(),
                verification_uri: "https://x/device?code=ABCD-1234".into(),
                device_code: "dc".into(),
                interval_secs: 5
            }
        );

        let pending = server
            .mock("POST", "/v1/auth/token")
            .with_status(400)
            .with_body(r#"{"error":"authorization_pending","message":"Wachten."}"#)
            .create();
        assert_eq!(session.login_poll("dc").unwrap(), CloudLoginPoll::Pending);
        assert_eq!(stored(&secrets), None);
        pending.remove();

        let _done = server
            .mock("POST", "/v1/auth/token")
            .match_body(Matcher::Json(json!({"device_code": "dc"})))
            .with_body(r#"{"access_token":"acc","refresh_token":"ref1","expires_in":900,"device_id":"d1"}"#)
            .create();
        let _account = server
            .mock("GET", "/v1/account")
            .match_header("authorization", "Bearer acc")
            .with_body(ACCOUNT)
            .create();
        assert_eq!(
            session.login_poll("dc").unwrap(),
            CloudLoginPoll::Done {
                email: "a@b.nl".into()
            }
        );
        assert_eq!(stored(&secrets).as_deref(), Some("ref1"));
        assert!(session.signed_in());
        assert_eq!(session.access_token().unwrap(), "acc");
    }

    #[test]
    fn poll_slow_down_and_expired_map() {
        let mut server = mockito::Server::new();
        let (_, session) = fixture(&server);
        let slow = server
            .mock("POST", "/v1/auth/token")
            .with_status(400)
            .with_body(r#"{"error":"slow_down"}"#)
            .create();
        assert_eq!(session.login_poll("dc").unwrap(), CloudLoginPoll::SlowDown);
        slow.remove();
        let _expired = server
            .mock("POST", "/v1/auth/token")
            .with_status(400)
            .with_body(r#"{"error":"expired_token"}"#)
            .create();
        assert_eq!(session.login_poll("dc").unwrap(), CloudLoginPoll::Expired);
    }

    #[test]
    fn access_token_refreshes_when_near_expiry_and_rotates() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        let first = server
            .mock("POST", "/v1/auth/refresh")
            .match_body(Matcher::Json(json!({"refresh_token": "old"})))
            .with_body(r#"{"access_token":"a1","refresh_token":"new1","expires_in":30}"#)
            .expect(1)
            .create();
        assert_eq!(session.access_token().unwrap(), "a1");
        assert_eq!(stored(&secrets).as_deref(), Some("new1"));
        first.assert();
        first.remove();

        let second = server
            .mock("POST", "/v1/auth/refresh")
            .match_body(Matcher::Json(json!({"refresh_token": "new1"})))
            .with_body(r#"{"access_token":"a2","refresh_token":"new2","expires_in":900}"#)
            .expect(1)
            .create();
        assert_eq!(session.access_token().unwrap(), "a2");
        assert_eq!(stored(&secrets).as_deref(), Some("new2"));
        assert_eq!(session.access_token().unwrap(), "a2");
        second.assert();
    }

    #[test]
    fn concurrent_callers_refresh_once() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        let refresh = server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"a1","refresh_token":"new1","expires_in":900}"#)
            .expect(1)
            .create();
        let session = Arc::new(session);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let session = session.clone();
                std::thread::spawn(move || session.access_token().unwrap())
            })
            .collect();
        for handle in handles {
            assert_eq!(handle.join().unwrap(), "a1");
        }
        refresh.assert();
    }

    #[test]
    fn revoked_session_signs_out_and_keeps_the_vault_key() {
        for code in ["session_revoked", "unauthorized"] {
            let mut server = mockito::Server::new();
            let (secrets, session) = fixture(&server);
            secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
            secrets.set(crate::cloud::VAULT_KEY_ACCOUNT.into(), Some("key".into()));
            let _refresh = server
                .mock("POST", "/v1/auth/refresh")
                .with_status(401)
                .with_body(format!(r#"{{"error":"{code}","message":"Nee."}}"#))
                .create();
            assert_eq!(session.access_token(), Err(PennelError::NotSignedIn));
            assert_eq!(stored(&secrets), None);
            assert_eq!(
                secrets
                    .get(crate::cloud::VAULT_KEY_ACCOUNT.into())
                    .as_deref(),
                Some("key")
            );
            assert!(!session.signed_in());
        }
    }

    #[test]
    fn missing_refresh_token_is_not_signed_in() {
        let server = mockito::Server::new();
        let (_, session) = fixture(&server);
        assert_eq!(session.access_token(), Err(PennelError::NotSignedIn));
    }

    #[test]
    fn account_and_devices_parse_server_shapes() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        let _refresh = server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"a1","refresh_token":"n","expires_in":900}"#)
            .create();
        let _account = server
            .mock("GET", "/v1/account")
            .with_body(ACCOUNT)
            .create();
        let _devices = server
            .mock("GET", "/v1/devices")
            .with_body(r#"[{"id":"d1","name":"Mac","platform":"macos","created_at":"c","last_seen_at":"2026-02-02T00:00:00Z","current":true},{"id":"d2","name":"Pc","platform":"windows","created_at":"c","last_seen_at":"2026-01-01T00:00:00Z","current":false}]"#)
            .create();
        let rename = server
            .mock("PATCH", "/v1/devices/d2")
            .match_body(Matcher::Json(json!({"name": "Werk"})))
            .with_status(204)
            .create();
        let revoke = server
            .mock("DELETE", "/v1/devices/d2")
            .with_status(204)
            .create();
        assert_eq!(
            session.account().unwrap(),
            CloudAccount {
                email: "a@b.nl".into(),
                audio_seconds: 5,
                tokens: 6,
                storage_bytes: 7,
                limit_audio_seconds: 50,
                limit_tokens: 60,
                limit_storage_bytes: 70
            }
        );
        let devices = session.devices().unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices[0].current && !devices[1].current);
        assert_eq!(devices[1].last_seen_iso, "2026-01-01T00:00:00Z");
        session.rename_device("d2", "Werk").unwrap();
        session.revoke_device("d2").unwrap();
        rename.assert();
        revoke.assert();
    }

    #[test]
    fn logout_revokes_own_device_and_clears_secrets() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        let _refresh = server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"a1","refresh_token":"n","expires_in":900}"#)
            .create();
        let _devices = server
            .mock("GET", "/v1/devices")
            .with_body(r#"[{"id":"d1","name":"x","platform":"p","created_at":"c","last_seen_at":"l","current":false},{"id":"d2","name":"y","platform":"p","created_at":"c","last_seen_at":"l","current":true}]"#)
            .create();
        let revoke = server
            .mock("DELETE", "/v1/devices/d2")
            .with_status(204)
            .create();
        session.logout();
        revoke.assert();
        assert_eq!(stored(&secrets), None);
    }

    #[test]
    fn logout_clears_secrets_even_if_server_unreachable() {
        let secrets = Arc::new(MemorySecrets::default());
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        secrets.set(crate::cloud::VAULT_KEY_ACCOUNT.into(), Some("key".into()));
        let session = Session::new("http://127.0.0.1:1".into(), secrets.clone());
        session.logout();
        assert_eq!(stored(&secrets), None);
        assert_eq!(secrets.get(crate::cloud::VAULT_KEY_ACCOUNT.into()), None);
    }

    #[test]
    fn delete_account_keeps_meeting_files() {
        let mut server = mockito::Server::new();
        let dir = tempfile::tempdir().unwrap();
        let engine = PennelEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let secrets = Arc::new(MemorySecrets::default());
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        engine.set_secret_store(secrets.clone());
        engine.use_cloud_session(Session::new(server.url(), secrets.clone()));
        let meeting = dir.path().join("Meetings").join("2026-01-01 Test");
        std::fs::create_dir_all(&meeting).unwrap();
        std::fs::write(meeting.join("meeting.md"), "notities").unwrap();
        let _refresh = server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"a1","refresh_token":"n","expires_in":900}"#)
            .create();
        let delete = server
            .mock("DELETE", "/v1/account")
            .with_status(204)
            .create();
        engine.cloud_delete_account().unwrap();
        delete.assert();
        assert_eq!(stored(&secrets), None);
        assert!(!engine.cloud_signed_in());
        assert_eq!(
            std::fs::read_to_string(meeting.join("meeting.md")).unwrap(),
            "notities"
        );
    }

    #[test]
    fn clear_during_in_flight_refresh_leaves_no_refresh_token() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        let _refresh = server
            .mock("POST", "/v1/auth/refresh")
            .with_body_from_request(|_| {
                std::thread::sleep(Duration::from_millis(400));
                br#"{"access_token":"a1","refresh_token":"new1","expires_in":900}"#.to_vec()
            })
            .create();
        let session = Arc::new(session);
        let refreshing = {
            let session = session.clone();
            std::thread::spawn(move || {
                let _ = session.access_token();
            })
        };
        std::thread::sleep(Duration::from_millis(150));
        session.clear_secrets();
        refreshing.join().unwrap();
        assert_eq!(stored(&secrets), None);
        assert!(!session.signed_in());
    }

    #[test]
    fn unauthorized_call_refreshes_once_and_retries_with_new_token() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        let first = server
            .mock("POST", "/v1/auth/refresh")
            .match_body(Matcher::Json(json!({"refresh_token": "old"})))
            .with_body(r#"{"access_token":"a1","refresh_token":"new1","expires_in":900}"#)
            .expect(1)
            .create();
        let second = server
            .mock("POST", "/v1/auth/refresh")
            .match_body(Matcher::Json(json!({"refresh_token": "new1"})))
            .with_body(r#"{"access_token":"a2","refresh_token":"new2","expires_in":900}"#)
            .expect(1)
            .create();
        let rejected = server
            .mock("GET", "/v1/account")
            .match_header("authorization", "Bearer a1")
            .with_status(401)
            .with_body(r#"{"error":"unauthorized"}"#)
            .expect(1)
            .create();
        let accepted = server
            .mock("GET", "/v1/account")
            .match_header("authorization", "Bearer a2")
            .with_body(ACCOUNT)
            .expect(1)
            .create();
        assert_eq!(session.account().unwrap().email, "a@b.nl");
        first.assert();
        second.assert();
        rejected.assert();
        accepted.assert();
        assert_eq!(stored(&secrets).as_deref(), Some("new2"));
    }

    #[test]
    fn forced_refresh_revoked_signs_out_and_keeps_the_vault_key() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        secrets.set(REFRESH_ACCOUNT.into(), Some("old".into()));
        secrets.set(crate::cloud::VAULT_KEY_ACCOUNT.into(), Some("key".into()));
        let _first = server
            .mock("POST", "/v1/auth/refresh")
            .match_body(Matcher::Json(json!({"refresh_token": "old"})))
            .with_body(r#"{"access_token":"a1","refresh_token":"new1","expires_in":900}"#)
            .create();
        let _revoked = server
            .mock("POST", "/v1/auth/refresh")
            .match_body(Matcher::Json(json!({"refresh_token": "new1"})))
            .with_status(401)
            .with_body(r#"{"error":"session_revoked"}"#)
            .create();
        let _rejected = server
            .mock("GET", "/v1/account")
            .with_status(401)
            .with_body(r#"{"error":"unauthorized"}"#)
            .create();
        assert_eq!(session.account(), Err(PennelError::NotSignedIn));
        assert_eq!(stored(&secrets), None);
        assert_eq!(
            secrets
                .get(crate::cloud::VAULT_KEY_ACCOUNT.into())
                .as_deref(),
            Some("key")
        );
    }

    #[test]
    fn login_done_survives_failing_account_lookup() {
        let mut server = mockito::Server::new();
        let (secrets, session) = fixture(&server);
        let _done = server
            .mock("POST", "/v1/auth/token")
            .with_body(r#"{"access_token":"acc","refresh_token":"ref1","expires_in":900,"device_id":"d1"}"#)
            .create();
        let _account = server
            .mock("GET", "/v1/account")
            .with_status(500)
            .with_body(r#"{"error":"internal"}"#)
            .create();
        assert_eq!(
            session.login_poll("dc").unwrap(),
            CloudLoginPoll::Done {
                email: String::new()
            }
        );
        assert_eq!(stored(&secrets).as_deref(), Some("ref1"));
        assert!(session.signed_in());
    }
}
