//! A plain-text log the user can open from the tray, like the Mac app's `pennel.log`.
//! Never log dictated or meeting text: lengths, timings and error messages only.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: u64 = 1_000_000;

pub struct LogFile {
    path: PathBuf,
    max_bytes: u64,
    lock: Mutex<()>,
}

impl LogFile {
    pub fn new(path: PathBuf, max_bytes: u64) -> Self {
        Self {
            path,
            max_bytes,
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Rotates to `<name>.1.log` when the file would grow past `max_bytes`.
    pub fn append(&self, category: &str, message: &str) {
        let line = format!(
            "{} [{category}] {}\n",
            chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
            message.replace(['\n', '\r'], " ")
        );
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let size = std::fs::metadata(&self.path).map_or(0, |m| m.len());
        if size + line.len() as u64 > self.max_bytes {
            let previous = self.path.with_extension("1.log");
            let _ = std::fs::remove_file(&previous);
            let _ = std::fs::rename(&self.path, &previous);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

static FILE: OnceLock<LogFile> = OnceLock::new();

pub fn init(path: PathBuf) {
    let _ = FILE.set(LogFile::new(path, MAX_BYTES));
}

pub fn file_path() -> Option<PathBuf> {
    FILE.get().map(|f| f.path().to_owned())
}

#[derive(Clone, Copy)]
pub struct Log(&'static str);

impl Log {
    pub const fn new(category: &'static str) -> Self {
        Self(category)
    }

    pub fn info(&self, message: impl AsRef<str>) {
        self.write(message.as_ref());
    }

    pub fn error(&self, message: impl AsRef<str>) {
        self.write(&error_line(message.as_ref()));
    }

    fn write(&self, message: &str) {
        if cfg!(debug_assertions) {
            eprintln!("[{}] {message}", self.0);
        }
        if let Some(file) = FILE.get() {
            file.append(self.0, message);
        }
    }
}

fn error_line(message: &str) -> String {
    format!("ERROR {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_severity_is_english_and_log_content_remains_metadata_only() {
        let dir = tempfile::tempdir().unwrap();
        let log = LogFile::new(dir.path().join("pennel.log"), MAX_BYTES);
        log.append("recording", &error_line("microphone stopped after 240 ms"));
        let text = std::fs::read_to_string(log.path()).unwrap();
        assert!(text.contains("[recording] ERROR microphone stopped after 240 ms"));
        assert!(!text.contains("FOUT"));
    }

    #[test]
    fn appends_lines_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let log = LogFile::new(dir.path().join("logs/pennel.log"), 200);
        log.append("app", "start");
        log.append("app", "two\nlines");
        let text = std::fs::read_to_string(log.path()).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("[app] two lines"));
        for _ in 0..5 {
            log.append("app", "padding padding padding padding");
        }
        assert!(dir.path().join("logs/pennel.1.log").exists());
        assert!(std::fs::metadata(log.path()).unwrap().len() <= 200);
    }
}
