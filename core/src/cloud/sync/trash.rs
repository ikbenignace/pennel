use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};

use crate::error::PennelError;

pub const TRASH_DIR: &str = "sync-trash";

fn day_dir(data_dir: &Path) -> PathBuf {
    data_dir
        .join(TRASH_DIR)
        .join(Local::now().format("%Y-%m-%d").to_string())
}

fn free_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|candidate| !candidate.exists())
        .expect("unbounded iterator")
}

fn copy_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(drop)
    }
}

fn move_path(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.is_dir() && from.is_dir() {
        // Earlier files of the same folder were trashed today already: merge.
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            move_path(&entry.path(), &to.join(entry.file_name()))?;
        }
        return std::fs::remove_dir(from);
    }
    let to = free_path(to);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, &to).is_ok() {
        return Ok(());
    }
    // Different volumes: copy first, delete the original only once the copy is complete.
    copy_recursive(from, &to)?;
    if from.is_dir() {
        std::fs::remove_dir_all(from)
    } else {
        std::fs::remove_file(from)
    }
}

/// Moves a file or folder of the meetings library into `<data_dir>/sync-trash/<day>/`, keeping
/// its relative path.
pub fn move_to_trash(
    data_dir: &Path,
    meetings_root: &Path,
    rel_path: &Path,
) -> Result<(), PennelError> {
    let source = meetings_root.join(rel_path);
    if !source.exists() {
        return Ok(());
    }
    move_path(&source, &day_dir(data_dir).join(rel_path)).map_err(PennelError::storage)
}

pub fn purge(data_dir: &Path, older_than_days: u32) {
    let Ok(days) = std::fs::read_dir(data_dir.join(TRASH_DIR)) else {
        return;
    };
    let cutoff = Local::now().date_naive() - chrono::Days::new(u64::from(older_than_days));
    for day in days.filter_map(Result::ok) {
        let expired = day
            .file_name()
            .to_str()
            .and_then(|name| NaiveDate::parse_from_str(name, "%Y-%m-%d").ok())
            .is_some_and(|date| date < cutoff);
        if expired {
            let _ = std::fs::remove_dir_all(day.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_keeps_relative_path_and_merges_folders() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("M")).unwrap();
        std::fs::write(root.path().join("M/notes.md"), "n").unwrap();
        std::fs::write(root.path().join("M/ik.m4a"), "a").unwrap();
        move_to_trash(data.path(), root.path(), Path::new("M/notes.md")).unwrap();
        move_to_trash(data.path(), root.path(), Path::new("M")).unwrap();
        let day = day_dir(data.path());
        assert_eq!(
            std::fs::read_to_string(day.join("M/notes.md")).unwrap(),
            "n"
        );
        assert_eq!(std::fs::read_to_string(day.join("M/ik.m4a")).unwrap(), "a");
        assert!(!root.path().join("M").exists());
        std::fs::create_dir_all(root.path().join("M")).unwrap();
        std::fs::write(root.path().join("M/notes.md"), "n2").unwrap();
        move_to_trash(data.path(), root.path(), Path::new("M/notes.md")).unwrap();
        assert_eq!(
            std::fs::read_to_string(day.join("M/notes (2).md")).unwrap(),
            "n2"
        );
    }

    #[test]
    fn purge_removes_only_old_days() {
        let data = tempfile::tempdir().unwrap();
        let trash = data.path().join(TRASH_DIR);
        let old = (Local::now().date_naive() - chrono::Days::new(31))
            .format("%Y-%m-%d")
            .to_string();
        let recent = (Local::now().date_naive() - chrono::Days::new(29))
            .format("%Y-%m-%d")
            .to_string();
        for name in [&old, &recent, &"anders".to_string()] {
            std::fs::create_dir_all(trash.join(name)).unwrap();
        }
        purge(data.path(), 30);
        assert!(!trash.join(&old).exists());
        assert!(trash.join(&recent).exists());
        assert!(trash.join("anders").exists());
    }
}
