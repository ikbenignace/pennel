use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::PennelError;

pub(crate) const PAD_FILE: &str = "dictation-pad.txt";

pub(crate) struct DictationPad {
    path: PathBuf,
    text: String,
}

impl DictationPad {
    pub(crate) fn open(dir: &Path) -> Result<Self, PennelError> {
        let path = dir.join(PAD_FILE);
        if path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(PennelError::Storage {
                message: "the dictation pad is a symbolic link; use a regular local file".into(),
            });
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(PennelError::storage(error)),
        };
        Ok(Self { path, text })
    }

    pub(crate) fn text(&self) -> String {
        self.text.clone()
    }

    pub(crate) fn save(&mut self, text: String) -> Result<(), PennelError> {
        if self.text == text {
            return Ok(());
        }
        let temporary = self
            .path
            .with_file_name(format!(".dictation-pad-{}.tmp", uuid::Uuid::now_v7()));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary).map_err(PennelError::storage)?;
            file.write_all(text.as_bytes())
                .map_err(PennelError::storage)?;
            file.sync_all().map_err(PennelError::storage)?;
            drop(file);
            fs::rename(&temporary, &self.path).map_err(PennelError::storage)?;
            #[cfg(unix)]
            fs::File::open(self.path.parent().expect("pad directory"))
                .and_then(|directory| directory.sync_all())
                .map_err(PennelError::storage)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        self.text = text;
        Ok(())
    }

    pub(crate) fn append(&mut self, text: String) -> Result<String, PennelError> {
        if text.trim().is_empty() {
            return Ok(self.text());
        }
        let mut joined = self.text();
        if !joined.is_empty()
            && !joined.ends_with(char::is_whitespace)
            && !text.starts_with(char::is_whitespace)
        {
            joined.push(' ');
        }
        joined.push_str(&text);
        self.save(joined)?;
        Ok(self.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appending_preserves_manual_spacing_and_punctuation() {
        let dir = tempfile::tempdir().unwrap();
        let mut pad = DictationPad::open(dir.path()).unwrap();
        pad.save("Mijn eigen tekst.".into()).unwrap();
        assert_eq!(
            pad.append("Nog een zin.".into()).unwrap(),
            "Mijn eigen tekst. Nog een zin."
        );
        pad.save("Notities:\n".into()).unwrap();
        assert_eq!(
            pad.append("Nieuw punt.".into()).unwrap(),
            "Notities:\nNieuw punt."
        );
    }

    #[test]
    fn a_failed_write_does_not_replace_the_current_draft() {
        let dir = tempfile::tempdir().unwrap();
        let mut pad = DictationPad::open(dir.path()).unwrap();
        pad.save("Bewaard".into()).unwrap();
        fs::remove_file(dir.path().join(PAD_FILE)).unwrap();
        fs::create_dir(dir.path().join(PAD_FILE)).unwrap();
        assert!(pad.save("Niet bewaard".into()).is_err());
        assert_eq!(pad.text(), "Bewaard");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn external_symlinks_are_not_read_and_new_drafts_are_private() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        symlink(outside.path(), dir.path().join(PAD_FILE)).unwrap();
        assert!(DictationPad::open(dir.path()).is_err());
        fs::remove_file(dir.path().join(PAD_FILE)).unwrap();
        let mut pad = DictationPad::open(dir.path()).unwrap();
        pad.save("Privé".into()).unwrap();
        assert_eq!(
            fs::metadata(dir.path().join(PAD_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
