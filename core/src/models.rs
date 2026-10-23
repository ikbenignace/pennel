use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::error::MusliError;
use crate::http;

const PROGRESS_STEP: u64 = 1024 * 1024;
const CHUNK_BYTES: u64 = 16 * 1024 * 1024;
/// Per-chunk budget; 16 MB in 120 s still allows ~140 KB/s connections.
const CHUNK_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LocalModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ModelState {
    NotDownloaded,
    Partial { bytes: u64 },
    Downloaded,
}

#[uniffi::export(with_foreign)]
pub trait DownloadListener: Send + Sync {
    fn on_progress(&self, downloaded: u64, total: u64);
}

#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub url: String,
}

fn catalog() -> Vec<ModelEntry> {
    let hf = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    vec![
        ModelEntry {
            id: "whisper-large-v3-turbo-q5_0".into(),
            name: "Whisper large-v3 turbo".into(),
            description: "Beste kwaliteit, ook in het Nederlands".into(),
            size_bytes: 574_041_195,
            sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2".into(),
            url: format!("{hf}/ggml-large-v3-turbo-q5_0.bin"),
        },
        ModelEntry {
            id: "whisper-small-q5_1".into(),
            name: "Whisper small".into(),
            description: "Sneller en lichter, iets minder nauwkeurig".into(),
            size_bytes: 190_085_487,
            sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb".into(),
            url: format!("{hf}/ggml-small-q5_1.bin"),
        },
    ]
}

#[uniffi::export]
pub fn local_models() -> Vec<LocalModel> {
    catalog()
        .into_iter()
        .map(|e| LocalModel { id: e.id, name: e.name, description: e.description, size_bytes: e.size_bytes })
        .collect()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct ModelStore {
    dir: PathBuf,
    catalog: Vec<ModelEntry>,
    chunk_bytes: u64,
}

impl ModelStore {
    pub fn new(data_dir: &Path) -> Self {
        Self::with_catalog(data_dir, catalog())
    }

    pub fn with_catalog(data_dir: &Path, catalog: Vec<ModelEntry>) -> Self {
        Self { dir: data_dir.join("models"), catalog, chunk_bytes: CHUNK_BYTES }
    }

    fn entry(&self, id: &str) -> Option<&ModelEntry> {
        self.catalog.iter().find(|e| e.id == id)
    }

    fn final_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.bin"))
    }

    fn part_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.part"))
    }

    pub fn state(&self, id: &str) -> ModelState {
        if self.final_path(id).exists() {
            return ModelState::Downloaded;
        }
        match std::fs::metadata(self.part_path(id)) {
            Ok(meta) => ModelState::Partial { bytes: meta.len() },
            Err(_) => ModelState::NotDownloaded,
        }
    }

    pub fn path(&self, id: &str) -> Option<PathBuf> {
        let path = self.final_path(id);
        path.exists().then_some(path)
    }

    pub fn downloaded(&self) -> Vec<String> {
        self.catalog.iter().filter(|e| self.final_path(&e.id).exists()).map(|e| e.id.clone()).collect()
    }

    /// Downloads in bounded Range requests so a stalled connection times out instead of hanging,
    /// and cancel takes effect within one chunk.
    pub fn download(&self, id: &str, listener: &dyn DownloadListener, cancel: &AtomicBool) -> Result<(), MusliError> {
        let entry = self.entry(id).ok_or(MusliError::NotFound)?.clone();
        if self.path(id).is_some() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir).map_err(MusliError::storage)?;
        let part = self.part_path(id);
        let mut file = OpenOptions::new().create(true).append(true).open(&part).map_err(MusliError::storage)?;
        let mut have = file.metadata().map_err(MusliError::storage)?.len();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(CHUNK_TIMEOUT_SECS)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut restarted = false;
        let mut progress = Progress { listener, total: entry.size_bytes, next_report: have + PROGRESS_STEP };

        while have < entry.size_bytes {
            if cancel.load(Ordering::SeqCst) {
                return Err(MusliError::Cancelled);
            }
            let range = range_header(have, entry.size_bytes, self.chunk_bytes);
            let mut response = agent
                .get(&entry.url)
                .header("Range", range)
                .call()
                .map_err(|e| MusliError::Network { message: e.to_string() })?;
            match response.status().as_u16() {
                206 => {}
                200 => {
                    file.set_len(0).map_err(MusliError::storage)?;
                    have = 0;
                }
                416 if !restarted => {
                    restarted = true;
                    file.set_len(0).map_err(MusliError::storage)?;
                    have = 0;
                    continue;
                }
                status => {
                    let body = response.body_mut().read_to_string().unwrap_or_default();
                    return Err(http::map_status(status, &body));
                }
            }
            let reader = response.into_body().into_with_config().limit(u64::MAX).reader();
            have = append_body(reader, &mut file, have, &mut progress, cancel)?;
        }
        file.flush().map_err(MusliError::storage)?;
        drop(file);

        if sha256_file(&part)? != entry.sha256 {
            let _ = std::fs::remove_file(&part);
            return Err(MusliError::InvalidResponse { message: "checksum mismatch".into() });
        }
        std::fs::rename(&part, self.final_path(id)).map_err(MusliError::storage)?;
        listener.on_progress(entry.size_bytes, entry.size_bytes);
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), MusliError> {
        for path in [self.final_path(id), self.part_path(id)] {
            match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(MusliError::storage(e)),
                _ => {}
            }
        }
        Ok(())
    }
}

pub(crate) fn range_header(have: u64, size: u64, chunk: u64) -> String {
    format!("bytes={have}-{}", (have + chunk).min(size) - 1)
}

struct Progress<'a> {
    listener: &'a dyn DownloadListener,
    total: u64,
    next_report: u64,
}

fn append_body(
    mut reader: impl Read,
    file: &mut File,
    mut have: u64,
    progress: &mut Progress,
    cancel: &AtomicBool,
) -> Result<u64, MusliError> {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(MusliError::Cancelled);
        }
        let n = reader.read(&mut buf).map_err(|e| MusliError::Network { message: e.to_string() })?;
        if n == 0 {
            return Ok(have);
        }
        file.write_all(&buf[..n]).map_err(MusliError::storage)?;
        have += n as u64;
        if have >= progress.next_report {
            progress.listener.on_progress(have, progress.total);
            progress.next_report = have + PROGRESS_STEP;
        }
    }
}

fn sha256_file(path: &Path) -> Result<String, MusliError> {
    let mut file = File::open(path).map_err(MusliError::storage)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(MusliError::storage)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

pub type CancelFlag = Arc<AtomicBool>;

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::sync::Mutex;

    const MB: usize = 1024 * 1024;

    fn payload() -> Vec<u8> {
        (0..3 * MB).map(|i| (i % 251) as u8).collect()
    }

    fn store(dir: &tempfile::TempDir, url: String, bytes: &[u8]) -> ModelStore {
        ModelStore::with_catalog(
            dir.path(),
            vec![ModelEntry {
                id: "tiny".into(),
                name: "Tiny".into(),
                description: "test".into(),
                size_bytes: bytes.len() as u64,
                sha256: hex(&Sha256::digest(bytes)),
                url,
            }],
        )
    }

    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<(u64, u64)>>,
        cancel_on_first: Option<Arc<AtomicBool>>,
    }

    impl DownloadListener for Recorder {
        fn on_progress(&self, downloaded: u64, total: u64) {
            self.calls.lock().unwrap().push((downloaded, total));
            if let Some(flag) = &self.cancel_on_first {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    #[test]
    fn catalog_has_turbo_and_small() {
        let models = local_models();
        let turbo = models.iter().find(|m| m.id == "whisper-large-v3-turbo-q5_0").unwrap();
        assert_eq!(turbo.size_bytes, 574_041_195);
        let small = models.iter().find(|m| m.id == "whisper-small-q5_1").unwrap();
        assert_eq!(small.size_bytes, 190_085_487);
    }

    #[test]
    fn download_verifies_and_marks_downloaded() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        server.mock("GET", "/m.bin").with_body(&bytes).create();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        assert_eq!(store.state("tiny"), ModelState::NotDownloaded);
        let listener = Recorder::default();
        store.download("tiny", &listener, &AtomicBool::new(false)).unwrap();
        assert_eq!(store.state("tiny"), ModelState::Downloaded);
        assert_eq!(std::fs::read(store.path("tiny").unwrap()).unwrap(), bytes);
        let total = bytes.len() as u64;
        assert_eq!(listener.calls.lock().unwrap().last(), Some(&(total, total)));
    }

    #[test]
    fn resumes_from_partial() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        std::fs::create_dir_all(dir.path().join("models")).unwrap();
        std::fs::write(dir.path().join("models/tiny.part"), &bytes[..MB]).unwrap();
        assert_eq!(store.state("tiny"), ModelState::Partial { bytes: MB as u64 });
        let mock = server
            .mock("GET", "/m.bin")
            .match_header("range", mockito::Matcher::Regex("^bytes=1048576-".into()))
            .with_status(206)
            .with_body(&bytes[MB..])
            .create();
        store.download("tiny", &Recorder::default(), &AtomicBool::new(false)).unwrap();
        mock.assert();
        assert_eq!(std::fs::read(store.path("tiny").unwrap()).unwrap(), bytes);
    }

    #[test]
    fn complete_part_is_verified_without_request() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        std::fs::create_dir_all(dir.path().join("models")).unwrap();
        std::fs::write(dir.path().join("models/tiny.part"), &bytes).unwrap();
        store.download("tiny", &Recorder::default(), &AtomicBool::new(false)).unwrap();
        assert_eq!(store.state("tiny"), ModelState::Downloaded);
    }

    #[test]
    fn range_not_satisfiable_restarts_from_zero() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        std::fs::create_dir_all(dir.path().join("models")).unwrap();
        std::fs::write(dir.path().join("models/tiny.part"), &bytes[..MB]).unwrap();
        server
            .mock("GET", "/m.bin")
            .match_header("range", mockito::Matcher::Regex("^bytes=1048576-".into()))
            .with_status(416)
            .create();
        server
            .mock("GET", "/m.bin")
            .match_header("range", mockito::Matcher::Regex("^bytes=0-".into()))
            .with_status(206)
            .with_body(&bytes)
            .create();
        store.download("tiny", &Recorder::default(), &AtomicBool::new(false)).unwrap();
        assert_eq!(std::fs::read(store.path("tiny").unwrap()).unwrap(), bytes);
    }

    #[test]
    fn ranges_are_bounded_chunks() {
        assert_eq!(range_header(0, 10, 4), "bytes=0-3");
        assert_eq!(range_header(8, 10, 4), "bytes=8-9");
    }

    #[test]
    fn checksum_mismatch_removes_part() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        server.mock("GET", "/m.bin").with_body(vec![0u8; bytes.len()]).create();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        let err = store.download("tiny", &Recorder::default(), &AtomicBool::new(false)).unwrap_err();
        assert!(matches!(err, MusliError::InvalidResponse { .. }), "{err:?}");
        assert_eq!(store.state("tiny"), ModelState::NotDownloaded);
    }

    #[test]
    fn cancel_keeps_partial() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        server.mock("GET", "/m.bin").with_body(&bytes).create();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        let cancel = Arc::new(AtomicBool::new(false));
        let listener = Recorder { cancel_on_first: Some(cancel.clone()), ..Default::default() };
        assert_eq!(store.download("tiny", &listener, &cancel).unwrap_err(), MusliError::Cancelled);
        assert!(matches!(store.state("tiny"), ModelState::Partial { .. }));
    }

    #[test]
    fn delete_removes_model() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let bytes = payload();
        server.mock("GET", "/m.bin").with_body(&bytes).create();
        let store = store(&dir, format!("{}/m.bin", server.url()), &bytes);
        store.download("tiny", &Recorder::default(), &AtomicBool::new(false)).unwrap();
        store.delete("tiny").unwrap();
        assert_eq!(store.state("tiny"), ModelState::NotDownloaded);
        assert_eq!(store.path("tiny"), None);
    }

    /// Downloads the real small model from Hugging Face in chunked Range requests.
    #[test]
    #[ignore]
    fn real_download_small_model() {
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::new(dir.path());
        let started = std::time::Instant::now();
        store.download("whisper-small-q5_1", &Recorder::default(), &AtomicBool::new(false)).unwrap();
        eprintln!("downloaded in {:?}", started.elapsed());
        assert_eq!(store.state("whisper-small-q5_1"), ModelState::Downloaded);
    }

    #[test]
    fn unknown_model_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = ModelStore::new(dir.path());
        let err = store.download("nope", &Recorder::default(), &AtomicBool::new(false)).unwrap_err();
        assert_eq!(err, MusliError::NotFound);
    }
}
