use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::{Value, json};

use crate::cloud::account::Session;
use crate::cloud::crypto::Kdf;
use crate::cloud::http::{Headers, agent, error_from, send_raw};
use crate::error::PennelError;

const REQUEST_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteVault {
    pub generation: u32,
    pub wrapped: Option<Vec<u8>>,
    pub kdf: Option<Kdf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    pub version: i64,
    pub size: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangePage {
    Ok {
        changes: Vec<Change>,
        cursor: i64,
        more: bool,
        generation: u32,
    },
    CursorExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutResult {
    Ok(i64),
    /// A missing object reports `{current: 0, deleted: true}`, like the server does.
    Conflict {
        current: i64,
        deleted: bool,
    },
    GenerationChanged,
}

pub trait SyncApi: Send + Sync {
    fn vault(&self) -> Result<RemoteVault, PennelError>;
    /// `create` sends `If-None-Match: *`; another device that created the vault first gives `VaultLocked`.
    fn put_vault(
        &self,
        generation: u32,
        wrapped: &[u8],
        kdf: &Kdf,
        create: bool,
    ) -> Result<(), PennelError>;
    /// Wipes every object and returns the new generation.
    fn reset_vault(&self, generation: u32) -> Result<u32, PennelError>;
    fn changes(&self, since: i64, limit: u32) -> Result<ChangePage, PennelError>;
    fn get(&self, id: &str) -> Result<Option<(i64, Vec<u8>)>, PennelError>;
    fn put(
        &self,
        id: &str,
        base: i64,
        generation: u32,
        blob: &[u8],
    ) -> Result<PutResult, PennelError>;
    fn delete(&self, id: &str, base: i64, generation: u32) -> Result<PutResult, PennelError>;
}

pub struct HttpSyncApi {
    session: Arc<Session>,
    http: ureq::Agent,
}

fn invalid(what: &str) -> PennelError {
    PennelError::InvalidResponse {
        message: format!("unexpected {what} response"),
    }
}

fn json_body(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

fn error_code(body: &Value) -> &str {
    body.get("error").and_then(Value::as_str).unwrap_or("")
}

fn version_of(body: &Value, what: &str) -> Result<i64, PennelError> {
    body.get("version")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(what))
}

fn write_result(status: u16, body: &[u8], what: &str) -> Result<PutResult, PennelError> {
    let body = json_body(body);
    match (status, error_code(&body)) {
        (200..=299, _) => Ok(PutResult::Ok(version_of(&body, what)?)),
        (409, "conflict") => {
            let current = body.get("current").ok_or_else(|| invalid(what))?;
            Ok(PutResult::Conflict {
                current: current
                    .get("version")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| invalid(what))?,
                deleted: current
                    .get("deleted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        }
        (409, "generation") => Ok(PutResult::GenerationChanged),
        _ => Err(error_from(status, &body)),
    }
}

impl HttpSyncApi {
    pub fn new(session: Arc<Session>) -> Self {
        Self {
            session,
            http: agent(REQUEST_TIMEOUT_SECS),
        }
    }

    /// One forced token refresh on 401, like `Session`'s JSON calls.
    fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, String)],
        content_type: &str,
        body: Option<&[u8]>,
    ) -> Result<(u16, Headers, Vec<u8>), PennelError> {
        let url = format!("{}{path}", self.session.base_url());
        let token = self.session.access_token()?;
        let response = send_raw(
            &self.http,
            method,
            &url,
            Some(&token),
            headers,
            content_type,
            body,
        )?;
        if response.0 != 401 {
            return Ok(response);
        }
        let token = self.session.refreshed_token()?;
        send_raw(
            &self.http,
            method,
            &url,
            Some(&token),
            headers,
            content_type,
            body,
        )
    }

    fn object_path(id: &str) -> String {
        format!("/v1/objects/{id}")
    }
}

impl SyncApi for HttpSyncApi {
    fn vault(&self) -> Result<RemoteVault, PennelError> {
        let (status, _, bytes) = self.request("GET", "/v1/vault", &[], "", None)?;
        let body = json_body(&bytes);
        if !(200..300).contains(&status) {
            return Err(error_from(status, &body));
        }
        let generation = body
            .get("generation")
            .and_then(Value::as_u64)
            .and_then(|g| u32::try_from(g).ok())
            .ok_or_else(|| invalid("vault"))?;
        let wrapped = match body.get("wrapped_key").and_then(Value::as_str) {
            Some(encoded) => Some(
                B64.decode(encoded.trim())
                    .map_err(|_| crate::cloud::crypto::vault_invalid())?,
            ),
            None => None,
        };
        let kdf = match body.get("kdf") {
            Some(Value::Null) | None => None,
            Some(value) => Some(
                serde_json::from_value(value.clone())
                    .map_err(|_| crate::cloud::crypto::vault_invalid())?,
            ),
        };
        if wrapped.is_some() && kdf.is_none() {
            return Err(crate::cloud::crypto::vault_invalid());
        }
        Ok(RemoteVault {
            generation,
            wrapped,
            kdf,
        })
    }

    fn put_vault(
        &self,
        generation: u32,
        wrapped: &[u8],
        kdf: &Kdf,
        create: bool,
    ) -> Result<(), PennelError> {
        let body = json!({
            "generation": generation,
            "wrapped_key": B64.encode(wrapped),
            "kdf": kdf,
        })
        .to_string();
        let headers: Vec<(&str, String)> = if create {
            vec![("If-None-Match", "*".into())]
        } else {
            Vec::new()
        };
        let (status, _, bytes) = self.request(
            "PUT",
            "/v1/vault",
            &headers,
            "application/json",
            Some(body.as_bytes()),
        )?;
        let body = json_body(&bytes);
        match (status, error_code(&body)) {
            (200..=299, _) => Ok(()),
            (409, "vault_exists") => Err(PennelError::VaultLocked),
            _ => Err(error_from(status, &body)),
        }
    }

    fn reset_vault(&self, generation: u32) -> Result<u32, PennelError> {
        let (status, _, bytes) = self.request(
            "DELETE",
            "/v1/vault",
            &[("X-Pennel-Generation", generation.to_string())],
            "",
            None,
        )?;
        let body = json_body(&bytes);
        if !(200..300).contains(&status) {
            return Err(error_from(status, &body));
        }
        body.get("generation")
            .and_then(Value::as_u64)
            .and_then(|g| u32::try_from(g).ok())
            .ok_or_else(|| invalid("vault reset"))
    }

    fn changes(&self, since: i64, limit: u32) -> Result<ChangePage, PennelError> {
        let path = format!("/v1/sync/changes?since={since}&limit={limit}");
        let (status, _, bytes) = self.request("GET", &path, &[], "", None)?;
        let body = json_body(&bytes);
        if status == 410 && error_code(&body) == "cursor_expired" {
            return Ok(ChangePage::CursorExpired);
        }
        if !(200..300).contains(&status) {
            return Err(error_from(status, &body));
        }
        let changes = body
            .get("changes")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("changes"))?
            .iter()
            .map(|change| {
                Some(Change {
                    id: change.get("id")?.as_str()?.to_string(),
                    version: change.get("version")?.as_i64()?,
                    size: change.get("size").and_then(Value::as_i64).unwrap_or(0),
                    deleted: change.get("deleted")?.as_bool()?,
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid("changes"))?;
        Ok(ChangePage::Ok {
            changes,
            cursor: body
                .get("cursor")
                .and_then(Value::as_i64)
                .ok_or_else(|| invalid("changes"))?,
            more: body.get("more").and_then(Value::as_bool).unwrap_or(false),
            generation: body
                .get("generation")
                .and_then(Value::as_u64)
                .and_then(|g| u32::try_from(g).ok())
                .ok_or_else(|| invalid("changes"))?,
        })
    }

    fn get(&self, id: &str) -> Result<Option<(i64, Vec<u8>)>, PennelError> {
        let (status, headers, bytes) =
            self.request("GET", &Self::object_path(id), &[], "", None)?;
        if status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&status) {
            return Err(error_from(status, &json_body(&bytes)));
        }
        let version = headers
            .get("etag")
            .map(|tag| tag.trim().trim_start_matches("W/").trim_matches('"'))
            .and_then(|tag| tag.parse().ok())
            .ok_or_else(|| invalid("object"))?;
        Ok(Some((version, bytes)))
    }

    fn put(
        &self,
        id: &str,
        base: i64,
        generation: u32,
        blob: &[u8],
    ) -> Result<PutResult, PennelError> {
        let (status, _, bytes) = self.request(
            "PUT",
            &Self::object_path(id),
            &[
                ("If-Match", base.to_string()),
                ("X-Pennel-Generation", generation.to_string()),
            ],
            "application/octet-stream",
            Some(blob),
        )?;
        write_result(status, &bytes, "upload")
    }

    fn delete(&self, id: &str, base: i64, generation: u32) -> Result<PutResult, PennelError> {
        let (status, _, bytes) = self.request(
            "DELETE",
            &Self::object_path(id),
            &[
                ("If-Match", base.to_string()),
                ("X-Pennel-Generation", generation.to_string()),
            ],
            "",
            None,
        )?;
        if status == 404 {
            return Ok(PutResult::Conflict {
                current: 0,
                deleted: true,
            });
        }
        write_result(status, &bytes, "delete")
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct Object {
        version: i64,
        deleted: bool,
        blob: Vec<u8>,
    }

    #[derive(Default)]
    struct Server {
        generation: u32,
        vault: Option<(Vec<u8>, Kdf)>,
        objects: HashMap<String, Object>,
        counter: i64,
        purged_through: i64,
        storage_limit: Option<usize>,
        offline: bool,
    }

    type Hook = Box<dyn FnMut(&str) + Send>;

    /// An in-memory Pennel Cloud with the server's version, tombstone and generation rules.
    pub(crate) struct FakeApi {
        server: Mutex<Server>,
        after_get: Mutex<Option<Hook>>,
    }

    impl FakeApi {
        pub(crate) fn new() -> Self {
            Self {
                server: Mutex::new(Server {
                    generation: 1,
                    ..Server::default()
                }),
                after_get: Mutex::new(None),
            }
        }

        pub(crate) fn set_offline(&self, offline: bool) {
            self.server.lock().unwrap().offline = offline;
        }

        pub(crate) fn set_storage_limit(&self, bytes: Option<usize>) {
            self.server.lock().unwrap().storage_limit = bytes;
        }

        /// Forgets every tombstone, as the server does after 90 days.
        pub(crate) fn purge_tombstones(&self) {
            let mut server = self.server.lock().unwrap();
            server.objects.retain(|_, object| !object.deleted);
            server.purged_through = server.counter;
        }

        pub(crate) fn on_get(&self, hook: impl FnMut(&str) + Send + 'static) {
            *self.after_get.lock().unwrap() = Some(Box::new(hook));
        }

        pub(crate) fn live_objects(&self) -> usize {
            let server = self.server.lock().unwrap();
            server.objects.values().filter(|o| !o.deleted).count()
        }

        fn check(server: &Server) -> Result<(), PennelError> {
            if server.offline {
                return Err(PennelError::Network {
                    message: "offline".into(),
                });
            }
            Ok(())
        }
    }

    impl SyncApi for FakeApi {
        fn vault(&self) -> Result<RemoteVault, PennelError> {
            let server = self.server.lock().unwrap();
            Self::check(&server)?;
            Ok(RemoteVault {
                generation: server.generation,
                wrapped: server.vault.as_ref().map(|(w, _)| w.clone()),
                kdf: server.vault.as_ref().map(|(_, k)| k.clone()),
            })
        }

        fn put_vault(
            &self,
            generation: u32,
            wrapped: &[u8],
            kdf: &Kdf,
            create: bool,
        ) -> Result<(), PennelError> {
            let mut server = self.server.lock().unwrap();
            Self::check(&server)?;
            if generation != server.generation {
                return Err(PennelError::Cloud {
                    code: "generation".into(),
                    message: "De kluis is opnieuw begonnen op een ander toestel.".into(),
                });
            }
            match (create, server.vault.is_some()) {
                (true, true) => return Err(PennelError::VaultLocked),
                (false, false) => {
                    return Err(PennelError::Cloud {
                        code: "vault_missing".into(),
                        message: "Er bestaat nog geen kluis.".into(),
                    });
                }
                _ => {}
            }
            server.vault = Some((wrapped.to_vec(), kdf.clone()));
            Ok(())
        }

        fn reset_vault(&self, generation: u32) -> Result<u32, PennelError> {
            let mut server = self.server.lock().unwrap();
            Self::check(&server)?;
            if generation != server.generation {
                return Err(PennelError::Cloud {
                    code: "generation".into(),
                    message: "De kluis is opnieuw begonnen op een ander toestel.".into(),
                });
            }
            server.objects.clear();
            server.vault = None;
            server.generation += 1;
            server.counter += 1;
            Ok(server.generation)
        }

        fn changes(&self, since: i64, limit: u32) -> Result<ChangePage, PennelError> {
            let server = self.server.lock().unwrap();
            Self::check(&server)?;
            if since > 0 && since < server.purged_through {
                return Ok(ChangePage::CursorExpired);
            }
            let mut rows: Vec<Change> = server
                .objects
                .iter()
                .filter(|(_, o)| o.version > since)
                .map(|(id, o)| Change {
                    id: id.clone(),
                    version: o.version,
                    size: o.blob.len() as i64,
                    deleted: o.deleted,
                })
                .collect();
            rows.sort_by_key(|c| c.version);
            let more = rows.len() > limit as usize;
            rows.truncate(limit as usize);
            Ok(ChangePage::Ok {
                cursor: rows.last().map_or(since, |c| c.version),
                changes: rows,
                more,
                generation: server.generation,
            })
        }

        fn get(&self, id: &str) -> Result<Option<(i64, Vec<u8>)>, PennelError> {
            let found = {
                let server = self.server.lock().unwrap();
                Self::check(&server)?;
                server
                    .objects
                    .get(id)
                    .filter(|o| !o.deleted)
                    .map(|o| (o.version, o.blob.clone()))
            };
            if let Some(hook) = self.after_get.lock().unwrap().as_mut() {
                hook(id);
            }
            Ok(found)
        }

        fn put(
            &self,
            id: &str,
            base: i64,
            generation: u32,
            blob: &[u8],
        ) -> Result<PutResult, PennelError> {
            let mut server = self.server.lock().unwrap();
            Self::check(&server)?;
            if generation != server.generation {
                return Ok(PutResult::GenerationChanged);
            }
            let matches = match server.objects.get(id) {
                None => base == 0,
                Some(o) if o.deleted => base == 0 || base == o.version,
                Some(o) => base == o.version,
            };
            if !matches {
                let (current, deleted) = server
                    .objects
                    .get(id)
                    .map_or((0, true), |o| (o.version, o.deleted));
                return Ok(PutResult::Conflict { current, deleted });
            }
            if let Some(limit) = server.storage_limit {
                let used: usize = server
                    .objects
                    .iter()
                    .filter(|(other, o)| !o.deleted && other.as_str() != id)
                    .map(|(_, o)| o.blob.len())
                    .sum();
                if used + blob.len() > limit {
                    return Err(PennelError::Cloud {
                        code: "storage_full".into(),
                        message: "Je cloudopslag is vol (1 GB). Lokaal blijft alles werken.".into(),
                    });
                }
            }
            server.counter += 1;
            let version = server.counter;
            server.objects.insert(
                id.to_string(),
                Object {
                    version,
                    deleted: false,
                    blob: blob.to_vec(),
                },
            );
            Ok(PutResult::Ok(version))
        }

        fn delete(&self, id: &str, base: i64, generation: u32) -> Result<PutResult, PennelError> {
            let mut server = self.server.lock().unwrap();
            Self::check(&server)?;
            if generation != server.generation {
                return Ok(PutResult::GenerationChanged);
            }
            let Some(current) = server.objects.get(id) else {
                return Ok(PutResult::Conflict {
                    current: 0,
                    deleted: true,
                });
            };
            if current.deleted && current.version == base {
                return Ok(PutResult::Ok(base));
            }
            if current.deleted || current.version != base {
                return Ok(PutResult::Conflict {
                    current: current.version,
                    deleted: current.deleted,
                });
            }
            server.counter += 1;
            let version = server.counter;
            server.objects.insert(
                id.to_string(),
                Object {
                    version,
                    deleted: true,
                    blob: Vec::new(),
                },
            );
            Ok(PutResult::Ok(version))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::REFRESH_ACCOUNT;
    use crate::cloud::SecretStore;
    use crate::cloud::tests::MemorySecrets;
    use mockito::Matcher;

    const ID: &str = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";

    fn fixture(server: &mut mockito::ServerGuard) -> (HttpSyncApi, mockito::Mock) {
        let secrets = Arc::new(MemorySecrets::default());
        secrets.set(REFRESH_ACCOUNT.into(), Some("r".into()));
        let refresh = server
            .mock("POST", "/v1/auth/refresh")
            .with_body(r#"{"access_token":"tok","refresh_token":"r2","expires_in":900}"#)
            .create();
        let session = Arc::new(Session::new(server.url(), secrets));
        (HttpSyncApi::new(session), refresh)
    }

    #[test]
    fn put_sends_if_match_and_generation() {
        let mut server = mockito::Server::new();
        let (api, _refresh) = fixture(&mut server);
        let put = server
            .mock("PUT", format!("/v1/objects/{ID}").as_str())
            .match_header("authorization", "Bearer tok")
            .match_header("if-match", "7")
            .match_header("x-pennel-generation", "3")
            .match_header("content-type", "application/octet-stream")
            .match_body(Matcher::Exact("blob".into()))
            .with_body(r#"{"version":8}"#)
            .create();
        assert_eq!(api.put(ID, 7, 3, b"blob").unwrap(), PutResult::Ok(8));
        put.assert();

        let delete = server
            .mock("DELETE", format!("/v1/objects/{ID}").as_str())
            .match_header("if-match", "8")
            .match_header("x-pennel-generation", "3")
            .with_body(r#"{"version":9}"#)
            .create();
        assert_eq!(api.delete(ID, 8, 3).unwrap(), PutResult::Ok(9));
        delete.assert();
    }

    #[test]
    fn conflict_409_maps_to_conflict() {
        let mut server = mockito::Server::new();
        let (api, _refresh) = fixture(&mut server);
        let _put = server
            .mock("PUT", format!("/v1/objects/{ID}").as_str())
            .with_status(409)
            .with_body(r#"{"error":"conflict","message":"Dit bestand is intussen op een ander toestel gewijzigd.","current":{"version":12,"deleted":true}}"#)
            .create();
        assert_eq!(
            api.put(ID, 3, 1, b"x").unwrap(),
            PutResult::Conflict {
                current: 12,
                deleted: true
            }
        );
        let _missing = server
            .mock("DELETE", format!("/v1/objects/{ID}").as_str())
            .with_status(404)
            .with_body(r#"{"error":"not_found","message":"Niet gevonden."}"#)
            .create();
        assert_eq!(
            api.delete(ID, 3, 1).unwrap(),
            PutResult::Conflict {
                current: 0,
                deleted: true
            }
        );
    }

    #[test]
    fn generation_409_maps() {
        let mut server = mockito::Server::new();
        let (api, _refresh) = fixture(&mut server);
        let _put = server
            .mock("PUT", format!("/v1/objects/{ID}").as_str())
            .with_status(409)
            .with_body(r#"{"error":"generation","message":"De kluis is opnieuw begonnen op een ander toestel."}"#)
            .create();
        assert_eq!(
            api.put(ID, 0, 1, b"x").unwrap(),
            PutResult::GenerationChanged
        );
        let _full = server
            .mock("PUT", format!("/v1/objects/{ID}").as_str())
            .match_header("if-match", "5")
            .with_status(403)
            .with_body(r#"{"error":"storage_full","message":"Je cloudopslag is vol (1 GB). Lokaal blijft alles werken."}"#)
            .create();
        assert_eq!(
            api.put(ID, 5, 1, b"x"),
            Err(PennelError::Cloud {
                code: "storage_full".into(),
                message: "Je cloudopslag is vol (1 GB). Lokaal blijft alles werken.".into()
            })
        );
    }

    #[test]
    fn cursor_expired_410_maps() {
        let mut server = mockito::Server::new();
        let (api, _refresh) = fixture(&mut server);
        let _gone = server
            .mock("GET", "/v1/sync/changes")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("since".into(), "40".into()),
                Matcher::UrlEncoded("limit".into(), "500".into()),
            ]))
            .with_status(410)
            .with_body(r#"{"error":"cursor_expired","message":"Je toestel was te lang offline; volledige vergelijking nodig."}"#)
            .create();
        assert_eq!(api.changes(40, 500).unwrap(), ChangePage::CursorExpired);
        let _page = server
            .mock("GET", "/v1/sync/changes")
            .match_query(Matcher::UrlEncoded("since".into(), "0".into()))
            .with_body(format!(r#"{{"changes":[{{"id":"{ID}","version":4,"size":10,"deleted":false}}],"cursor":4,"more":true,"generation":2}}"#))
            .create();
        assert_eq!(
            api.changes(0, 500).unwrap(),
            ChangePage::Ok {
                changes: vec![Change {
                    id: ID.into(),
                    version: 4,
                    size: 10,
                    deleted: false
                }],
                cursor: 4,
                more: true,
                generation: 2
            }
        );
    }

    #[test]
    fn get_reads_quoted_etag_and_maps_404() {
        let mut server = mockito::Server::new();
        let (api, _refresh) = fixture(&mut server);
        let _found = server
            .mock("GET", format!("/v1/objects/{ID}").as_str())
            .with_header("ETag", "\"15\"")
            .with_header("X-Pennel-Generation", "1")
            .with_body(vec![1u8, 2, 3])
            .create();
        assert_eq!(api.get(ID).unwrap(), Some((15, vec![1, 2, 3])));
        let other = "1".repeat(64);
        let _missing = server
            .mock("GET", format!("/v1/objects/{other}").as_str())
            .with_status(404)
            .with_body(r#"{"error":"not_found","message":"Niet gevonden."}"#)
            .create();
        assert_eq!(api.get(&other).unwrap(), None);
    }

    #[test]
    fn vault_create_sends_if_none_match_and_maps_vault_exists() {
        let mut server = mockito::Server::new();
        let (api, _refresh) = fixture(&mut server);
        let kdf = Kdf {
            alg: "argon2id".into(),
            m_kib: 64,
            t: 1,
            p: 1,
            salt: "c2FsdHNhbHRzYWx0c2FsdA==".into(),
        };
        let _get = server
            .mock("GET", "/v1/vault")
            .with_body(r#"{"generation":1,"wrapped_key":null,"kdf":null}"#)
            .create();
        assert_eq!(
            api.vault().unwrap(),
            RemoteVault {
                generation: 1,
                wrapped: None,
                kdf: None
            }
        );
        let created = server
            .mock("PUT", "/v1/vault")
            .match_header("if-none-match", "*")
            .match_body(Matcher::PartialJson(
                json!({"generation": 1, "wrapped_key": "AQI=", "kdf": {"alg": "argon2id"}}),
            ))
            .with_status(204)
            .create();
        api.put_vault(1, &[1, 2], &kdf, true).unwrap();
        created.assert();
        created.remove();
        let _exists = server
            .mock("PUT", "/v1/vault")
            .with_status(409)
            .with_body(r#"{"error":"vault_exists","message":"Er bestaat al een kluis. Voer je herstelcode in."}"#)
            .create();
        assert_eq!(
            api.put_vault(1, &[1, 2], &kdf, true),
            Err(PennelError::VaultLocked)
        );
        let reset = server
            .mock("DELETE", "/v1/vault")
            .match_header("x-pennel-generation", "1")
            .with_body(r#"{"generation":2}"#)
            .create();
        assert_eq!(api.reset_vault(1).unwrap(), 2);
        reset.assert();
    }
}
