//! Records a meeting as two timestamped tracks. The native/runtime owns capture independently of
//! any webview; archived audio and transcript ingestion share the same monotonic session clock.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(target_os = "macos")]
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
#[cfg(target_os = "macos")]
use std::time::Duration;

use pennel_core::{MeetingSession, Speaker};

#[cfg(not(target_os = "macos"))]
use crate::audio::{Capture, Source};
use crate::diagnostics::Log;
#[cfg(target_os = "linux")]
use crate::linux_meetings as archive_backend;
#[cfg(windows)]
use crate::windows_meetings as archive_backend;

const LOG: Log = Log::new("meeting");
#[cfg(any(not(target_os = "macos"), test))]
const BATCH: usize = 16_000;

#[cfg(any(target_os = "macos", test))]
fn start_with_echo_recovery<T>(
    echo: bool,
    mut start: impl FnMut(bool) -> Result<T, (String, bool)>,
    mut wait: impl FnMut(),
) -> Result<T, String> {
    match start(echo) {
        Ok(capture) => return Ok(capture),
        Err((error, retryable)) if !echo || !retryable => return Err(error),
        Err(_) => {}
    }
    for attempt in 0..3 {
        wait();
        match start(false) {
            Ok(capture) => return Ok(capture),
            Err((error, retryable)) if !retryable || attempt == 2 => return Err(error),
            Err(_) => {}
        }
    }
    unreachable!()
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
struct MacCaptureHealth {
    microphone_active: bool,
    microphone_error: Option<String>,
    system_audio_available: bool,
    system_audio_error: Option<String>,
    echo_cancellation: bool,
    archive_error: Option<String>,
    capture_warning: Option<String>,
    terminal_error: Option<String>,
}

#[cfg(target_os = "macos")]
impl MacCaptureHealth {
    fn from_status(status: serde_json::Value) -> Self {
        serde_json::from_value(status).unwrap_or_else(|_| Self {
            capture_warning: Some("De opnamestatus kon niet worden gecontroleerd.".into()),
            ..Self::default()
        })
    }

    fn warning(&self) -> Option<String> {
        let mut warnings = Vec::new();
        for warning in [
            &self.microphone_error,
            &self.system_audio_error,
            &self.archive_error,
            &self.capture_warning,
        ]
        .into_iter()
        .flatten()
        {
            let warning = crate::commands::localize_error_message(warning);
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        (!warnings.is_empty()).then(|| warnings.join(" "))
    }
}

/// The status ABI marshals synchronously through AppKit. Its owner polls without Shared's locks;
/// status consumers only read the cache and normal capture finalization runs on a worker.
#[cfg(target_os = "macos")]
struct MacCaptureMonitor {
    health: Arc<Mutex<MacCaptureHealth>>,
    stop: Option<SyncSender<()>>,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
}

#[cfg(target_os = "macos")]
impl MacCaptureMonitor {
    fn start(native: crate::macos::MeetingCapture) -> Self {
        let health = Arc::new(Mutex::new(MacCaptureHealth::from_status(native.status())));
        let next_health = health.clone();
        let (stop, receiver) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            loop {
                match receiver.recv_timeout(Duration::from_secs(1)) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break native.stop(),
                    Err(RecvTimeoutError::Timeout) => {
                        let snapshot = MacCaptureHealth::from_status(native.status());
                        *next_health.lock().unwrap() = snapshot;
                    }
                }
            }
        });
        Self {
            health,
            stop: Some(stop),
            worker: Some(worker),
        }
    }

    fn snapshot(&self) -> MacCaptureHealth {
        self.health.lock().unwrap().clone()
    }

    fn stop(mut self) -> Result<(), String> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.try_send(());
        }
        self.worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .unwrap_or_else(|_| Err("De opnamestatus is onverwacht gestopt.".into()))
        })
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacCaptureMonitor {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.try_send(());
        }
        // Never join an AppKit-bound worker from a possible main-thread destruction path.
        let _ = self.worker.take();
    }
}

pub struct MeetingRecorder {
    pub session: Arc<MeetingSession>,
    pub title: String,
    started: Instant,
    #[cfg(target_os = "macos")]
    native: Option<MacCaptureMonitor>,
    #[cfg(not(target_os = "macos"))]
    mic: Option<Microphone>,
    #[cfg(not(target_os = "macos"))]
    system: Option<Capture>,
    has_system: bool,
    #[cfg(not(target_os = "macos"))]
    tracks: [Arc<Mutex<TrackSink>>; 2],
    #[cfg(not(target_os = "macos"))]
    capture_warning: Option<String>,
    #[cfg(windows)]
    echo_active: bool,
    dictating_since: Mutex<Option<u64>>,
    heard_system: Arc<AtomicBool>,
}

#[cfg(not(target_os = "macos"))]
enum Microphone {
    Raw(Capture),
    #[cfg(windows)]
    Echo(crate::windows_meetings::EchoCapture),
}

#[cfg(not(target_os = "macos"))]
impl Microphone {
    fn error(&self) -> Option<String> {
        match self {
            Self::Raw(capture) => capture.error(),
            #[cfg(windows)]
            Self::Echo(capture) => capture.error(),
        }
    }

    fn stop(self) -> Result<(), String> {
        match self {
            Self::Raw(capture) => capture.stop_checked(),
            #[cfg(windows)]
            Self::Echo(capture) => capture.stop(),
        }
    }
}

#[cfg(any(not(target_os = "macos"), test))]
enum TrackBlock<'a> {
    Audio { start_ms: u64, samples: &'a [f32] },
    Gap { samples: u64 },
}

#[cfg(any(not(target_os = "macos"), test))]
#[derive(Default)]
struct TimedTrack {
    position: u64,
    pending_start: u64,
    pending: Vec<f32>,
}

#[cfg(any(not(target_os = "macos"), test))]
impl TimedTrack {
    fn push_at(
        &mut self,
        start_ms: u64,
        samples: &[f32],
        mut emit: impl FnMut(TrackBlock<'_>),
    ) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        let mut start = start_ms.checked_mul(16).ok_or("Ongeldige audiotijd")?;
        // The shared API timestamps have millisecond precision. Avoid accumulating sub-ms
        // clock-bridge rounding into repeated drops/insertions at every device callback.
        if start.abs_diff(self.position) < 16 {
            start = self.position;
        }
        let end = start
            .checked_add(samples.len() as u64)
            .ok_or("Ongeldige audiolengte")?;
        if end <= self.position {
            return Ok(());
        }
        if start > self.position {
            self.flush(&mut emit);
            emit(TrackBlock::Gap {
                samples: start - self.position,
            });
            self.position = start;
        }
        let overlap = self.position.saturating_sub(start) as usize;
        let mut remaining = &samples[overlap..];
        while !remaining.is_empty() {
            if self.pending.is_empty() {
                self.pending_start = self.position;
            }
            let take = remaining.len().min(BATCH - self.pending.len());
            self.pending.extend_from_slice(&remaining[..take]);
            self.position += take as u64;
            remaining = &remaining[take..];
            if self.pending.len() == BATCH {
                self.flush(&mut emit);
            }
        }
        Ok(())
    }

    fn flush(&mut self, mut emit: impl FnMut(TrackBlock<'_>)) {
        if self.pending.is_empty() {
            return;
        }
        let samples = std::mem::take(&mut self.pending);
        emit(TrackBlock::Audio {
            start_ms: self.pending_start / 16,
            samples: &samples,
        });
    }

    fn finish_at(
        &mut self,
        end_ms: u64,
        mut emit: impl FnMut(TrackBlock<'_>),
    ) -> Result<(), String> {
        // A final millisecond also advances a silent/absent track to the shared stop boundary.
        // Large gaps are represented by counts rather than a correspondingly large allocation.
        if end_ms != 0 {
            self.push_at(end_ms - 1, &[0.0; 16], &mut emit)?;
        }
        self.flush(emit);
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
struct TrackSink {
    timing: TimedTrack,
    #[cfg(any(windows, target_os = "linux"))]
    archive: Option<archive_backend::AudioArchive>,
    error: Option<String>,
}

#[cfg(not(target_os = "macos"))]
impl TrackSink {
    fn new(
        #[cfg(any(windows, target_os = "linux"))] archive: archive_backend::AudioArchive,
    ) -> Self {
        Self {
            timing: TimedTrack::default(),
            #[cfg(any(windows, target_os = "linux"))]
            archive: Some(archive),
            error: None,
        }
    }

    fn emit(
        session: &MeetingSession,
        speaker: Speaker,
        #[cfg(any(windows, target_os = "linux"))] archive: &mut Option<
            archive_backend::AudioArchive,
        >,
        error: &mut Option<String>,
        block: TrackBlock<'_>,
    ) {
        let archive_result = match block {
            TrackBlock::Gap { samples } => {
                #[cfg(any(windows, target_os = "linux"))]
                {
                    archive
                        .as_mut()
                        .map_or(Ok(()), |writer| writer.write_silence(samples))
                }
                #[cfg(not(any(windows, target_os = "linux")))]
                {
                    let _ = samples;
                    Ok::<(), String>(())
                }
            }
            TrackBlock::Audio { start_ms, samples } => {
                #[cfg(any(windows, target_os = "linux"))]
                let archived = archive
                    .as_mut()
                    .map_or(Ok(()), |writer| writer.write(samples));
                // An archive failure must never discard otherwise recoverable transcript audio.
                if let Err(failure) = session.push_audio_at(speaker, start_ms, samples.to_vec()) {
                    error.get_or_insert_with(|| failure.to_string());
                }
                #[cfg(any(windows, target_os = "linux"))]
                {
                    archived
                }
                #[cfg(not(any(windows, target_os = "linux")))]
                {
                    Ok::<(), String>(())
                }
            }
        };
        if let Err(failure) = archive_result {
            error.get_or_insert(failure);
        }
    }

    fn push_at(
        &mut self,
        session: &MeetingSession,
        speaker: Speaker,
        start_ms: u64,
        samples: &[f32],
    ) {
        let result = self.timing.push_at(start_ms, samples, |block| {
            Self::emit(
                session,
                speaker,
                #[cfg(any(windows, target_os = "linux"))]
                &mut self.archive,
                &mut self.error,
                block,
            );
        });
        if let Err(failure) = result {
            self.error.get_or_insert(failure);
        }
    }

    fn finish(
        &mut self,
        session: &MeetingSession,
        speaker: Speaker,
        end_ms: u64,
    ) -> Result<(), String> {
        let result = self.timing.finish_at(end_ms, |block| {
            Self::emit(
                session,
                speaker,
                #[cfg(any(windows, target_os = "linux"))]
                &mut self.archive,
                &mut self.error,
                block,
            );
        });
        if let Err(failure) = result {
            self.error.get_or_insert(failure);
        }
        #[cfg(any(windows, target_os = "linux"))]
        if let Some(writer) = self.archive.take()
            && let Err(failure) = writer.finish()
        {
            self.error.get_or_insert(failure);
        }
        self.error.clone().map_or(Ok(()), Err)
    }
}

#[cfg(not(target_os = "macos"))]
fn batching(
    session: Arc<MeetingSession>,
    speaker: Speaker,
    pending: Arc<Mutex<TrackSink>>,
    heard: Option<Arc<AtomicBool>>,
    started: Instant,
) -> impl FnMut(&[f32], Instant) + Send + 'static {
    move |chunk, captured_at| {
        if let Some(heard) = &heard
            && chunk.iter().any(|sample| *sample != 0.0)
        {
            heard.store(true, Ordering::Relaxed);
        }
        let start_ms = captured_at.saturating_duration_since(started).as_millis() as u64;
        pending
            .lock()
            .unwrap()
            .push_at(&session, speaker, start_ms, chunk);
    }
}

impl MeetingRecorder {
    pub fn start(
        session: Arc<MeetingSession>,
        title: String,
        detected_app: Option<String>,
        echo: bool,
    ) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        {
            let heard_system = Arc::new(AtomicBool::new(false));
            let started = Instant::now();
            let directory = std::path::PathBuf::from(session.dir());
            let native = start_with_echo_recovery(
                echo,
                |echo| {
                    let heard = heard_system.clone();
                    let sink = session.clone();
                    crate::macos::MeetingCapture::start(
                        &directory,
                        detected_app.as_deref(),
                        echo,
                        move |track, samples, offset| {
                            let speaker = if track == 0 {
                                Speaker::Me
                            } else {
                                Speaker::Others
                            };
                            if track == 1 && samples.iter().any(|sample| *sample != 0.0) {
                                heard.store(true, Ordering::Relaxed);
                            }
                            if sink
                                .push_audio_at(speaker, offset, samples.to_vec())
                                .is_err()
                            {
                                LOG.error("Audioblok kon niet aan de meeting worden toegevoegd");
                            }
                        },
                    )
                },
                || std::thread::sleep(Duration::from_secs(1)),
            )?;
            let native = MacCaptureMonitor::start(native);
            let has_system = native.snapshot().system_audio_available;
            Ok(Self {
                session,
                title,
                started,
                native: Some(native),
                has_system,
                dictating_since: Mutex::new(None),
                heard_system,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = detected_app;
            #[cfg(any(windows, target_os = "linux"))]
            let tracks = {
                let dir = std::path::PathBuf::from(session.dir());
                [
                    Arc::new(Mutex::new(TrackSink::new(
                        archive_backend::AudioArchive::create(&dir.join("ik.m4a"))?,
                    ))),
                    Arc::new(Mutex::new(TrackSink::new(
                        archive_backend::AudioArchive::create(&dir.join("anderen.m4a"))?,
                    ))),
                ]
            };
            #[cfg(not(any(windows, target_os = "linux")))]
            let tracks = [
                Arc::new(Mutex::new(TrackSink::new())),
                Arc::new(Mutex::new(TrackSink::new())),
            ];
            let started = Instant::now();
            let heard_system = Arc::new(AtomicBool::new(false));
            #[cfg(windows)]
            let (mic, echo_active, mut capture_warning) = if echo {
                match crate::windows_meetings::EchoCapture::start_timed(batching(
                    session.clone(),
                    Speaker::Me,
                    tracks[0].clone(),
                    None,
                    started,
                )) {
                    Ok(capture) => (Microphone::Echo(capture), true, None),
                    Err(error) => {
                        LOG.error(format!("echo-onderdrukking niet actief: {error}"));
                        let capture = Capture::start_timed(
                            Source::Microphone,
                            batching(
                                session.clone(),
                                Speaker::Me,
                                tracks[0].clone(),
                                None,
                                started,
                            ),
                        )
                        .map_err(|error| format!("microfoon: {error}"))?;
                        (
                            Microphone::Raw(capture),
                            false,
                            Some(format!(
                                "Echo-onderdrukking is niet actief: {error}. Gebruik een koptelefoon."
                            )),
                        )
                    }
                }
            } else {
                (
                    Microphone::Raw(
                        Capture::start_timed(
                            Source::Microphone,
                            batching(
                                session.clone(),
                                Speaker::Me,
                                tracks[0].clone(),
                                None,
                                started,
                            ),
                        )
                        .map_err(|error| format!("microfoon: {error}"))?,
                    ),
                    false,
                    None,
                )
            };
            #[cfg(not(windows))]
            let (mic, mut capture_warning) = (
                Microphone::Raw(Capture::start_timed(Source::Microphone, batching(session.clone(), Speaker::Me, tracks[0].clone(), None, started))
                    .map_err(|error| format!("microfoon: {error}"))?),
                echo.then(|| "Echo-onderdrukking is op dit platform niet beschikbaar. Gebruik een koptelefoon.".to_owned()),
            );
            let system = match Capture::start_timed(
                Source::SystemOutput,
                batching(
                    session.clone(),
                    Speaker::Others,
                    tracks[1].clone(),
                    Some(heard_system.clone()),
                    started,
                ),
            ) {
                Ok(capture) => Some(capture),
                Err(error) => {
                    LOG.error(format!("systeemgeluid niet beschikbaar: {error}"));
                    let warning = format!(
                        "Systeemgeluid is niet beschikbaar: {error}. Alleen de microfoon wordt opgenomen."
                    );
                    capture_warning = Some(
                        capture_warning
                            .map_or(warning.clone(), |previous| format!("{previous} {warning}")),
                    );
                    None
                }
            };
            let has_system = system.is_some();
            LOG.info(format!(
                "meeting gestart, systeemgeluid {}",
                if has_system { "aan" } else { "uit" }
            ));
            Ok(Self {
                session,
                title,
                started,
                mic: Some(mic),
                system,
                has_system,
                tracks,
                capture_warning,
                #[cfg(windows)]
                echo_active,
                dictating_since: Mutex::new(None),
                heard_system,
            })
        }
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn started_at(&self) -> Instant {
        self.started
    }

    pub fn has_system_audio(&self) -> bool {
        #[cfg(target_os = "macos")]
        if let Some(native) = &self.native {
            return native.snapshot().system_audio_available;
        }
        self.has_system
    }

    pub fn heard_system_audio(&self) -> bool {
        self.heard_system.load(Ordering::Relaxed)
    }

    #[cfg(any(windows, target_os = "macos"))]
    pub fn echo_cancellation_active(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.native.as_ref().is_some_and(|native| {
                let health = native.snapshot();
                health.microphone_active && health.echo_cancellation
            })
        }
        #[cfg(windows)]
        {
            self.echo_active
                && self
                    .mic
                    .as_ref()
                    .is_some_and(|capture| capture.error().is_none())
        }
    }

    pub fn terminal_error(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            self.native
                .as_ref()
                .and_then(|native| native.snapshot().terminal_error)
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    /// All values are cached/atomic or ordinary Rust mutexes; this is safe under Shared's lock.
    pub fn audio_warning(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            self.native
                .as_ref()
                .and_then(|native| native.snapshot().warning())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut warnings = Vec::new();
            if let Some(error) = self.mic.as_ref().and_then(Microphone::error) {
                warnings.push(format!("Microfoonopname is onderbroken: {error}"));
            }
            if let Some(error) = self.system.as_ref().and_then(Capture::error) {
                warnings.push(format!("Systeemgeluid is onderbroken: {error}"));
            }
            for track in &self.tracks {
                if let Some(error) = &track.lock().unwrap().error {
                    warnings.push(format!("Audio verwerken mislukt: {error}"));
                }
            }
            if let Some(warning) = &self.capture_warning {
                warnings.push(warning.clone());
            }
            (!warnings.is_empty()).then(|| warnings.join(" "))
        }
    }

    /// While dictating, microphone input is suppressed in the meeting transcript. Archived audio
    /// remains the captured track; the recorded interval also filters echoes from the transcript.
    pub fn set_dictating(&self, dictating: bool) {
        self.session.set_dictating(dictating);
        let now_ms = self.started.elapsed().as_millis() as u64;
        let mut since = self.dictating_since.lock().unwrap();
        match (dictating, since.take()) {
            (true, _) => *since = Some(now_ms),
            (false, Some(start)) => self.session.mark_dictation(start, now_ms),
            (false, None) => {}
        }
    }

    /// Capture, archives and transcript are all finalized even if one stage reports an error.
    pub fn stop(mut self) -> Result<String, String> {
        self.set_dictating(false);
        let mut failures = Vec::new();
        #[cfg(target_os = "macos")]
        if let Some(native) = self.native.take()
            && let Err(error) = native.stop()
        {
            failures.push(error);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let end_ms = self.started.elapsed().as_millis() as u64;
            if let Some(mic) = self.mic.take()
                && let Err(error) = mic.stop()
            {
                failures.push(error);
            }
            if let Some(system) = self.system.take()
                && let Err(error) = system.stop_checked()
            {
                failures.push(error);
            }
            for (speaker, track) in [Speaker::Me, Speaker::Others].into_iter().zip(&self.tracks) {
                if let Err(error) = track.lock().unwrap().finish(&self.session, speaker, end_ms) {
                    failures.push(error);
                }
            }
        }
        if let Err(error) = self.session.stop() {
            failures.push(error.to_string());
        }
        LOG.info(format!(
            "meeting gestopt na {} s",
            self.started.elapsed().as_secs()
        ));
        if !failures.is_empty() {
            return Err(failures.join("; "));
        }
        Ok(self.session.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_start_failure_retries_raw_microphone_after_device_recovers() {
        let mut modes = Vec::new();
        let mut waits = 0;
        let result = start_with_echo_recovery(
            true,
            |echo| {
                modes.push(echo);
                if modes.len() < 3 {
                    Err(("unavailable".to_owned(), true))
                } else {
                    Ok(42)
                }
            },
            || waits += 1,
        );
        assert_eq!(result.unwrap(), 42);
        assert_eq!(modes, [true, false, false]);
        assert_eq!(waits, 2);
    }

    #[test]
    fn permission_failure_is_not_retried() {
        let mut attempts = 0;
        let result: Result<(), _> = start_with_echo_recovery(
            true,
            |_| {
                attempts += 1;
                Err(("permission denied".to_owned(), false))
            },
            || panic!("permission failure must not be retried"),
        );
        assert_eq!(result.unwrap_err(), "permission denied");
        assert_eq!(attempts, 1);
    }

    #[test]
    fn raw_start_failure_without_echo_is_not_retried() {
        let mut attempts = 0;
        let result: Result<(), _> = start_with_echo_recovery(
            false,
            |_| {
                attempts += 1;
                Err(("microphone failed".to_owned(), true))
            },
            || panic!("raw-only start must not be retried as an echo fallback"),
        );
        assert_eq!(result.unwrap_err(), "microphone failed");
        assert_eq!(attempts, 1);
    }

    #[test]
    fn raw_capture_failure_is_bounded_and_reported() {
        let mut attempts = 0;
        let result: Result<(), _> = start_with_echo_recovery(
            true,
            |_| {
                attempts += 1;
                Err(("microphone failed".to_owned(), true))
            },
            || {},
        );
        assert_eq!(result.unwrap_err(), "microphone failed");
        assert_eq!(attempts, 4);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_status_exposes_actual_echo_and_degraded_system_audio() {
        let status = MacCaptureHealth::from_status(serde_json::json!({
            "microphone_active": true,
            "system_audio_available": false,
            "echo_cancellation": false,
            "system_audio_error": "Alleen de microfoon wordt opgenomen.",
            "capture_warning": "Echo-onderdrukking is niet actief.",
            "terminal_error": null
        }));
        assert!(status.microphone_active);
        assert!(!status.system_audio_available);
        assert!(!status.echo_cancellation);
        assert_eq!(status.terminal_error, None);
        assert_eq!(
            status.warning().as_deref(),
            Some("Only the microphone is being recorded. Echo cancellation is not active.")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_archive_error_remains_actionable_in_cached_health() {
        let failure = "Audio bewaren mislukt.";
        let status = MacCaptureHealth::from_status(serde_json::json!({
            "microphone_active": true,
            "system_audio_available": true,
            "archive_error": failure,
            "terminal_error": failure
        }));
        assert_eq!(status.terminal_error.as_deref(), Some(failure));
        assert_eq!(status.warning().as_deref(), Some("Saving audio failed."));
        assert!(status.microphone_active);
        assert!(status.system_audio_available);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn malformed_native_health_is_visible_without_claiming_a_terminal_capture_failure() {
        let status = MacCaptureHealth::from_status(serde_json::Value::Null);
        assert!(status.warning().is_some());
        assert!(status.terminal_error.is_none());
    }

    #[derive(Default)]
    struct Collected {
        audio: Vec<f32>,
        starts: Vec<u64>,
        max_batch: usize,
    }

    impl Collected {
        fn block(&mut self, block: TrackBlock<'_>) {
            match block {
                TrackBlock::Gap { samples } => self
                    .audio
                    .extend(std::iter::repeat_n(0.0, samples as usize)),
                TrackBlock::Audio { start_ms, samples } => {
                    self.starts.push(start_ms);
                    self.max_batch = self.max_batch.max(samples.len());
                    self.audio.extend_from_slice(samples);
                }
            }
        }
    }

    #[test]
    fn delayed_track_has_leading_silence_and_shared_offsets() {
        let mut track = TimedTrack::default();
        let mut collected = Collected::default();
        track
            .push_at(250, &vec![0.5; 16_000], |block| collected.block(block))
            .unwrap();
        assert_eq!(collected.starts, [250]);
        assert_eq!(collected.audio.len(), 20_000);
        assert!(collected.audio[..4_000].iter().all(|sample| *sample == 0.0));
        assert!(collected.audio[4_000..].iter().all(|sample| *sample == 0.5));
    }

    #[test]
    fn device_gap_flushes_old_batch_then_preserves_archive_timeline() {
        let mut track = TimedTrack::default();
        let mut collected = Collected::default();
        track
            .push_at(0, &vec![0.5; 8_000], |block| collected.block(block))
            .unwrap();
        track
            .push_at(2_000, &vec![-0.5; 8_000], |block| collected.block(block))
            .unwrap();
        track
            .finish_at(3_000, |block| collected.block(block))
            .unwrap();
        assert_eq!(collected.audio.len(), 48_000);
        assert!(
            collected.audio[8_000..32_000]
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            collected.audio[32_000..40_000]
                .iter()
                .all(|sample| *sample == -0.5)
        );
        assert!(collected.max_batch <= BATCH);
        assert_eq!(collected.starts[..2], [0, 2_000]);
    }

    #[test]
    fn repeated_or_overlapping_packets_are_trimmed_once() {
        let mut track = TimedTrack::default();
        let mut collected = Collected::default();
        track
            .push_at(0, &vec![1.0; 8_000], |block| collected.block(block))
            .unwrap();
        track
            .push_at(250, &vec![2.0; 8_000], |block| collected.block(block))
            .unwrap();
        track
            .push_at(0, &[9.0; 16], |block| collected.block(block))
            .unwrap();
        track.flush(|block| collected.block(block));
        assert_eq!(collected.audio.len(), 12_000);
        assert!(collected.audio[..8_000].iter().all(|sample| *sample == 1.0));
        assert!(collected.audio[8_000..].iter().all(|sample| *sample == 2.0));
    }

    #[test]
    fn huge_packet_is_emitted_in_bounded_batches() {
        let mut track = TimedTrack::default();
        let mut collected = Collected::default();
        track
            .push_at(0, &vec![0.3; 100_000], |block| collected.block(block))
            .unwrap();
        assert!(track.pending.len() < BATCH);
        track.flush(|block| collected.block(block));
        assert_eq!(collected.audio.len(), 100_000);
        assert!(collected.max_batch <= BATCH);
        assert!(track.push_at(u64::MAX, &[0.0], |_| {}).is_err());
    }
}
