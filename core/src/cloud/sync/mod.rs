pub mod api;
pub mod scan;
pub mod state;
pub mod trash;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use api::{ChangePage, PutResult, SyncApi};
use chrono::{Local, TimeZone};
use scan::{
    Key, LocalFile, MEETING_FILE, Slot, TEMPLATES_DIR, clean_folder, find_dir, hash_file, nfc,
    parse_key, sha256_hex, slot,
};
use state::{Entry, SyncState};

use crate::cloud::crypto::{self, Payload, VaultKey};
use crate::error::PennelError;
use crate::meetings::MeetingStore;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SyncStatus {
    Off,
    SignedOut,
    NeedsSetup,
    /// The server has a vault key and this device has none.
    NeedsRecoveryCode,
    /// The vault was restarted on another device; this device forgot its key.
    VaultReset,
    Synced,
    Offline,
    StorageFull {
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SyncReport {
    pub status: SyncStatus,
    pub uploaded: u32,
    pub downloaded: u32,
    pub conflicts: u32,
    pub last_success_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SyncItemState {
    Synced,
    Pending,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MeetingSyncState {
    pub id: String,
    pub state: SyncItemState,
}

pub struct PassInput<'a> {
    pub api: &'a dyn SyncApi,
    pub state: &'a SyncState,
    pub key: &'a VaultKey,
    pub root: &'a Path,
    pub data_dir: &'a Path,
    pub device: &'a str,
    pub recording: &'a [String],
}

const PAGE: u32 = 500;
const TRASH_DAYS: u32 = 30;
/// The server's per-object limit (5 MiB), checked on the sealed blob before uploading.
const MAX_OBJECT_BYTES: usize = 5_242_880;
const MISSING_ROOT: &str = "De meetingmap is niet gevonden.";
const TOO_LARGE: &str = "Dit bestand is groter dan 5 MB.";
const UNKNOWN_DEVICE: &str = "ander toestel";

struct Stop(SyncStatus);

impl From<PennelError> for Stop {
    fn from(error: PennelError) -> Self {
        Stop(status_for(error))
    }
}

pub(crate) fn status_for(error: PennelError) -> SyncStatus {
    match error {
        PennelError::Network { .. } => SyncStatus::Offline,
        PennelError::NotSignedIn => SyncStatus::SignedOut,
        PennelError::VaultLocked => SyncStatus::NeedsRecoveryCode,
        PennelError::Cloud { code, message } if code == "storage_full" => {
            SyncStatus::StorageFull { message }
        }
        PennelError::Cloud { code, .. } if code == "generation" => SyncStatus::VaultReset,
        other => SyncStatus::Error {
            message: other.to_string(),
        },
    }
}

#[derive(Default)]
struct Counts {
    uploaded: u32,
    downloaded: u32,
    conflicts: u32,
}

struct Pass<'a> {
    api: &'a dyn SyncApi,
    state: &'a SyncState,
    key: &'a VaultKey,
    generation: u32,
    root: PathBuf,
    data_dir: &'a Path,
    device: &'a str,
    recording: &'a [String],
    store: MeetingStore,
    /// uid → folder name on disk, including meetings that are recording.
    folders: HashMap<String, String>,
    counts: Counts,
    /// A problem that did not stop the pass, reported once it finishes.
    problem: Option<String>,
}

pub fn run(input: PassInput<'_>) -> SyncReport {
    let mut pass = Pass {
        api: input.api,
        state: input.state,
        key: input.key,
        generation: 0,
        root: input.root.to_path_buf(),
        data_dir: input.data_dir,
        device: input.device,
        recording: input.recording,
        store: MeetingStore::new(input.root),
        folders: HashMap::new(),
        counts: Counts::default(),
        problem: None,
    };
    let status = match pass.execute() {
        Ok(()) => pass
            .problem
            .take()
            .map_or(SyncStatus::Synced, |message| SyncStatus::Error { message }),
        Err(Stop(status)) => status,
    };
    SyncReport {
        status,
        uploaded: pass.counts.uploaded,
        downloaded: pass.counts.downloaded,
        conflicts: pass.counts.conflicts,
        last_success_ms: None,
    }
}

fn conflict_name(name: &str, payload: &Payload) -> (String, String) {
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    let device = clean_folder(&payload.device).unwrap_or_else(|| UNKNOWN_DEVICE.to_string());
    let when = Local
        .timestamp_millis_opt(payload.modified_ms)
        .single()
        .unwrap_or_else(Local::now)
        .format("%Y-%m-%d %H%M");
    (
        format!("{stem} (conflict {device} {when})"),
        ext.to_string(),
    )
}

fn modified_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

fn temp_path(dir: &Path) -> PathBuf {
    dir.join(format!(".pennel-sync-{}.tmp", uuid::Uuid::now_v7()))
}

/// Replaces `target` only if it still has the content the decision was based on (`expected`,
/// `None` = absent). Returns false, leaving `target` alone, when it changed in the meantime.
fn write_checked(
    dir: &Path,
    target: &Path,
    content: &[u8],
    expected: Option<&str>,
) -> Result<bool, PennelError> {
    let tmp = temp_path(dir);
    std::fs::write(&tmp, content).map_err(PennelError::storage)?;
    if hash_file(target).as_deref() != expected || (expected.is_none() && target.exists()) {
        let _ = std::fs::remove_file(&tmp);
        return Ok(false);
    }
    std::fs::rename(&tmp, target).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        PennelError::storage(error)
    })?;
    Ok(true)
}

impl Pass<'_> {
    fn execute(&mut self) -> Result<(), Stop> {
        if !self.root.is_dir() {
            return Err(Stop(SyncStatus::Error {
                message: MISSING_ROOT.into(),
            }));
        }
        if let Ok(canonical) = self.root.canonicalize() {
            self.root = canonical;
            self.store = MeetingStore::new(&self.root);
        }
        trash::purge(self.data_dir, TRASH_DAYS);
        let root_id = self.root.to_string_lossy().into_owned();
        match self.state.root()? {
            Some(previous) if previous == root_id => {}
            // Another meetings folder: compare everything again instead of treating the
            // old folder's files as deleted.
            Some(_) => {
                self.state.forget_progress()?;
                self.state.set_root(&root_id)?;
            }
            None => self.state.set_root(&root_id)?,
        }
        let vault = self.api.vault()?;
        match self.state.generation()? {
            None => self.state.set_generation(vault.generation)?,
            Some(local) if local < vault.generation => return Err(Stop(SyncStatus::VaultReset)),
            Some(local) if local > vault.generation => {
                return Err(Stop(status_for(crypto::vault_invalid())));
            }
            Some(_) => {}
        }
        if vault.wrapped.is_none() {
            return Err(Stop(SyncStatus::NeedsSetup));
        }
        self.generation = vault.generation;
        self.folders = self.resolve_folders()?;
        self.pull()?;
        self.assign_uids()?;
        self.push()
    }

    fn is_recording(&self, folder: &str) -> bool {
        let folder = nfc(folder);
        self.recording.iter().any(|id| nfc(id) == folder)
    }

    /// One folder per uid. A copied folder shares its uid with the original; the one this device
    /// synced before keeps it.
    fn resolve_folders(&self) -> Result<HashMap<String, String>, PennelError> {
        let mut by_uid: HashMap<String, Vec<String>> = HashMap::new();
        for folder in scan::meeting_folders(&self.root) {
            if let Some(uid) = folder.uid {
                by_uid.entry(uid).or_default().push(folder.name);
            }
        }
        let mut owners = HashMap::new();
        for (uid, names) in by_uid {
            let known = self.state.folder_of_uid(&uid)?;
            let owner = names
                .iter()
                .find(|name| known.as_deref() == Some(nfc(name).as_str()))
                .unwrap_or(&names[0])
                .clone();
            owners.insert(uid, owner);
        }
        Ok(owners)
    }

    fn assign_uids(&mut self) -> Result<(), Stop> {
        let mut owners = self.resolve_folders()?;
        for folder in scan::meeting_folders(&self.root) {
            if self.is_recording(&folder.name) {
                continue;
            }
            let owned = folder
                .uid
                .as_ref()
                .is_some_and(|uid| owners.get(uid) == Some(&folder.name));
            if owned {
                continue;
            }
            let known = self
                .state
                .uid_for_folder(&nfc(&folder.name))?
                .filter(|uid| !owners.contains_key(uid));
            let assigned = match (known, folder.uid.is_none()) {
                (None, true) => self.store.ensure_uid(&folder.name).ok(),
                (known, _) => {
                    let uid = known.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
                    self.store.set_uid(&folder.name, &uid).ok().map(|()| uid)
                }
            };
            if let Some(uid) = assigned {
                owners.insert(uid, folder.name);
            }
        }
        for (uid, folder) in &owners {
            self.state.set_meeting_folder(uid, &nfc(folder))?;
        }
        self.folders = owners;
        Ok(())
    }

    fn note_problem(&mut self, message: String) {
        self.problem.get_or_insert(message);
    }

    fn pull(&mut self) -> Result<(), Stop> {
        for id in self.state.deferred()? {
            self.state.undefer(&id)?;
            let entry = self.state.entry_by_object(&id)?;
            match self.api.get(&id)? {
                Some((version, blob)) => {
                    let before = entry.as_ref().map(|e| self.local_hash(&e.key));
                    self.apply_remote(&id, version, &blob, entry, before)?;
                }
                None => {
                    if let Some(entry) = entry {
                        let version = entry.server_version;
                        self.apply_tombstones(vec![(entry, version)])?;
                    }
                }
            }
        }
        let mut cursor = self.state.cursor()?;
        let mut full: Option<HashSet<String>> = None;
        loop {
            let (changes, next, more, generation) = match self.api.changes(cursor, PAGE)? {
                ChangePage::CursorExpired if full.is_none() => {
                    full = Some(HashSet::new());
                    cursor = 0;
                    continue;
                }
                ChangePage::CursorExpired => {
                    return Err(Stop(status_for(PennelError::InvalidResponse {
                        message: "cursor expired at 0".into(),
                    })));
                }
                ChangePage::Ok {
                    changes,
                    cursor,
                    more,
                    generation,
                } => (changes, cursor, more, generation),
            };
            if generation != self.generation {
                return Err(Stop(SyncStatus::VaultReset));
            }
            let mut tombstones = Vec::new();
            for change in changes {
                if let Some(seen) = full.as_mut() {
                    seen.insert(change.id.clone());
                }
                let entry = self.state.entry_by_object(&change.id)?;
                if entry
                    .as_ref()
                    .is_some_and(|e| e.server_version >= change.version)
                {
                    continue;
                }
                if change.deleted {
                    if let Some(entry) = entry {
                        tombstones.push((entry, change.version));
                    }
                    continue;
                }
                // Hashed before the download, so an edit made while it runs is noticed.
                let before = entry.as_ref().map(|e| self.local_hash(&e.key));
                if let Some((version, blob)) = self.api.get(&change.id)? {
                    self.apply_remote(&change.id, version, &blob, entry, before)?;
                }
            }
            self.apply_tombstones(tombstones)?;
            self.state.set_cursor(next)?;
            cursor = next;
            if !more {
                break;
            }
        }
        // After a full listing, objects missing from it were purged from the server long ago;
        // forgetting them makes the local files upload again instead of being deleted.
        if let Some(seen) = full {
            for entry in self.state.entries()? {
                if !seen.contains(&entry.object_id) {
                    self.state.remove(&entry.key)?;
                }
            }
        }
        Ok(())
    }

    /// The directory a key lives in and its file name; `None` for a meeting not on this device.
    fn location(&self, key: &str) -> Option<(PathBuf, Option<String>, String)> {
        match parse_key(key)? {
            Key::Template { name } => Some((self.root.join(TEMPLATES_DIR), None, name)),
            Key::Meeting { uid, name } => {
                let folder = self.folders.get(&uid)?.clone();
                Some((self.root.join(&folder), Some(folder), name))
            }
        }
    }

    fn local_hash(&self, key: &str) -> Option<String> {
        let (dir, _, name) = self.location(key)?;
        match slot(&dir, &name) {
            Slot::Existing(path) => hash_file(&path),
            _ => None,
        }
    }

    /// The folder for a meeting uid, adopting a same-named folder without uid (the same
    /// meeting from before sync) or creating one.
    fn folder_for(&mut self, uid: &str, wanted: Option<&str>) -> Result<String, Stop> {
        if let Some(folder) = self.folders.get(uid) {
            return Ok(folder.clone());
        }
        let wanted = wanted
            .and_then(clean_folder)
            .unwrap_or_else(|| uid.to_string());
        let mut candidate = wanted.clone();
        for n in 2.. {
            match find_dir(&self.root, &candidate) {
                None => {
                    std::fs::create_dir_all(self.root.join(&candidate))
                        .map_err(PennelError::storage)?;
                    self.folders.insert(uid.to_string(), candidate.clone());
                    return Ok(candidate);
                }
                Some(existing) => {
                    let owned = self.folders.values().any(|f| *f == existing)
                        || self.store.uid_of(&existing).is_some();
                    if !owned {
                        if !self.is_recording(&existing)
                            && self.root.join(&existing).join(MEETING_FILE).is_file()
                        {
                            let _ = self.store.set_uid(&existing, uid);
                        }
                        self.folders.insert(uid.to_string(), existing.clone());
                        return Ok(existing);
                    }
                }
            }
            candidate = format!("{wanted} ({n})");
        }
        unreachable!("unbounded range")
    }

    fn rename_folder(&mut self, uid: &str, current: &str, wanted: &str) -> Option<String> {
        if self.is_recording(current) {
            return None;
        }
        let mut candidate = wanted.to_string();
        for n in 2.. {
            match find_dir(&self.root, &candidate) {
                Some(existing) if existing != current => candidate = format!("{wanted} ({n})"),
                _ => break,
            }
        }
        if candidate == current {
            return None;
        }
        std::fs::rename(self.root.join(current), self.root.join(&candidate)).ok()?;
        self.folders.insert(uid.to_string(), candidate.clone());
        let _ = self.state.set_meeting_folder(uid, &nfc(&candidate));
        Some(candidate)
    }

    /// Writes the remote version beside `name`; an identical copy already there is reused.
    fn conflict_copy(&mut self, dir: &Path, name: &str, payload: &Payload) -> Result<(), Stop> {
        let (base, ext) = conflict_name(name, payload);
        let content_hash = sha256_hex(&payload.content);
        for n in 1.. {
            let candidate = if n == 1 {
                format!("{base}{ext}")
            } else {
                format!("{base} ({n}){ext}")
            };
            match slot(dir, &candidate) {
                Slot::Existing(path) if hash_file(&path).as_deref() == Some(&content_hash) => {
                    return Ok(());
                }
                Slot::Free(path) if write_checked(dir, &path, &payload.content, None)? => {
                    self.counts.conflicts += 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        unreachable!("unbounded range")
    }

    /// `before`: for a known object, the local hash taken before the download started.
    fn apply_remote(
        &mut self,
        id: &str,
        version: i64,
        blob: &[u8],
        entry: Option<Entry>,
        before: Option<Option<String>>,
    ) -> Result<(), Stop> {
        let payload = match crypto::open(self.key, id, self.generation, blob) {
            Ok(payload) => payload,
            Err(error) => {
                self.note_problem(error.to_string());
                return Ok(());
            }
        };
        let Some(parsed) = parse_key(&payload.key) else {
            self.note_problem(crypto::object_unreadable().to_string());
            return Ok(());
        };
        let (mut dir, folder, name, uid) = match parsed {
            Key::Template { name } => {
                let dir = self.root.join(TEMPLATES_DIR);
                std::fs::create_dir_all(&dir).map_err(PennelError::storage)?;
                (dir, None, name, None)
            }
            Key::Meeting { uid, name } => {
                let folder = self.folder_for(&uid, payload.folder.as_deref())?;
                if self.is_recording(&folder) {
                    self.state.defer(id)?;
                    return Ok(());
                }
                (self.root.join(&folder), Some(folder), name, Some(uid))
            }
        };
        let target = match slot(&dir, &name) {
            Slot::Taken => {
                // Writing would replace a different file on a case-insensitive disk.
                return self.conflict_copy(&dir, &name, &payload);
            }
            Slot::Existing(path) | Slot::Free(path) => path,
        };
        let local = before.unwrap_or_else(|| hash_file(&target));
        let remote_hash = sha256_hex(&payload.content);
        let base_hash = entry
            .as_ref()
            .map(|e| e.synced_hash.clone())
            .unwrap_or_default();
        let record = |hash: String, folder: Option<String>| Entry {
            key: payload.key.clone(),
            object_id: id.to_string(),
            server_version: version,
            synced_hash: hash,
            folder: folder.map(|f| nfc(&f)),
        };
        if local.as_deref() == Some(remote_hash.as_str()) {
            self.state.put(&record(remote_hash, folder))?;
            return Ok(());
        }
        let unchanged_here = local.is_none()
            || entry
                .as_ref()
                .is_some_and(|e| local.as_deref() == Some(e.synced_hash.as_str()));
        if !unchanged_here {
            self.conflict_copy(&dir, &name, &payload)?;
            self.state.put(&record(base_hash, folder))?;
            return Ok(());
        }
        let mut folder = folder;
        let mut target = target;
        if name == MEETING_FILE
            && let (Some(uid), Some(current)) = (uid.as_deref(), folder.clone())
            && let Some(wanted) = payload.folder.as_deref().and_then(clean_folder)
            && nfc(&current) != wanted
            && let Some(renamed) = self.rename_folder(uid, &current, &wanted)
        {
            dir = self.root.join(&renamed);
            target = dir.join(target.file_name().expect("file path"));
            folder = Some(renamed);
        }
        if write_checked(&dir, &target, &payload.content, local.as_deref())? {
            self.counts.downloaded += 1;
            self.state.put(&record(remote_hash, folder))?;
        } else {
            self.conflict_copy(&dir, &name, &payload)?;
            self.state.put(&record(base_hash, folder))?;
        }
        Ok(())
    }

    /// True when deleting this meeting would lose something: a file never synced or changed since.
    fn meeting_has_local_work(&self, uid: &str, dir: &Path) -> Result<bool, PennelError> {
        for (name, path) in scan::syncable_files(dir) {
            if name == MEETING_FILE {
                continue;
            }
            let entry = self.state.entry(&scan::meeting_key(uid, &name))?;
            if entry.is_none_or(|e| hash_file(&path).as_deref() != Some(e.synced_hash.as_str())) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn apply_tombstones(&mut self, tombstones: Vec<(Entry, i64)>) -> Result<(), Stop> {
        let (meeting_files, others): (Vec<_>, Vec<_>) = tombstones.into_iter().partition(|(e, _)| {
            matches!(parse_key(&e.key), Some(Key::Meeting { name, .. }) if name == MEETING_FILE)
        });
        for (entry, version) in others.into_iter().chain(meeting_files) {
            let Some((dir, folder, name)) = self.location(&entry.key) else {
                self.state.remove(&entry.key)?;
                continue;
            };
            if folder.as_deref().is_some_and(|f| self.is_recording(f)) {
                self.state.defer(&entry.object_id)?;
                continue;
            }
            let uid = match parse_key(&entry.key) {
                Some(Key::Meeting { uid, .. }) => Some(uid),
                _ => None,
            };
            // Edits made here to other files of a deleted meeting keep the meeting alive.
            if name == MEETING_FILE
                && let Some(uid) = uid.as_deref()
                && self.meeting_has_local_work(uid, &dir)?
            {
                self.state.put(&Entry {
                    server_version: version,
                    synced_hash: String::new(),
                    ..entry
                })?;
                continue;
            }
            match slot(&dir, &name) {
                Slot::Existing(path) if hash_file(&path).as_deref() == Some(&entry.synced_hash) => {
                    let rel = path.strip_prefix(&self.root).unwrap_or(&path).to_path_buf();
                    trash::move_to_trash(self.data_dir, &self.root, &rel)?;
                    self.state.remove(&entry.key)?;
                }
                Slot::Existing(_) => {
                    self.state.put(&Entry {
                        server_version: version,
                        ..entry
                    })?;
                }
                _ => self.state.remove(&entry.key)?,
            }
            if let (Some(uid), Some(folder)) = (uid, folder) {
                self.clean_up_folder(&uid, &folder)?;
            }
        }
        Ok(())
    }

    /// A meeting folder left with nothing sync carries (only audio, say) follows its meeting
    /// into the trash.
    fn clean_up_folder(&mut self, uid: &str, folder: &str) -> Result<(), PennelError> {
        let dir = self.root.join(folder);
        if !dir.is_dir() || !scan::syncable_files(&dir).is_empty() {
            return Ok(());
        }
        let empty = std::fs::read_dir(&dir)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false);
        if empty {
            std::fs::remove_dir(&dir).map_err(PennelError::storage)?;
        } else {
            trash::move_to_trash(self.data_dir, &self.root, Path::new(folder))?;
        }
        self.folders.remove(uid);
        Ok(())
    }

    fn push(&mut self) -> Result<(), Stop> {
        let live: HashMap<String, String> = self
            .folders
            .iter()
            .filter(|(_, folder)| !self.is_recording(folder))
            .map(|(uid, folder)| (uid.clone(), folder.clone()))
            .collect();
        let files = scan::scan(&self.root, &live)?;
        let keys: HashSet<String> = files.iter().map(|f| f.key.clone()).collect();
        for file in &files {
            let entry = self.state.entry(&file.key)?;
            if entry.as_ref().is_some_and(|e| e.synced_hash == file.hash) {
                continue;
            }
            self.push_file(file, entry)?;
        }
        for entry in self.state.entries()? {
            if !keys.contains(&entry.key) && self.vanished(&entry) {
                self.push_delete(entry)?;
            }
        }
        Ok(())
    }

    fn push_file(&mut self, file: &LocalFile, entry: Option<Entry>) -> Result<(), Stop> {
        let path = self.root.join(&file.rel_path);
        let Ok(content) = std::fs::read(&path) else {
            return Ok(());
        };
        let hash = sha256_hex(&content);
        if entry.as_ref().is_some_and(|e| e.synced_hash == hash) {
            return Ok(());
        }
        let id = crypto::object_id(self.key, &file.key);
        let payload = Payload {
            key: file.key.clone(),
            folder: file.folder.clone(),
            content,
            modified_ms: modified_ms(&path),
            device: self.device.to_string(),
        };
        let blob = crypto::seal(self.key, &id, self.generation, &payload);
        if blob.len() > MAX_OBJECT_BYTES {
            self.note_problem(TOO_LARGE.into());
            return Ok(());
        }
        let dir = path.parent().unwrap_or(&self.root).to_path_buf();
        let name = path
            .file_name()
            .map(|n| nfc(&n.to_string_lossy()))
            .unwrap_or_default();
        let mut base = entry.as_ref().map_or(0, |e| e.server_version);
        // A conflict is resolved and retried once; anything left over waits for the next pass.
        for _ in 0..3 {
            let result = match self.api.put(&id, base, self.generation, &blob) {
                Err(PennelError::Cloud { code, message }) if code == "object_too_large" => {
                    self.note_problem(message);
                    return Ok(());
                }
                other => other?,
            };
            match result {
                PutResult::Ok(version) => {
                    self.state.put(&Entry {
                        key: file.key.clone(),
                        object_id: id,
                        server_version: version,
                        synced_hash: hash,
                        folder: file.folder.clone(),
                    })?;
                    self.counts.uploaded += 1;
                    return Ok(());
                }
                PutResult::GenerationChanged => return Err(Stop(SyncStatus::VaultReset)),
                PutResult::Conflict {
                    current,
                    deleted: true,
                } => base = current,
                PutResult::Conflict { current, .. } => {
                    let Some((version, remote)) = self.api.get(&id)? else {
                        base = current;
                        continue;
                    };
                    let remote = match crypto::open(self.key, &id, self.generation, &remote) {
                        Ok(remote) => remote,
                        Err(error) => {
                            self.note_problem(error.to_string());
                            return Ok(());
                        }
                    };
                    if sha256_hex(&remote.content) == hash {
                        self.state.put(&Entry {
                            key: file.key.clone(),
                            object_id: id,
                            server_version: version,
                            synced_hash: hash,
                            folder: file.folder.clone(),
                        })?;
                        return Ok(());
                    }
                    self.conflict_copy(&dir, &name, &remote)?;
                    base = version;
                }
            }
        }
        Ok(())
    }

    /// Only a file whose meeting is still here, or a meeting folder that is gone entirely,
    /// counts as deleted. An unreadable `meeting.md` or a recording meeting never does.
    fn vanished(&self, entry: &Entry) -> bool {
        match parse_key(&entry.key) {
            Some(Key::Template { name }) => !matches!(
                slot(&self.root.join(TEMPLATES_DIR), &name),
                Slot::Existing(_)
            ),
            Some(Key::Meeting { uid, name }) => match self.folders.get(&uid) {
                Some(folder) => {
                    !self.is_recording(folder)
                        && !matches!(slot(&self.root.join(folder), &name), Slot::Existing(_))
                }
                None => entry.folder.as_deref().is_some_and(|folder| {
                    !self.is_recording(folder) && find_dir(&self.root, folder).is_none()
                }),
            },
            None => false,
        }
    }

    fn push_delete(&mut self, entry: Entry) -> Result<(), Stop> {
        match self
            .api
            .delete(&entry.object_id, entry.server_version, self.generation)?
        {
            PutResult::Ok(_) | PutResult::Conflict { deleted: true, .. } => {
                self.state.remove(&entry.key)?;
            }
            PutResult::GenerationChanged => return Err(Stop(SyncStatus::VaultReset)),
            // Changed on another device after this one deleted it: bring that version back.
            PutResult::Conflict { .. } => {
                self.state.remove(&entry.key)?;
                if let Some((version, blob)) = self.api.get(&entry.object_id)? {
                    self.apply_remote(&entry.object_id, version, &blob, None, None)?;
                }
            }
        }
        Ok(())
    }
}

fn provider_of(segment: &str) -> Option<&'static str> {
    if segment == "Dropbox" || segment.starts_with("Dropbox-") || segment.starts_with("Dropbox (") {
        Some("Dropbox")
    } else if segment.starts_with("OneDrive") {
        Some("OneDrive")
    } else if segment == "iCloudDrive" || segment == "Mobile Documents" {
        Some("iCloud")
    } else if segment == "Google Drive" || segment.starts_with("GoogleDrive") {
        Some("Google Drive")
    } else {
        None
    }
}

/// Warns when the meetings folder already lives in a cloud-synced folder.
pub fn folder_warning(root: &Path, home: &Path) -> Option<String> {
    let segments: Vec<String> = root
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let in_library = |sub: &str| root.starts_with(home.join("Library").join(sub));
    let name = if in_library("Mobile Documents") {
        Some("iCloud")
    } else if in_library("CloudStorage") {
        let provider = root
            .strip_prefix(home.join("Library").join("CloudStorage"))
            .ok()
            .and_then(|rest| rest.components().next())
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(provider_of(&provider).unwrap_or("iCloud"))
    } else {
        segments.iter().find_map(|segment| provider_of(segment))
    };
    name.map(|name| {
        format!("Deze map synct al via {name}. Dubbel synchroniseren kan conflicten geven.")
    })
}

#[cfg(test)]
mod tests {
    use super::api::fake::FakeApi;
    use super::*;
    use crate::cloud::crypto;
    use crate::meetings::MeetingStore;
    use chrono::{Local, TimeZone};
    use std::path::PathBuf;

    struct Device {
        _data: tempfile::TempDir,
        data_dir: PathBuf,
        root: PathBuf,
        state: SyncState,
        key: String,
        name: String,
    }

    fn shared_key() -> String {
        VaultKey::generate().to_b64()
    }

    impl Device {
        fn new(name: &str, key: &str) -> Self {
            let data = tempfile::tempdir().unwrap();
            let data_dir = data.path().join("data");
            let root = data.path().join("Meetings");
            std::fs::create_dir_all(&data_dir).unwrap();
            std::fs::create_dir_all(&root).unwrap();
            let state = SyncState::open(&data_dir).unwrap();
            state.set_generation(1).unwrap();
            Self {
                _data: data,
                data_dir,
                root,
                state,
                key: key.to_string(),
                name: name.to_string(),
            }
        }

        fn sync(&self, api: &FakeApi) -> SyncReport {
            self.sync_recording(api, &[])
        }

        fn sync_recording(&self, api: &FakeApi, recording: &[String]) -> SyncReport {
            let key = VaultKey::from_b64(&self.key).unwrap();
            run(PassInput {
                api,
                state: &self.state,
                key: &key,
                root: &self.root,
                data_dir: &self.data_dir,
                device: &self.name,
                recording,
            })
        }

        fn store(&self) -> MeetingStore {
            MeetingStore::new(&self.root)
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }

        fn read(&self, rel: &str) -> Option<String> {
            std::fs::read_to_string(self.path(rel)).ok()
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.path(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn names_in(&self, folder: &str) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.path(folder))
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            names
        }

        fn folders(&self) -> Vec<String> {
            self.names_in("")
                .into_iter()
                .filter(|n| !n.starts_with('.'))
                .collect()
        }

        fn conflict_copies(&self, folder: &str) -> Vec<String> {
            self.names_in(folder)
                .into_iter()
                .filter(|n| n.contains(" (conflict "))
                .collect()
        }

        fn trash_files(&self) -> Vec<String> {
            fn walk(dir: &Path, out: &mut Vec<String>) {
                for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    if entry.path().is_dir() {
                        walk(&entry.path(), out);
                    } else {
                        out.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
            let mut out = Vec::new();
            walk(&self.data_dir.join(trash::TRASH_DIR), &mut out);
            out.sort();
            out
        }
    }

    fn setup() -> (FakeApi, Device, Device) {
        let api = FakeApi::new();
        let key = shared_key();
        let wrapped_key = VaultKey::from_b64(&key).unwrap();
        let code = crypto::new_recovery_code();
        let (wrapped, kdf) = crypto::wrap(&wrapped_key, &code).unwrap();
        api.put_vault(1, &wrapped, &kdf, true).unwrap();
        (api, Device::new("Mac", &key), Device::new("Laptop", &key))
    }

    fn start() -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 25, 14, 0, 0).unwrap()
    }

    fn synced(report: &SyncReport) {
        assert_eq!(report.status, SyncStatus::Synced, "{report:?}");
    }

    /// A meeting with notes, transcript and audio, synced from `a` to `b`.
    fn shared_meeting(api: &FakeApi, a: &Device, b: &Device) -> String {
        let id = a.store().create("Klant", start()).unwrap();
        a.write(&format!("{id}/notes.md"), "notities\n");
        a.write(&format!("{id}/transcript.md"), "[00:01] **Ik:** Hallo\n");
        a.write(&format!("{id}/ik.m4a"), "audio");
        synced(&a.sync(api));
        synced(&b.sync(api));
        id
    }

    fn assert_conflict_name(name: &str, stem: &str, ext: &str) {
        let inner = name
            .strip_prefix(&format!("{stem} (conflict "))
            .and_then(|rest| rest.strip_suffix(&format!("){ext}")))
            .unwrap_or_else(|| panic!("unexpected conflict name {name}"));
        let (device, stamp) = inner.split_at(inner.len() - "2026-10-25 1400".len());
        assert!(!device.trim().is_empty(), "{name}");
        assert!(device.ends_with(' '), "{name}");
        assert!(
            chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H%M").is_ok(),
            "{name}"
        );
    }

    #[test]
    fn new_meeting_reaches_second_device() {
        let (api, a, b) = setup();
        let id = a.store().create("Klant", start()).unwrap();
        a.write(&format!("{id}/notes.md"), "notities\n");
        a.write(&format!("{id}/transcript.md"), "[00:01] **Ik:** Hallo\n");
        a.write(&format!("{id}/ik.m4a"), "audio");
        let report = a.sync(&api);
        synced(&report);
        assert_eq!(report.uploaded, 3);
        let report = b.sync(&api);
        synced(&report);
        assert_eq!(report.downloaded, 3);
        assert_eq!(
            b.names_in(&id),
            vec!["meeting.md", "notes.md", "transcript.md"]
        );
        for name in ["meeting.md", "notes.md", "transcript.md"] {
            assert_eq!(
                b.read(&format!("{id}/{name}")),
                a.read(&format!("{id}/{name}"))
            );
        }
        assert_eq!(b.store().uid_of(&id), a.store().uid_of(&id));
        let again = b.sync(&api);
        assert_eq!((again.uploaded, again.downloaded), (0, 0));
    }

    #[test]
    fn edit_roundtrips_back() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        b.write(&format!("{id}/notes.md"), "aangevuld op laptop\n");
        assert_eq!(b.sync(&api).uploaded, 1);
        let report = a.sync(&api);
        synced(&report);
        assert_eq!(report.downloaded, 1);
        assert_eq!(
            a.read(&format!("{id}/notes.md")).as_deref(),
            Some("aangevuld op laptop\n")
        );
        assert!(a.conflict_copies(&id).is_empty());
        assert!(b.conflict_copies(&id).is_empty());
    }

    #[test]
    fn concurrent_edit_creates_conflict_copy_and_keeps_local() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        a.write(&format!("{id}/notes.md"), "versie A\n");
        synced(&a.sync(&api));
        b.write(&format!("{id}/notes.md"), "versie B\n");
        let report = b.sync(&api);
        synced(&report);
        assert_eq!(report.conflicts, 1);
        assert_eq!(
            b.read(&format!("{id}/notes.md")).as_deref(),
            Some("versie B\n")
        );
        let copies = b.conflict_copies(&id);
        assert_eq!(copies.len(), 1, "{copies:?}");
        assert_conflict_name(&copies[0], "notes", ".md");
        assert!(copies[0].contains("(conflict Mac "), "{copies:?}");
        assert_eq!(
            b.read(&format!("{id}/{}", copies[0])).as_deref(),
            Some("versie A\n")
        );

        synced(&a.sync(&api));
        assert_eq!(
            a.read(&format!("{id}/notes.md")).as_deref(),
            Some("versie B\n")
        );
        assert_eq!(
            a.read(&format!("{id}/{}", copies[0])).as_deref(),
            Some("versie A\n")
        );
        let settle = b.sync(&api);
        assert_eq!(
            (settle.uploaded, settle.downloaded, settle.conflicts),
            (0, 0, 0)
        );
    }

    #[test]
    fn delete_moves_to_trash_on_other_device() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        b.write(&format!("{id}/ik.m4a"), "audio op laptop");
        a.store().delete(&id).unwrap();
        synced(&a.sync(&api));
        assert_eq!(api.live_objects(), 0);
        synced(&b.sync(&api));
        assert!(!b.path(&id).exists());
        assert_eq!(
            b.trash_files(),
            vec!["ik.m4a", "meeting.md", "notes.md", "transcript.md"]
        );
        let again = a.sync(&api);
        synced(&again);
        assert!(!a.path(&id).exists());
    }

    #[test]
    fn locally_modified_file_survives_remote_delete() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        a.store().delete(&id).unwrap();
        synced(&a.sync(&api));
        b.write(&format!("{id}/notes.md"), "nog bezig op laptop\n");
        synced(&b.sync(&api));
        assert_eq!(
            b.read(&format!("{id}/notes.md")).as_deref(),
            Some("nog bezig op laptop\n")
        );
        assert!(b.path(&format!("{id}/meeting.md")).exists());
        assert_eq!(b.trash_files(), vec!["transcript.md"]);
        synced(&a.sync(&api));
        assert_eq!(
            a.read(&format!("{id}/notes.md")).as_deref(),
            Some("nog bezig op laptop\n")
        );
        assert!(a.path(&format!("{id}/meeting.md")).exists());
    }

    #[test]
    fn rename_on_a_edit_on_b() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        let renamed = a.store().rename(&id, "Nieuwe naam").unwrap();
        assert_ne!(renamed, id);
        synced(&a.sync(&api));
        b.write(&format!("{id}/notes.md"), "edit op laptop\n");
        synced(&b.sync(&api));
        assert_eq!(b.folders(), vec![renamed.clone()]);
        assert_eq!(
            b.read(&format!("{renamed}/notes.md")).as_deref(),
            Some("edit op laptop\n")
        );
        assert_eq!(
            b.read(&format!("{renamed}/meeting.md")),
            a.read(&format!("{renamed}/meeting.md"))
        );
        synced(&a.sync(&api));
        assert_eq!(
            a.read(&format!("{renamed}/notes.md")).as_deref(),
            Some("edit op laptop\n")
        );
        assert!(a.conflict_copies(&renamed).is_empty());
        assert!(b.conflict_copies(&renamed).is_empty());
    }

    #[test]
    fn first_sync_matches_identical_files_without_conflicts() {
        let (api, a, b) = setup();
        let legacy = "2026-01-05 0900 Oud overleg";
        let meeting = "---\ntitle: Oud overleg\nstart: 2026-01-05T09:00:00+01:00\nduration_min: 30\ntemplate: \n---\n\nSamenvatting\n";
        let shared = a.store().create("Gedeeld", start()).unwrap();
        a.write(&format!("{shared}/notes.md"), "gedeeld\n");
        for device in [&a, &b] {
            device.write(&format!("{legacy}/meeting.md"), meeting);
            device.write(&format!("{legacy}/notes.md"), "oude notities\n");
            device.write(".templates/sales.md", "# Sales\n");
        }
        for name in ["meeting.md", "notes.md"] {
            b.write(
                &format!("{shared}/{name}"),
                &a.read(&format!("{shared}/{name}")).unwrap(),
            );
        }
        synced(&a.sync(&api));
        let report = b.sync(&api);
        synced(&report);
        assert_eq!(report.conflicts, 0);
        assert_eq!(report.uploaded, 0);
        let mut expected = vec![legacy.to_string(), shared.clone()];
        expected.sort();
        assert_eq!(b.folders(), expected);
        for folder in [legacy, shared.as_str()] {
            assert!(b.conflict_copies(folder).is_empty());
            for name in ["meeting.md", "notes.md"] {
                assert_eq!(
                    b.read(&format!("{folder}/{name}")),
                    a.read(&format!("{folder}/{name}"))
                );
            }
        }
        assert!(b.store().uid_of(legacy).is_some());
        let again = a.sync(&api);
        assert_eq!(
            (again.uploaded, again.downloaded, again.conflicts),
            (0, 0, 0)
        );
    }

    #[test]
    fn external_edit_during_pull_is_kept() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        a.write(&format!("{id}/notes.md"), "van de server\n");
        synced(&a.sync(&api));
        let target = b.path(&format!("{id}/notes.md"));
        let mut fired = false;
        api.on_get(move |_| {
            if !fired {
                fired = true;
                std::fs::write(&target, "getypt in Obsidian\n").unwrap();
            }
        });
        let report = b.sync(&api);
        synced(&report);
        assert_eq!(
            b.read(&format!("{id}/notes.md")).as_deref(),
            Some("getypt in Obsidian\n")
        );
        let copies = b.conflict_copies(&id);
        assert_eq!(copies.len(), 1, "{copies:?}");
        assert_eq!(
            b.read(&format!("{id}/{}", copies[0])).as_deref(),
            Some("van de server\n")
        );
    }

    #[test]
    fn cursor_expired_does_full_compare_without_deleting() {
        let (api, a, b) = setup();
        let first = shared_meeting(&api, &a, &b);
        let second = a.store().create("Tweede", start()).unwrap();
        a.write(&format!("{second}/notes.md"), "v1\n");
        synced(&a.sync(&api));
        synced(&b.sync(&api));
        a.store().delete(&first).unwrap();
        a.write(&format!("{second}/notes.md"), "v2\n");
        synced(&a.sync(&api));
        api.purge_tombstones();

        let report = b.sync(&api);
        synced(&report);
        assert_eq!(
            b.read(&format!("{second}/notes.md")).as_deref(),
            Some("v2\n")
        );
        for name in ["meeting.md", "notes.md", "transcript.md"] {
            assert!(b.path(&format!("{first}/{name}")).exists(), "{name}");
        }
        assert!(b.trash_files().is_empty());
        synced(&a.sync(&api));
        assert!(a.path(&format!("{first}/notes.md")).exists());
    }

    #[test]
    fn recording_meeting_is_skipped() {
        let (api, a, b) = setup();
        let recording = a.store().create("Opname", start()).unwrap();
        let other = a.store().create("Andere", start()).unwrap();
        a.write(&format!("{recording}/transcript.md"), "[00:01] **Ik:** a\n");
        synced(&a.sync_recording(&api, std::slice::from_ref(&recording)));
        synced(&b.sync(&api));
        assert_eq!(b.folders(), vec![other.clone()]);
        synced(&a.sync(&api));
        synced(&b.sync(&api));
        assert!(b.path(&format!("{recording}/transcript.md")).exists());

        a.write(&format!("{recording}/notes.md"), "na afloop\n");
        synced(&a.sync(&api));
        synced(&b.sync_recording(&api, std::slice::from_ref(&recording)));
        assert!(!b.path(&format!("{recording}/notes.md")).exists());
        synced(&b.sync(&api));
        assert_eq!(
            b.read(&format!("{recording}/notes.md")).as_deref(),
            Some("na afloop\n")
        );

        std::fs::remove_dir_all(b.path(&recording)).unwrap();
        b.store().create("Opname", start()).unwrap();
        synced(&b.sync_recording(&api, std::slice::from_ref(&recording)));
        assert_eq!(api.live_objects(), 4);
    }

    #[test]
    fn templates_sync() {
        let (api, a, b) = setup();
        a.write(".templates/sales.md", "# Sales\n");
        a.write(".templates/notes.txt", "geen sjabloon");
        synced(&a.sync(&api));
        synced(&b.sync(&api));
        assert_eq!(b.read(".templates/sales.md").as_deref(), Some("# Sales\n"));
        assert!(b.read(".templates/notes.txt").is_none());
        b.write(".templates/sales.md", "# Sales v2\n");
        synced(&b.sync(&api));
        synced(&a.sync(&api));
        assert_eq!(
            a.read(".templates/sales.md").as_deref(),
            Some("# Sales v2\n")
        );
        std::fs::remove_file(a.path(".templates/sales.md")).unwrap();
        synced(&a.sync(&api));
        synced(&b.sync(&api));
        assert!(b.read(".templates/sales.md").is_none());
        assert_eq!(b.trash_files(), vec!["sales.md"]);
    }

    #[test]
    fn vault_reset_on_other_device_reports_vault_reset() {
        let (api, a, b) = setup();
        shared_meeting(&api, &a, &b);
        api.reset_vault(1).unwrap();
        let report = b.sync(&api);
        assert_eq!(report.status, SyncStatus::VaultReset);
    }

    #[test]
    fn storage_full_reported_with_server_message() {
        let (api, a, _) = setup();
        api.set_storage_limit(Some(10));
        a.store().create("Groot", start()).unwrap();
        assert_eq!(
            a.sync(&api).status,
            SyncStatus::StorageFull {
                message: "Je cloudopslag is vol (1 GB). Lokaal blijft alles werken.".into()
            }
        );
    }

    #[test]
    fn network_error_reports_offline() {
        let (api, a, _) = setup();
        a.store().create("Offline", start()).unwrap();
        api.set_offline(true);
        assert_eq!(a.sync(&api).status, SyncStatus::Offline);
        api.set_offline(false);
        assert_eq!(a.sync(&api).uploaded, 1);
    }

    #[test]
    fn existing_meeting_gets_uid_once() {
        let (api, a, _) = setup();
        let original = "---\ntitle: Oud\nstart: 2026-01-01T10:00:00+01:00\n---\n\nInhoud\n";
        a.write("2026-01-01 1000 Oud/meeting.md", original);
        synced(&a.sync(&api));
        let uid = a
            .store()
            .uid_of("2026-01-01 1000 Oud")
            .expect("uid assigned");
        let text = a.read("2026-01-01 1000 Oud/meeting.md").unwrap();
        assert_eq!(
            text,
            format!(
                "---\ntitle: Oud\nstart: 2026-01-01T10:00:00+01:00\npennel_id: {uid}\n---\n\nInhoud\n"
            )
        );
        let again = a.sync(&api);
        assert_eq!(again.uploaded, 0);
        assert_eq!(a.read("2026-01-01 1000 Oud/meeting.md").unwrap(), text);
    }

    #[test]
    fn folder_warning_detects_cloud_paths() {
        let home = Path::new("/Users/els");
        let message = |name: &str| {
            Some(format!(
                "Deze map synct al via {name}. Dubbel synchroniseren kan conflicten geven."
            ))
        };
        let cases = [
            (
                "/Users/els/Library/Mobile Documents/com~apple~CloudDocs/Pennel",
                message("iCloud"),
            ),
            (
                "/Users/els/Library/CloudStorage/OneDrive-Persoonlijk/Pennel",
                message("OneDrive"),
            ),
            (
                "/Users/els/Library/CloudStorage/GoogleDrive-els@x.be/My Drive/Pennel",
                message("Google Drive"),
            ),
            (
                "/Users/els/Library/CloudStorage/Dropbox/Pennel",
                message("Dropbox"),
            ),
            ("/Users/els/Dropbox/Pennel", message("Dropbox")),
            ("C:/Users/els/OneDrive - AE/Pennel", message("OneDrive")),
            ("C:/Users/els/iCloudDrive/Pennel", message("iCloud")),
            ("G:/Google Drive/Pennel", message("Google Drive")),
            ("/Users/els/Pennel", None),
            ("/Users/els/Documents/DropboxBackup/Pennel", None),
        ];
        for (path, expected) in cases {
            assert_eq!(folder_warning(Path::new(path), home), expected, "{path}");
        }
    }

    #[test]
    fn missing_meetings_root_reports_error_and_deletes_nothing() {
        let (api, a, _) = setup();
        let id = a.store().create("Extern", start()).unwrap();
        a.write(&format!("{id}/notes.md"), "n\n");
        synced(&a.sync(&api));
        std::fs::remove_dir_all(&a.root).unwrap();
        assert!(matches!(a.sync(&api).status, SyncStatus::Error { .. }));
        assert_eq!(api.live_objects(), 2);
    }

    #[test]
    fn unreadable_meeting_md_deletes_nothing_remotely() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        a.write(&format!("{id}/meeting.md"), "kapotte frontmatter");
        synced(&a.sync(&api));
        assert_eq!(api.live_objects(), 3);
        assert!(b.path(&format!("{id}/notes.md")).exists());
    }

    #[test]
    fn case_variant_local_file_is_not_overwritten() {
        let (api, a, b) = setup();
        let id = shared_meeting(&api, &a, &b);
        a.write(&format!("{id}/Ideeen.md"), "van A\n");
        synced(&a.sync(&api));
        b.write(&format!("{id}/IDEEEN.md"), "van B\n");
        synced(&b.sync(&api));
        assert_eq!(
            b.read(&format!("{id}/IDEEEN.md")).as_deref(),
            Some("van B\n")
        );
        let copies = b.conflict_copies(&id);
        assert_eq!(copies.len(), 1, "{copies:?}");
        assert_conflict_name(&copies[0], "Ideeen", ".md");
        assert_eq!(
            b.read(&format!("{id}/{}", copies[0])).as_deref(),
            Some("van A\n")
        );
    }

    #[test]
    fn unreadable_object_is_skipped_and_reported() {
        let (api, a, b) = setup();
        let id = a.store().create("Leesbaar", start()).unwrap();
        a.write(&format!("{id}/notes.md"), "ok\n");
        synced(&a.sync(&api));
        let bogus = crypto::object_id(&VaultKey::generate(), "templates/x.md");
        assert!(matches!(
            api.put(&bogus, 0, 1, b"garbage").unwrap(),
            api::PutResult::Ok(_)
        ));
        let report = b.sync(&api);
        assert_eq!(
            report.status,
            SyncStatus::Error {
                message: "Een bestand uit Pennel Cloud kon niet worden ontsleuteld.".into()
            }
        );
        assert_eq!(b.read(&format!("{id}/notes.md")).as_deref(), Some("ok\n"));
    }

    #[test]
    fn copied_meeting_folder_gets_its_own_uid() {
        let (api, a, b) = setup();
        let id = a.store().create("Origineel", start()).unwrap();
        synced(&a.sync(&api));
        let copy = format!("{id} kopie");
        a.write(
            &format!("{copy}/meeting.md"),
            &a.read(&format!("{id}/meeting.md")).unwrap(),
        );
        synced(&a.sync(&api));
        assert_ne!(a.store().uid_of(&id), a.store().uid_of(&copy));
        synced(&b.sync(&api));
        assert_eq!(b.store().uid_of(&id), a.store().uid_of(&id));
        assert_eq!(b.store().uid_of(&copy), a.store().uid_of(&copy));
        let mut expected = vec![id.clone(), copy.clone()];
        expected.sort();
        assert_eq!(b.folders(), expected);
    }
}
