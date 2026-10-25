use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::PennelError;

const FORMAT_VERSION: u32 = 1;
const JOURNAL: &str = "migration.json";
const BACKUP: &str = "migration-backup-v1";

#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    version: u32,
    complete: bool,
    snapshot: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    files: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct DirectoryLease {
    _file: File,
}

impl DirectoryLease {
    pub(crate) fn acquire(dir: &Path) -> Result<Self, PennelError> {
        Self::acquire_named(dir, ".pennel.lock")
    }

    pub(crate) fn acquire_library(dir: &Path) -> Result<Self, PennelError> {
        Self::acquire_named(dir, ".pennel-library.lock")
    }

    fn acquire_named(dir: &Path, name: &str) -> Result<Self, PennelError> {
        fs::create_dir_all(dir).map_err(PennelError::storage)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join(name))
            .map_err(PennelError::storage)?;
        file.try_lock().map_err(|_| PennelError::Storage {
            message: "deze gegevensmap wordt al door een andere Pennel-app gebruikt".into(),
        })?;
        Ok(Self { _file: file })
    }
}

pub(crate) struct StartupStorage {
    _lease: DirectoryLease,
    dir: PathBuf,
    journal: Journal,
}

impl StartupStorage {
    /// Must run before opening stores, normalizing settings, or applying retention.
    pub(crate) fn prepare(dir: &Path) -> Result<Self, PennelError> {
        let lease = DirectoryLease::acquire(dir)?;
        let journal_path = dir.join(JOURNAL);
        let previous: Option<Journal> = match fs::read(&journal_path) {
            Ok(bytes) => Some(serde_json::from_slice(&bytes).map_err(PennelError::storage)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(PennelError::storage(error)),
        };
        if let Some(journal) = previous {
            if journal.version != FORMAT_VERSION {
                return Err(PennelError::Storage {
                    message: "deze Pennel-gegevens vereisen een andere appversie".into(),
                });
            }
            if !journal.complete && journal.snapshot {
                ensure_snapshot(dir)?;
            }
            return Ok(Self {
                _lease: lease,
                dir: dir.to_owned(),
                journal,
            });
        }
        let snapshot = [
            "settings.json",
            crate::dictation_pad::PAD_FILE,
            "history.sqlite",
            "index.sqlite",
            "failed-audio",
        ]
        .iter()
        .any(|name| dir.join(name).exists());
        let journal = Journal {
            version: FORMAT_VERSION,
            complete: false,
            snapshot,
        };
        write_json(&journal_path, &journal)?;
        if snapshot {
            ensure_snapshot(dir)?;
        }
        Ok(Self {
            _lease: lease,
            dir: dir.to_owned(),
            journal,
        })
    }

    pub(crate) fn finish(&mut self) -> Result<(), PennelError> {
        if !self.journal.complete {
            self.journal.complete = true;
            write_json(&self.dir.join(JOURNAL), &self.journal)?;
        }
        Ok(())
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), PennelError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(PennelError::storage)?;
    let temp = path.with_extension("json.tmp");
    let mut file = File::create(&temp).map_err(PennelError::storage)?;
    file.write_all(&bytes).map_err(PennelError::storage)?;
    file.sync_all().map_err(PennelError::storage)?;
    drop(file);
    fs::rename(temp, path).map_err(PennelError::storage)?;
    sync_directory(path.parent().expect("file parent"))
}

fn ensure_snapshot(dir: &Path) -> Result<(), PennelError> {
    let backup = dir.join(BACKUP);
    if backup.exists() {
        let manifest: Manifest = serde_json::from_slice(
            &fs::read(backup.join("manifest.json")).map_err(PennelError::storage)?,
        )
        .map_err(PennelError::storage)?;
        if manifest.version != FORMAT_VERSION
            || manifest.files.iter().any(|name| {
                let relative = Path::new(name);
                relative.is_absolute()
                    || relative
                        .components()
                        .any(|component| !matches!(component, std::path::Component::Normal(_)))
                    || !backup.join(relative).is_file()
            })
        {
            return Err(PennelError::Storage {
                message:
                    "de migratieback-up is onvolledig; herstel de gegevens voordat Pennel start"
                        .into(),
            });
        }
        return Ok(());
    }
    let staging = dir.join(format!("{BACKUP}.incomplete-{}", uuid::Uuid::now_v7()));
    fs::create_dir(&staging).map_err(PennelError::storage)?;
    private_directory(&staging)?;
    let mut files = Vec::new();
    for name in [
        "settings.json",
        crate::dictation_pad::PAD_FILE,
        "history.sqlite",
        "index.sqlite",
    ] {
        let source = dir.join(name);
        if !source.exists() {
            continue;
        }
        if source
            .symlink_metadata()
            .map_err(PennelError::storage)?
            .file_type()
            .is_symlink()
        {
            return Err(PennelError::Storage { message: "een gegevensbestand is een symbolische koppeling; maak eerst een gewone lokale kopie".into() });
        }
        let target = staging.join(name);
        if name.ends_with(".sqlite") {
            // VACUUM INTO reads a consistent SQLite snapshot, including committed WAL pages.
            let connection = Connection::open_with_flags(&source, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(PennelError::storage)?;
            connection
                .busy_timeout(std::time::Duration::from_secs(5))
                .map_err(PennelError::storage)?;
            connection
                .execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])
                .map_err(PennelError::storage)?;
        } else {
            fs::copy(&source, &target).map_err(PennelError::storage)?;
        }
        sync_file(&target)?;
        files.push(name.to_owned());
    }
    if dir.join("failed-audio").exists() {
        copy_tree(dir, &staging, Path::new("failed-audio"), &mut files)?;
    }
    write_json(
        &staging.join("manifest.json"),
        &Manifest {
            version: FORMAT_VERSION,
            files,
        },
    )?;
    sync_directory(&staging)?;
    fs::rename(staging, backup).map_err(PennelError::storage)?;
    sync_directory(dir)
}

fn copy_tree(
    root: &Path,
    target_root: &Path,
    relative: &Path,
    files: &mut Vec<String>,
) -> Result<(), PennelError> {
    let source = root.join(relative);
    let metadata = source.symlink_metadata().map_err(PennelError::storage)?;
    if metadata.file_type().is_symlink() {
        return Err(PennelError::Storage {
            message:
                "een audiobestand is een symbolische koppeling; maak eerst een gewone lokale kopie"
                    .into(),
        });
    }
    let target = target_root.join(relative);
    if metadata.is_dir() {
        fs::create_dir(&target).map_err(PennelError::storage)?;
        private_directory(&target)?;
        for entry in fs::read_dir(&source).map_err(PennelError::storage)? {
            let entry = entry.map_err(PennelError::storage)?;
            copy_tree(root, target_root, &relative.join(entry.file_name()), files)?;
        }
    } else if metadata.is_file() {
        fs::copy(source, &target).map_err(PennelError::storage)?;
        sync_file(&target)?;
        files.push(relative.to_string_lossy().into_owned());
    } else {
        return Err(PennelError::Storage {
            message: "onbekend bestandstype in de audiomap".into(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn private_directory(path: &Path) -> Result<(), PennelError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(PennelError::storage)
}

#[cfg(not(unix))]
fn private_directory(_path: &Path) -> Result<(), PennelError> {
    Ok(())
}

fn sync_file(path: &Path) -> Result<(), PennelError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.write(true);
    options
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(PennelError::storage)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), PennelError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(PennelError::storage)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), PennelError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_lease_excludes_another_owner_and_releases_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let first = DirectoryLease::acquire(dir.path()).unwrap();
        assert!(DirectoryLease::acquire(dir.path()).is_err());
        drop(first);
        assert!(DirectoryLease::acquire(dir.path()).is_ok());
    }

    #[test]
    fn snapshot_includes_wal_rows_and_failed_audio_before_pruning() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "original").unwrap();
        fs::write(
            dir.path().join(crate::dictation_pad::PAD_FILE),
            "Eigen concept",
        )
        .unwrap();
        fs::create_dir(dir.path().join("failed-audio")).unwrap();
        fs::write(dir.path().join("failed-audio/one.wav"), b"private audio").unwrap();
        let connection = Connection::open(dir.path().join("history.sqlite")).unwrap();
        connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE items (value TEXT); INSERT INTO items VALUES ('committed');").unwrap();
        let mut storage = StartupStorage::prepare(dir.path()).unwrap();
        let saved = Connection::open(dir.path().join(BACKUP).join("history.sqlite")).unwrap();
        assert_eq!(
            saved
                .query_row("SELECT value FROM items", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "committed"
        );
        assert_eq!(
            fs::read(dir.path().join(BACKUP).join("failed-audio/one.wav")).unwrap(),
            b"private audio"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(BACKUP).join(crate::dictation_pad::PAD_FILE))
                .unwrap(),
            "Eigen concept"
        );
        storage.finish().unwrap();
    }

    #[test]
    fn interrupted_migration_reuses_original_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "original").unwrap();
        drop(StartupStorage::prepare(dir.path()).unwrap());
        fs::write(dir.path().join("settings.json"), "repaired").unwrap();
        let mut storage = StartupStorage::prepare(dir.path()).unwrap();
        storage.finish().unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(BACKUP).join("settings.json")).unwrap(),
            "original"
        );
    }

    #[test]
    fn an_incomplete_snapshot_does_not_allow_startup() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "original").unwrap();
        drop(StartupStorage::prepare(dir.path()).unwrap());
        fs::remove_file(dir.path().join(BACKUP).join("settings.json")).unwrap();
        assert!(StartupStorage::prepare(dir.path()).is_err());
    }

    #[test]
    fn future_migration_versions_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        write_json(
            &dir.path().join(JOURNAL),
            &Journal {
                version: 99,
                complete: true,
                snapshot: false,
            },
        )
        .unwrap();
        assert!(StartupStorage::prepare(dir.path()).is_err());
        let journal: Journal =
            serde_json::from_slice(&fs::read(dir.path().join(JOURNAL)).unwrap()).unwrap();
        assert_eq!(journal.version, 99);
    }
}
