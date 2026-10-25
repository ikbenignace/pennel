use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::PennelError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub object_id: String,
    pub server_version: i64,
    /// Empty when the local file must be uploaded regardless of its content.
    pub synced_hash: String,
    pub folder: Option<String>,
}

/// What this device last agreed with the server, per file. Lives in the data directory,
/// never in the meetings folder.
pub struct SyncState {
    conn: Connection,
}

const CURSOR: &str = "cursor";
const GENERATION: &str = "generation";
const ROOT: &str = "root";
const LAST_SUCCESS: &str = "last_success_ms";
const DEVICE: &str = "device";
const CONFIRM_DELETES: &str = "confirm_deletes";

fn entry_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        key: row.get(0)?,
        object_id: row.get(1)?,
        server_version: row.get(2)?,
        synced_hash: row.get(3)?,
        folder: row.get(4)?,
    })
}

const COLUMNS: &str = "key, object_id, server_version, synced_hash, folder";

impl SyncState {
    pub fn open(data_dir: &Path) -> Result<Self, PennelError> {
        Self::init(Connection::open(data_dir.join("sync.sqlite")).map_err(PennelError::storage)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> Result<Self, PennelError> {
        conn.execute_batch(
            "pragma journal_mode = wal;
             create table if not exists entries (
                 key text primary key,
                 object_id text not null,
                 server_version integer not null,
                 synced_hash text not null,
                 folder text
             );
             create index if not exists entries_object on entries(object_id);
             create table if not exists meta (k text primary key, v text not null);
             create table if not exists deferred (object_id text primary key);
             create table if not exists intended (uid text primary key);",
        )
        .map_err(PennelError::storage)?;
        Ok(Self { conn })
    }

    pub fn entry(&self, key: &str) -> Result<Option<Entry>, PennelError> {
        self.conn
            .query_row(
                &format!("select {COLUMNS} from entries where key = ?1"),
                [key],
                entry_from,
            )
            .optional()
            .map_err(PennelError::storage)
    }

    pub fn entry_by_object(&self, object_id: &str) -> Result<Option<Entry>, PennelError> {
        self.conn
            .query_row(
                &format!("select {COLUMNS} from entries where object_id = ?1"),
                [object_id],
                entry_from,
            )
            .optional()
            .map_err(PennelError::storage)
    }

    pub fn entries(&self) -> Result<Vec<Entry>, PennelError> {
        let mut statement = self
            .conn
            .prepare(&format!("select {COLUMNS} from entries order by key"))
            .map_err(PennelError::storage)?;
        statement
            .query_map([], entry_from)
            .and_then(Iterator::collect)
            .map_err(PennelError::storage)
    }

    pub fn put(&self, entry: &Entry) -> Result<(), PennelError> {
        self.conn
            .execute(
                &format!("insert or replace into entries ({COLUMNS}) values (?1, ?2, ?3, ?4, ?5)"),
                params![
                    entry.key,
                    entry.object_id,
                    entry.server_version,
                    entry.synced_hash,
                    entry.folder
                ],
            )
            .map(drop)
            .map_err(PennelError::storage)
    }

    pub fn remove(&self, key: &str) -> Result<(), PennelError> {
        self.conn
            .execute("delete from entries where key = ?1", [key])
            .map(drop)
            .map_err(PennelError::storage)
    }

    /// Keeps the folder of every file of a meeting current, so a vanished file can be told
    /// apart from a meeting whose `meeting.md` became unreadable.
    pub fn set_meeting_folder(&self, uid: &str, folder: &str) -> Result<(), PennelError> {
        self.conn
            .execute(
                "update entries set folder = ?2 where substr(key, 1, ?3) = ?1 and folder is not ?2",
                params![
                    format!("meeting/{uid}/"),
                    folder,
                    format!("meeting/{uid}/").len() as i64
                ],
            )
            .map(drop)
            .map_err(PennelError::storage)
    }

    pub fn folder_of_uid(&self, uid: &str) -> Result<Option<String>, PennelError> {
        self.conn
            .query_row(
                "select folder from entries where substr(key, 1, ?2) = ?1 and folder is not null limit 1",
                params![format!("meeting/{uid}/"), format!("meeting/{uid}/").len() as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(PennelError::storage)
    }

    /// The uid this device last synced for `folder`, so a meeting whose `pennel_id` line was
    /// lost keeps its identity instead of becoming a duplicate.
    pub fn uid_for_folder(&self, folder: &str) -> Result<Option<String>, PennelError> {
        let key: Option<String> = self
            .conn
            .query_row(
                "select key from entries where folder = ?1 and key like 'meeting/%' limit 1",
                [folder],
                |row| row.get(0),
            )
            .optional()
            .map_err(PennelError::storage)?;
        Ok(key.and_then(|key| key.split('/').nth(1).map(str::to_string)))
    }

    pub fn defer(&self, object_id: &str) -> Result<(), PennelError> {
        self.conn
            .execute(
                "insert or ignore into deferred (object_id) values (?1)",
                [object_id],
            )
            .map(drop)
            .map_err(PennelError::storage)
    }

    pub fn deferred(&self) -> Result<Vec<String>, PennelError> {
        let mut statement = self
            .conn
            .prepare("select object_id from deferred order by object_id")
            .map_err(PennelError::storage)?;
        statement
            .query_map([], |row| row.get(0))
            .and_then(Iterator::collect)
            .map_err(PennelError::storage)
    }

    pub fn undefer(&self, object_id: &str) -> Result<(), PennelError> {
        self.conn
            .execute("delete from deferred where object_id = ?1", [object_id])
            .map(drop)
            .map_err(PennelError::storage)
    }

    fn meta(&self, key: &str) -> Result<Option<String>, PennelError> {
        self.conn
            .query_row("select v from meta where k = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(PennelError::storage)
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<(), PennelError> {
        self.conn
            .execute(
                "insert or replace into meta (k, v) values (?1, ?2)",
                [key, value],
            )
            .map(drop)
            .map_err(PennelError::storage)
    }

    /// A meeting the user deleted in Pennel itself: its deletion is pushed without the brake.
    pub fn mark_deleted(&self, uid: &str) -> Result<(), PennelError> {
        self.conn
            .execute("insert or ignore into intended (uid) values (?1)", [uid])
            .map(drop)
            .map_err(PennelError::storage)
    }

    pub fn deleted_on_purpose(&self) -> Result<HashSet<String>, PennelError> {
        let mut statement = self
            .conn
            .prepare("select uid from intended")
            .map_err(PennelError::storage)?;
        statement
            .query_map([], |row| row.get(0))
            .and_then(Iterator::collect)
            .map_err(PennelError::storage)
    }

    pub fn unmark_deleted(&self, uid: &str) -> Result<(), PennelError> {
        self.conn
            .execute("delete from intended where uid = ?1", [uid])
            .map(drop)
            .map_err(PennelError::storage)
    }

    /// Lets the next pass push deletions the mass-delete brake held back.
    pub fn confirm_deletes(&self) -> Result<(), PennelError> {
        self.set_meta(CONFIRM_DELETES, "1")
    }

    pub fn deletes_confirmed(&self) -> Result<bool, PennelError> {
        Ok(self.meta(CONFIRM_DELETES)?.is_some())
    }

    pub fn clear_confirmation(&self) -> Result<(), PennelError> {
        self.conn
            .execute("delete from meta where k = ?1", [CONFIRM_DELETES])
            .map(drop)
            .map_err(PennelError::storage)
    }

    pub fn cursor(&self) -> Result<i64, PennelError> {
        Ok(self.meta(CURSOR)?.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    pub fn set_cursor(&self, cursor: i64) -> Result<(), PennelError> {
        self.set_meta(CURSOR, &cursor.to_string())
    }

    pub fn generation(&self) -> Result<Option<u32>, PennelError> {
        Ok(self.meta(GENERATION)?.and_then(|v| v.parse().ok()))
    }

    /// A new generation means a new vault key: every recorded version and object id is void.
    pub fn set_generation(&self, generation: u32) -> Result<(), PennelError> {
        if self
            .generation()?
            .is_some_and(|current| current != generation)
        {
            self.forget_progress()?;
        }
        self.set_meta(GENERATION, &generation.to_string())
    }

    pub fn root(&self) -> Result<Option<String>, PennelError> {
        self.meta(ROOT)
    }

    pub fn set_root(&self, root: &str) -> Result<(), PennelError> {
        self.set_meta(ROOT, root)
    }

    pub fn last_success_ms(&self) -> Result<Option<i64>, PennelError> {
        Ok(self.meta(LAST_SUCCESS)?.and_then(|v| v.parse().ok()))
    }

    pub fn set_last_success_ms(&self, ms: i64) -> Result<(), PennelError> {
        self.set_meta(LAST_SUCCESS, &ms.to_string())
    }

    pub fn device(&self) -> Result<Option<String>, PennelError> {
        self.meta(DEVICE)
    }

    pub fn set_device(&self, name: &str) -> Result<(), PennelError> {
        self.set_meta(DEVICE, name)
    }

    /// Drops what this device knows about the server; local files are untouched and the next
    /// pass compares everything again (equal files match, different ones become conflict copies).
    pub fn forget_progress(&self) -> Result<(), PennelError> {
        self.conn
            .execute_batch(
                "delete from entries; delete from deferred; delete from intended;
                 delete from meta where k in ('cursor', 'confirm_deletes');",
            )
            .map_err(PennelError::storage)
    }

    pub fn clear(&self) -> Result<(), PennelError> {
        self.conn
            .execute_batch(
                "delete from entries; delete from deferred; delete from intended; delete from meta;",
            )
            .map_err(PennelError::storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, folder: Option<&str>) -> Entry {
        Entry {
            key: key.into(),
            object_id: format!("id-{key}"),
            server_version: 3,
            synced_hash: "h".into(),
            folder: folder.map(str::to_string),
        }
    }

    #[test]
    fn entries_meta_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let state = SyncState::open(dir.path()).unwrap();
        state.put(&entry("meeting/u1/notes.md", Some("A"))).unwrap();
        state.put(&entry("templates/x.md", None)).unwrap();
        assert_eq!(
            state
                .entry_by_object("id-templates/x.md")
                .unwrap()
                .unwrap()
                .key,
            "templates/x.md"
        );
        state.set_meeting_folder("u1", "B").unwrap();
        assert_eq!(
            state
                .entry("meeting/u1/notes.md")
                .unwrap()
                .unwrap()
                .folder
                .as_deref(),
            Some("B")
        );
        assert_eq!(state.uid_for_folder("B").unwrap().as_deref(), Some("u1"));
        state.set_cursor(42).unwrap();
        state.set_generation(1).unwrap();
        assert_eq!(state.cursor().unwrap(), 42);
        state.set_generation(1).unwrap();
        assert_eq!(state.cursor().unwrap(), 42);
        state.set_generation(2).unwrap();
        assert_eq!(state.cursor().unwrap(), 0);
        assert!(state.entries().unwrap().is_empty());
        state.put(&entry("templates/x.md", None)).unwrap();
        state.defer("abc").unwrap();
        state.set_device("Mac").unwrap();
        state.clear().unwrap();
        assert!(state.entries().unwrap().is_empty());
        assert!(state.deferred().unwrap().is_empty());
        assert_eq!(state.generation().unwrap(), None);
        assert_eq!(state.device().unwrap(), None);
    }
}
