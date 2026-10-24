use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::PennelError;

const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at_ms: i64,
    pub app_bundle_id: Option<String>,
    pub raw: String,
    pub text: String,
    pub status: HistoryStatus,
    pub error: Option<String>,
    pub has_audio: bool,
}

pub struct HistoryStore {
    conn: Connection,
    audio_dir: PathBuf,
}

impl HistoryStore {
    pub fn open(data_dir: &Path) -> Result<Self, PennelError> {
        let audio_dir = data_dir.join("failed-audio");
        std::fs::create_dir_all(&audio_dir).map_err(PennelError::storage)?;
        let conn =
            Connection::open(data_dir.join("history.sqlite")).map_err(PennelError::storage)?;
        let version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(PennelError::storage)?;
        if version > 1 {
            return Err(PennelError::Storage {
                message: "de dicteergeschiedenis is gemaakt met een nieuwere Pennel-versie".into(),
            });
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dictations (
                id TEXT PRIMARY KEY,
                created_at_ms INTEGER NOT NULL,
                app_bundle_id TEXT,
                raw TEXT NOT NULL,
                text TEXT NOT NULL,
                status TEXT NOT NULL,
                error TEXT,
                has_audio INTEGER NOT NULL
            );
            PRAGMA user_version = 1;",
        )
        .map_err(PennelError::storage)?;
        Ok(Self { conn, audio_dir })
    }

    fn audio_path(&self, id: &str) -> PathBuf {
        self.audio_dir.join(format!("{id}.wav"))
    }

    pub fn insert_done(
        &self,
        app: Option<&str>,
        raw: &str,
        text: &str,
        now_ms: i64,
    ) -> Result<String, PennelError> {
        let id = uuid::Uuid::now_v7().to_string();
        self.conn
            .execute(
                "INSERT INTO dictations VALUES (?1, ?2, ?3, ?4, ?5, 'done', NULL, 0)",
                params![id, now_ms, app, raw, text],
            )
            .map_err(PennelError::storage)?;
        Ok(id)
    }

    pub fn insert_failed(
        &self,
        app: Option<&str>,
        wav: &[u8],
        error: &str,
        now_ms: i64,
    ) -> Result<String, PennelError> {
        let id = uuid::Uuid::now_v7().to_string();
        std::fs::write(self.audio_path(&id), wav).map_err(PennelError::storage)?;
        self.conn
            .execute(
                "INSERT INTO dictations VALUES (?1, ?2, ?3, '', '', 'failed', ?4, 1)",
                params![id, now_ms, app, error],
            )
            .map_err(PennelError::storage)?;
        Ok(id)
    }

    pub fn mark_done(&self, id: &str, raw: &str, text: &str) -> Result<(), PennelError> {
        let changed = self
            .conn
            .execute(
                "UPDATE dictations SET raw = ?2, text = ?3, status = 'done', error = NULL, has_audio = 0 WHERE id = ?1",
                params![id, raw, text],
            )
            .map_err(PennelError::storage)?;
        if changed == 0 {
            return Err(PennelError::NotFound);
        }
        remove_if_exists(&self.audio_path(id))
    }

    pub fn failed_audio(&self, id: &str) -> Result<Vec<u8>, PennelError> {
        match std::fs::read(self.audio_path(id)) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(PennelError::NotFound),
            Err(e) => Err(PennelError::storage(e)),
        }
    }

    pub fn app_bundle_id(&self, id: &str) -> Result<Option<String>, PennelError> {
        self.conn
            .query_row(
                "SELECT app_bundle_id FROM dictations WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(PennelError::storage)?
            .ok_or(PennelError::NotFound)
    }

    pub fn list(&self, limit: u32) -> Result<Vec<HistoryEntry>, PennelError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, created_at_ms, app_bundle_id, raw, text, status, error, has_audio
                 FROM dictations ORDER BY created_at_ms DESC, id DESC LIMIT ?1",
            )
            .map_err(PennelError::storage)?;
        let rows = stmt
            .query_map([limit], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    created_at_ms: row.get(1)?,
                    app_bundle_id: row.get(2)?,
                    raw: row.get(3)?,
                    text: row.get(4)?,
                    status: if row.get::<_, String>(5)? == "failed" {
                        HistoryStatus::Failed
                    } else {
                        HistoryStatus::Done
                    },
                    error: row.get(6)?,
                    has_audio: row.get(7)?,
                })
            })
            .map_err(PennelError::storage)?;
        rows.collect::<Result<_, _>>().map_err(PennelError::storage)
    }

    pub fn prune(&self, retention_days: u32, now_ms: i64) -> Result<u32, PennelError> {
        if retention_days == 0 {
            return Ok(0);
        }
        let cutoff = now_ms - i64::from(retention_days) * DAY_MS;
        let ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id FROM dictations WHERE created_at_ms < ?1")
                .map_err(PennelError::storage)?;
            stmt.query_map([cutoff], |row| row.get(0))
                .map_err(PennelError::storage)?
                .collect::<Result<_, _>>()
                .map_err(PennelError::storage)?
        };
        for id in &ids {
            self.delete(id)?;
        }
        Ok(ids.len() as u32)
    }

    pub fn delete(&self, id: &str) -> Result<(), PennelError> {
        self.conn
            .execute("DELETE FROM dictations WHERE id = ?1", [id])
            .map_err(PennelError::storage)?;
        remove_if_exists(&self.audio_path(id))
    }
}

fn remove_if_exists(path: &Path) -> Result<(), PennelError> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(PennelError::storage(e)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 86_400_000;

    #[test]
    fn insert_and_list_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(dir.path()).unwrap();
        store
            .insert_done(Some("com.apple.TextEdit"), "een", "Een.", 1_000)
            .unwrap();
        store.insert_done(None, "twee", "Twee.", 2_000).unwrap();
        let entries = store.list(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "Twee.");
        assert_eq!(
            entries[1].app_bundle_id.as_deref(),
            Some("com.apple.TextEdit")
        );
        assert_eq!(entries[1].status, HistoryStatus::Done);
        assert!(!entries[1].has_audio);
    }

    #[test]
    fn failed_entry_keeps_audio_until_marked_done() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(dir.path()).unwrap();
        let id = store
            .insert_failed(Some("com.apple.TextEdit"), b"RIFFdata", "http 503", 1_000)
            .unwrap();
        let entry = &store.list(1).unwrap()[0];
        assert_eq!(entry.status, HistoryStatus::Failed);
        assert_eq!(entry.error.as_deref(), Some("http 503"));
        assert!(entry.has_audio);
        assert_eq!(store.failed_audio(&id).unwrap(), b"RIFFdata");

        store.mark_done(&id, "hallo", "Hallo.").unwrap();
        let entry = &store.list(1).unwrap()[0];
        assert_eq!(entry.status, HistoryStatus::Done);
        assert_eq!(entry.text, "Hallo.");
        assert!(!entry.has_audio);
        assert_eq!(store.failed_audio(&id).unwrap_err(), PennelError::NotFound);
    }

    #[test]
    fn prune_removes_old_rows_and_audio() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(dir.path()).unwrap();
        let now = 100 * DAY_MS;
        let old = store
            .insert_failed(None, b"old", "x", now - 31 * DAY_MS)
            .unwrap();
        store
            .insert_done(None, "new", "New.", now - DAY_MS)
            .unwrap();
        assert_eq!(store.prune(30, now).unwrap(), 1);
        assert_eq!(store.list(10).unwrap().len(), 1);
        assert!(
            !dir.path()
                .join("failed-audio")
                .join(format!("{old}.wav"))
                .exists()
        );
    }

    #[test]
    fn retention_zero_keeps_everything() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(dir.path()).unwrap();
        store.insert_done(None, "a", "A.", 0).unwrap();
        assert_eq!(store.prune(0, 1_000 * DAY_MS).unwrap(), 0);
        assert_eq!(store.list(10).unwrap().len(), 1);
    }

    #[test]
    fn unknown_id_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(dir.path()).unwrap();
        assert_eq!(
            store.failed_audio("nope").unwrap_err(),
            PennelError::NotFound
        );
        assert_eq!(
            store.mark_done("nope", "a", "b").unwrap_err(),
            PennelError::NotFound
        );
    }

    #[test]
    fn delete_removes_row_and_audio() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::open(dir.path()).unwrap();
        let id = store.insert_failed(None, b"a", "x", 0).unwrap();
        store.delete(&id).unwrap();
        assert!(store.list(10).unwrap().is_empty());
        assert!(
            !dir.path()
                .join("failed-audio")
                .join(format!("{id}.wav"))
                .exists()
        );
    }

    #[test]
    fn newer_history_schema_is_not_downgraded() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("history.sqlite")).unwrap();
        conn.execute_batch("PRAGMA user_version = 99;").unwrap();
        assert!(HistoryStore::open(dir.path()).is_err());
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            99
        );
    }
}
