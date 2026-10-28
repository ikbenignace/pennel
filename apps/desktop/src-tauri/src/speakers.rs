//! Speaker recognition for finished meetings and learning the user's voice. Enrollment audio stays
//! in memory and is dropped as soon as the core has taken its voiceprint.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pennel_core::{PennelError, SpeakerRecognition, VoiceProfile};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::audio::{CaptureError, Recorder};
use crate::diagnostics::Log;
use crate::state::Shared;

const SAMPLE_RATE: usize = 16_000;
pub const ENROLLMENT_MAX_SECS: u64 = 60;
const ENROLLMENT_EVENT: &str = "voice-enrollment";
const LOG: Log = Log::new("speakers");

/// The core's own messages are Dutch, but its error type prefixes some of them in English.
pub fn speaker_error(error: PennelError) -> String {
    crate::commands::localize_error_message(speaker_error_source(error))
}

fn speaker_error_source(error: PennelError) -> String {
    match error {
        PennelError::InvalidResponse { message } | PennelError::Cloud { message, .. } => message,
        PennelError::ModelNotDownloaded => {
            "Download eerst de sprekermodellen bij Instellingen → Meetings.".into()
        }
        PennelError::NotFound => "Meeting of stem niet gevonden.".into(),
        PennelError::Network { .. } => {
            "Pennel Cloud is niet bereikbaar. Probeer opnieuw als je weer online bent.".into()
        }
        PennelError::Storage { .. } => "Bewaren is mislukt. Probeer opnieuw.".into(),
        other => other.to_string(),
    }
}

/// The meeting's own audio files, microphone first. A track that resolves outside the meeting
/// folder (a symlink, say) is ignored.
pub fn meeting_tracks(dir: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let tracks = pennel_core::resolve_meeting_audio_tracks(dir);
    let track = |speaker| {
        tracks
            .iter()
            .find(|track| track.speaker == speaker && track.available)
            .and_then(|track| dir.join(&track.relative_path).canonicalize().ok())
    };
    (
        track(pennel_core::Speaker::Me),
        track(pennel_core::Speaker::Others),
    )
}

/// Decodes the system track first and the microphone track only when the core would split it, so
/// a long call never holds both tracks in memory.
pub fn recognize(shared: &Shared, id: &str, dir: &Path) -> Result<SpeakerRecognition, String> {
    let (microphone, system) = meeting_tracks(dir);
    if microphone.is_none() && system.is_none() {
        return Err("Deze meeting heeft geen bewaarde audio meer.".into());
    }
    let decode = |path: Option<PathBuf>| {
        path.map(|path| crate::platform_audio::decode_audio(&path))
            .transpose()
    };
    let system = decode(system)?;
    let microphone = if shared.engine.wants_microphone_track(system.as_deref()) {
        decode(microphone)?
    } else {
        None
    };
    shared
        .engine
        .recognize_speakers(id.to_owned(), microphone, system)
        .map_err(speaker_error)
}

#[derive(Clone, Serialize)]
struct EnrollmentProgress {
    seconds: u64,
    level: f32,
    stopped: bool,
}

enum Capture {
    Recording(Recorder),
    Captured(Vec<f32>),
}

struct Enrollment {
    token: u64,
    capture: Capture,
}

static ENROLLMENT: Mutex<Option<Enrollment>> = Mutex::new(None);
static TOKEN: AtomicU64 = AtomicU64::new(0);

fn capped(mut samples: Vec<f32>) -> Vec<f32> {
    samples.truncate(SAMPLE_RATE * ENROLLMENT_MAX_SECS as usize);
    samples
}

pub fn start_enrollment(app: &AppHandle, shared: &Arc<Shared>) -> Result<(), String> {
    if shared.recording.lock().unwrap().is_busy() {
        return Err("Wacht tot de opname is afgerond.".into());
    }
    if shared.dictation_active.load(Ordering::SeqCst) {
        return Err("Wacht tot het dictaat is afgerond.".into());
    }
    cancel_enrollment();
    let token = TOKEN.fetch_add(1, Ordering::SeqCst) + 1;
    let started = Instant::now();
    let last = Arc::new(Mutex::new(started - Duration::from_secs(1)));
    let events = app.clone();
    let recorder = Recorder::start(
        move |level| {
            let mut last = last.lock().unwrap();
            if last.elapsed() < Duration::from_millis(100) {
                return;
            }
            *last = Instant::now();
            let _ = events.emit(
                ENROLLMENT_EVENT,
                EnrollmentProgress {
                    seconds: started.elapsed().as_secs(),
                    level,
                    stopped: false,
                },
            );
        },
        |_| {},
    )
    .map_err(|error| match error {
        CaptureError::NoDevice => "Geen microfoon gevonden.".to_owned(),
        CaptureError::Failed(message) => format!("De microfoon kon niet starten: {message}"),
    })?;
    *ENROLLMENT.lock().unwrap() = Some(Enrollment {
        token,
        capture: Capture::Recording(recorder),
    });
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(ENROLLMENT_MAX_SECS));
        let mut slot = ENROLLMENT.lock().unwrap();
        let recording = slot
            .as_ref()
            .is_some_and(|e| e.token == token && matches!(e.capture, Capture::Recording(_)));
        if !recording {
            return;
        }
        if let Some(Enrollment {
            capture: Capture::Recording(recorder),
            ..
        }) = slot.take()
        {
            *slot = Some(Enrollment {
                token,
                capture: Capture::Captured(capped(recorder.stop())),
            });
        }
        drop(slot);
        let _ = app.emit(
            ENROLLMENT_EVENT,
            EnrollmentProgress {
                seconds: ENROLLMENT_MAX_SECS,
                level: 0.0,
                stopped: true,
            },
        );
    });
    Ok(())
}

fn take_samples() -> Option<Vec<f32>> {
    let enrollment = ENROLLMENT.lock().unwrap().take()?;
    Some(capped(match enrollment.capture {
        Capture::Recording(recorder) => recorder.stop(),
        Capture::Captured(samples) => samples,
    }))
}

pub fn finish_enrollment(shared: &Shared) -> Result<VoiceProfile, String> {
    let started = Instant::now();
    let samples = take_samples().ok_or("Er loopt geen opname van je stem.")?;
    let stopped = started.elapsed();
    let seconds = samples.len() / SAMPLE_RATE;
    let result = shared.engine.enroll_voice(samples).map_err(speaker_error);
    LOG.info(format!(
        "stem: {seconds} s opname, microfoon gestopt in {} ms, verwerkt in {} ms{}",
        stopped.as_millis(),
        (started.elapsed() - stopped).as_millis(),
        if result.is_ok() { "" } else { ", mislukt" }
    ));
    result
}

pub fn cancel_enrollment() {
    drop(take_samples());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_are_found_inside_the_meeting_folder_only() {
        let root = tempfile::tempdir().unwrap();
        let meeting = root.path().join("meeting");
        std::fs::create_dir(&meeting).unwrap();
        assert_eq!(meeting_tracks(&meeting), (None, None));

        std::fs::write(meeting.join("anderen.m4a"), b"audio").unwrap();
        let (microphone, system) = meeting_tracks(&meeting);
        assert_eq!(microphone, None);
        assert_eq!(
            system,
            Some(meeting.join("anderen.m4a").canonicalize().unwrap())
        );

        std::fs::write(meeting.join("ik.m4a"), b"audio").unwrap();
        assert!(meeting_tracks(&meeting).0.is_some());
    }

    #[test]
    fn grouped_tracks_reach_speaker_recognition() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("audio")).unwrap();
        std::fs::write(root.path().join("audio/room.m4a"), b"audio").unwrap();
        let (microphone, others) = meeting_tracks(root.path());
        assert!(microphone.is_none());
        assert_eq!(
            others,
            Some(root.path().join("audio/room.m4a").canonicalize().unwrap())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_track_linked_from_outside_the_meeting_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let meeting = root.path().join("meeting");
        std::fs::create_dir(&meeting).unwrap();
        let outside = root.path().join("elsewhere.m4a");
        std::fs::write(&outside, b"audio").unwrap();
        std::os::unix::fs::symlink(&outside, meeting.join("ik.m4a")).unwrap();
        std::fs::create_dir(meeting.join("anderen.m4a")).unwrap();
        assert_eq!(meeting_tracks(&meeting), (None, None));
    }

    #[test]
    fn enrollment_keeps_at_most_a_minute_of_audio() {
        let minute = SAMPLE_RATE * ENROLLMENT_MAX_SECS as usize;
        assert_eq!(capped(vec![0.0; minute + 5]).len(), minute);
        assert_eq!(capped(vec![0.0; 10]).len(), 10);
    }

    #[test]
    fn speaker_error_sources_remain_plain_text() {
        assert_eq!(
            speaker_error_source(PennelError::InvalidResponse {
                message: "Pennel hoorde te weinig spraak.".into()
            }),
            "Pennel hoorde te weinig spraak."
        );
        assert!(
            speaker_error_source(PennelError::ModelNotDownloaded).starts_with("Download eerst")
        );
        assert_eq!(
            speaker_error_source(PennelError::NotSignedIn),
            PennelError::NotSignedIn.to_string()
        );
    }
}
