use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::chunker::{Chunk, Chunker};
use crate::engine::{ApiKeys, MusliEngine};
use crate::error::MusliError;
use crate::meetings::{MeetingStore, Segment, Speaker};
use crate::speech::is_hallucination;
use crate::transcript_filter::filter_segments;
use crate::wav::encode_wav;

const SAMPLE_RATE: u32 = 16_000;
const ATTEMPTS: usize = 2;

struct Job {
    speaker: Speaker,
    chunk: Chunk,
}

#[derive(Default)]
struct Progress {
    segments: Vec<Segment>,
    dictations: Vec<(u64, u64)>,
    pending: u32,
}

impl Progress {
    fn visible(&self) -> Vec<Segment> {
        filter_segments(&self.segments, &self.dictations)
    }
}

/// Keyed by whole seconds because transcript.md only keeps `[mm:ss]`, so a reread start must find the file.
pub(crate) fn failed_path(meeting_dir: &Path, speaker: Speaker, start_ms: u64) -> PathBuf {
    let start_ms = start_ms / 1000 * 1000;
    let who = match speaker {
        Speaker::Me => "me",
        Speaker::Others => "others",
    };
    meeting_dir.join("failed").join(format!("{who}-{start_ms}.wav"))
}

/// A recording in progress: audio goes in per speaker, transcript segments come out in the background.
#[derive(uniffi::Object)]
pub struct MeetingSession {
    id: String,
    store: MeetingStore,
    chunkers: Mutex<[Chunker; 2]>,
    pushed_samples: Mutex<[u64; 2]>,
    dictating: std::sync::atomic::AtomicBool,
    sender: Mutex<Option<Sender<Job>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    progress: Arc<Mutex<Progress>>,
}

fn index(speaker: Speaker) -> usize {
    match speaker {
        Speaker::Me => 0,
        Speaker::Others => 1,
    }
}

impl MeetingSession {
    pub(crate) fn start(engine: Arc<MusliEngine>, store: MeetingStore, id: String, keys: ApiKeys) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let progress = Arc::new(Mutex::new(Progress::default()));
        let worker_progress = progress.clone();
        let worker_store = MeetingStore::new(store.dir_of(&id).parent().unwrap_or(Path::new(".")));
        let worker_id = id.clone();
        let worker = std::thread::spawn(move || {
            for job in receiver {
                let segment = transcribe_job(&engine, &worker_store, &worker_id, &keys, job);
                let mut progress = worker_progress.lock().unwrap();
                if let Some(segment) = segment {
                    progress.segments.push(segment);
                    progress.segments.sort_by_key(|s| s.start_ms);
                    let _ = worker_store.write_transcript(&worker_id, &progress.visible());
                }
                progress.pending -= 1;
            }
        });
        Arc::new(Self {
            id,
            store,
            chunkers: Mutex::new([Chunker::new(SAMPLE_RATE), Chunker::new(SAMPLE_RATE)]),
            pushed_samples: Mutex::new([0, 0]),
            dictating: std::sync::atomic::AtomicBool::new(false),
            sender: Mutex::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
            progress,
        })
    }

    fn enqueue(&self, speaker: Speaker, chunks: Vec<Chunk>) {
        let sender = self.sender.lock().unwrap();
        let Some(sender) = sender.as_ref() else { return };
        for chunk in chunks {
            self.progress.lock().unwrap().pending += 1;
            if sender.send(Job { speaker, chunk }).is_err() {
                self.progress.lock().unwrap().pending -= 1;
            }
        }
    }
}

fn transcribe_job(engine: &MusliEngine, store: &MeetingStore, id: &str, keys: &ApiKeys, job: Job) -> Option<Segment> {
    let settings = engine.settings();
    let wav = encode_wav(&job.chunk.samples, SAMPLE_RATE);
    let start_ms = job.chunk.start_ms;
    let end_ms = start_ms + job.chunk.samples.len() as u64 * 1000 / u64::from(SAMPLE_RATE);
    let mut result = Err(MusliError::NotFound);
    for _ in 0..ATTEMPTS {
        result = engine.stt(&settings, &settings.meeting_stt, &wav, keys);
        if result.is_ok() {
            break;
        }
    }
    match result {
        Ok(text) if is_hallucination(&text) => None,
        Ok(text) => Some(Segment { start_ms, end_ms, speaker: job.speaker, text, failed: false }),
        Err(_) => {
            let path = failed_path(&store.dir_of(id), job.speaker, start_ms);
            let _ = std::fs::create_dir_all(path.parent().expect("failed dir"));
            let _ = std::fs::write(&path, &wav);
            Some(Segment { start_ms, end_ms, speaker: job.speaker, text: String::new(), failed: true })
        }
    }
}

#[uniffi::export]
impl MeetingSession {
    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn dir(&self) -> String {
        self.store.dir_of(&self.id).to_string_lossy().into_owned()
    }

    /// Accepts 16 kHz mono samples; transcription happens on a background thread.
    pub fn push_audio(&self, speaker: Speaker, samples: Vec<f32>) {
        let samples = if speaker == Speaker::Me && self.dictating.load(std::sync::atomic::Ordering::SeqCst) {
            vec![0.0; samples.len()]
        } else {
            samples
        };
        self.pushed_samples.lock().unwrap()[index(speaker)] += samples.len() as u64;
        let chunks = self.chunkers.lock().unwrap()[index(speaker)].push(&samples);
        self.enqueue(speaker, chunks);
    }

    pub fn set_notes(&self, text: String) -> Result<(), MusliError> {
        self.store.write_notes(&self.id, &text)
    }

    pub fn transcript(&self) -> Vec<Segment> {
        self.progress.lock().unwrap().visible()
    }

    /// While true, the user's microphone is replaced by silence: they are dictating into another app.
    pub fn set_dictating(&self, dictating: bool) {
        self.dictating.store(dictating, std::sync::atomic::Ordering::SeqCst);
    }

    /// Marks a period (meeting timeline, ms) in which the user dictated into another app.
    pub fn mark_dictation(&self, start_ms: u64, end_ms: u64) {
        self.progress.lock().unwrap().dictations.push((start_ms, end_ms));
    }

    pub fn pending(&self) -> u32 {
        self.progress.lock().unwrap().pending
    }

    /// Flushes the remaining audio, waits for every chunk to be transcribed and records the duration.
    pub fn stop(&self) -> Result<(), MusliError> {
        for speaker in [Speaker::Me, Speaker::Others] {
            let rest = self.chunkers.lock().unwrap()[index(speaker)].flush();
            self.enqueue(speaker, rest.into_iter().collect());
        }
        self.sender.lock().unwrap().take();
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
        self.store.write_transcript(&self.id, &self.transcript())?;
        let longest = *self.pushed_samples.lock().unwrap().iter().max().unwrap_or(&0);
        let minutes = longest.div_ceil(u64::from(SAMPLE_RATE) * 60) as u32;
        self.store.set_duration(&self.id, minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{ApiKeys, MusliEngine};
    use crate::meetings::Speaker;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    const SR: usize = 16_000;

    fn tone(seconds: f32) -> Vec<f32> {
        (0..(seconds * SR as f32) as usize)
            .map(|i| 0.2 * (i as f32 * 440.0 * std::f32::consts::TAU / SR as f32).sin())
            .collect()
    }

    fn keys() -> ApiKeys {
        ApiKeys { by_provider: HashMap::from([("groq".to_owned(), "k".to_owned())]) }
    }

    fn engine(dir: &tempfile::TempDir, server: &mockito::Server) -> Arc<MusliEngine> {
        let engine = MusliEngine::new(dir.path().join("data").to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.providers[0].base_url = server.url();
        settings.meetings_dir = dir.path().join("meetings").to_string_lossy().into_owned();
        engine.update_settings(settings).unwrap();
        engine
    }

    fn stt_ok(server: &mut mockito::Server, text: &str) -> mockito::Mock {
        server
            .mock("POST", "/audio/transcriptions")
            .with_body(serde_json::json!({ "text": text }).to_string())
            .create()
    }

    fn wait_idle(session: &MeetingSession) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while session.pending() > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn session_transcribes_both_speakers_in_order() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        stt_ok(&mut server, "hallo");
        let session = engine.clone().start_meeting("Sync".into(), keys()).unwrap();
        session.push_audio(Speaker::Me, tone(20.0));
        session.push_audio(Speaker::Others, [vec![0.0; 5 * SR], tone(20.0)].concat());
        session.stop().unwrap();
        let transcript = session.transcript();
        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript[0].speaker, Speaker::Me);
        assert_eq!(transcript[1].speaker, Speaker::Others);
        assert_eq!(transcript[0].end_ms, 20_000);
        let md = std::fs::read_to_string(PathBuf::from(session.dir()).join("transcript.md")).unwrap();
        assert!(md.contains("**Ik:** hallo") && md.contains("**Anderen:** hallo"), "{md}");
        assert_eq!(engine.meetings()[0].duration_min, 1);
    }

    #[test]
    fn transcript_is_written_after_each_chunk() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        stt_ok(&mut server, "eerste stuk");
        let session = engine.clone().start_meeting("Crash".into(), keys()).unwrap();
        session.push_audio(Speaker::Me, tone(31.0));
        wait_idle(&session);
        let md = std::fs::read_to_string(PathBuf::from(session.dir()).join("transcript.md")).unwrap();
        assert!(md.contains("eerste stuk"), "{md}");
    }

    #[test]
    fn failed_chunk_is_saved_and_retranscribed() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let failing = server.mock("POST", "/audio/transcriptions").with_status(503).expect(2).create();
        let session = engine.clone().start_meeting("Wifi weg".into(), keys()).unwrap();
        session.push_audio(Speaker::Me, tone(10.0));
        session.stop().unwrap();
        failing.assert();
        let transcript = session.transcript();
        assert!(transcript[0].failed);
        assert!(PathBuf::from(session.dir()).join("failed/me-0.wav").exists());

        failing.remove();
        stt_ok(&mut server, "hersteld");
        assert_eq!(engine.retranscribe_failed(session.id(), keys()).unwrap(), 1);
        let fixed = engine.meeting_transcript(session.id());
        assert!(!fixed[0].failed);
        assert_eq!(fixed[0].text, "hersteld");
        assert!(!PathBuf::from(session.dir()).join("failed/me-0.wav").exists());
    }

    #[test]
    fn failed_chunk_off_the_second_is_retranscribed() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let first = stt_ok(&mut server, "eerste");
        let session = engine.clone().start_meeting("Gat".into(), keys()).unwrap();
        let quiet = vec![0.0; SR];
        session.push_audio(Speaker::Me, [tone(20.0), quiet, tone(25.0)].concat());
        wait_idle(&session);
        first.remove();
        let failing = server.mock("POST", "/audio/transcriptions").with_status(503).expect(2).create();
        session.stop().unwrap();
        failing.assert();
        let failed = session.transcript().into_iter().find(|s| s.failed).unwrap();
        assert_ne!(failed.start_ms % 1000, 0, "test needs an off-second chunk start");

        failing.remove();
        stt_ok(&mut server, "tweede");
        assert_eq!(engine.retranscribe_failed(session.id(), keys()).unwrap(), 1);
        assert!(engine.meeting_transcript(session.id()).iter().all(|s| !s.failed));
    }

    #[test]
    fn audio_while_dictating_is_silenced() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let mock = stt_ok(&mut server, "stuur de planning naar Sarah").expect(0);
        let session = engine.clone().start_meeting("Dictaat".into(), keys()).unwrap();
        session.set_dictating(true);
        session.push_audio(Speaker::Me, tone(8.0));
        session.set_dictating(false);
        session.stop().unwrap();
        mock.assert();
        assert!(session.transcript().is_empty());
    }

    #[test]
    fn session_applies_filters_to_transcript_md() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        stt_ok(&mut server, "stuur de planning naar Sarah vandaag");
        let session = engine.clone().start_meeting("Dictaat".into(), keys()).unwrap();
        session.push_audio(Speaker::Me, tone(10.0));
        session.mark_dictation(0, 10_000);
        session.stop().unwrap();
        assert!(session.transcript().is_empty());
        let md = std::fs::read_to_string(PathBuf::from(session.dir()).join("transcript.md")).unwrap();
        assert!(!md.contains("Sarah"), "{md}");
    }

    #[test]
    fn stop_flushes_remainder() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let mock = stt_ok(&mut server, "kort").expect(1);
        let session = engine.clone().start_meeting("Kort".into(), keys()).unwrap();
        session.push_audio(Speaker::Me, tone(3.0));
        assert_eq!(session.pending(), 0);
        session.stop().unwrap();
        mock.assert();
        assert_eq!(session.transcript()[0].text, "kort");
    }

    #[test]
    fn notes_are_written_and_meeting_is_listed() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let session = engine.clone().start_meeting("Notities".into(), keys()).unwrap();
        session.set_notes("- export naar Excel".into()).unwrap();
        assert_eq!(engine.meeting_notes(session.id()), "- export naar Excel");
        assert_eq!(engine.meetings()[0].id, session.id());
        assert_eq!(engine.search_meetings("notities".into()).len(), 1);
        session.stop().unwrap();
    }
}
