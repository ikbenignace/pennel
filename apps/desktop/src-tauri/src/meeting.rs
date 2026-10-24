//! Records a meeting: the microphone as "Ik" and what the computer plays as "Anderen", each pushed
//! to the core's `MeetingSession` in ~1 s batches. The session transcribes in the background.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use pennel_core::{MeetingSession, Speaker};

use crate::audio::{Capture, Source};
use crate::diagnostics::Log;

const LOG: Log = Log::new("meeting");
const BATCH: usize = 16_000;

pub struct MeetingRecorder {
    pub session: Arc<MeetingSession>,
    pub title: String,
    started: Instant,
    mic: Option<Capture>,
    system: Option<Capture>,
    batches: [Arc<Mutex<Vec<f32>>>; 2],
    dictating_since: Mutex<Option<u64>>,
    heard_system: Arc<AtomicBool>,
}

fn batching(
    session: Arc<MeetingSession>,
    speaker: Speaker,
    pending: Arc<Mutex<Vec<f32>>>,
    heard: Option<Arc<AtomicBool>>,
) -> impl FnMut(&[f32]) + Send + 'static {
    move |chunk| {
        if let Some(heard) = &heard
            && chunk.iter().any(|s| *s != 0.0)
        {
            heard.store(true, Ordering::Relaxed);
        }
        let mut pending = pending.lock().unwrap();
        pending.extend_from_slice(chunk);
        if pending.len() >= BATCH {
            session.push_audio(speaker, std::mem::take(&mut *pending));
        }
    }
}

impl MeetingRecorder {
    pub fn start(session: Arc<MeetingSession>, title: String) -> Result<Self, String> {
        let batches = [
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(Vec::new())),
        ];
        let heard_system = Arc::new(AtomicBool::new(false));
        let mic = Capture::start(
            Source::Microphone,
            batching(session.clone(), Speaker::Me, batches[0].clone(), None),
        )
        .map_err(|e| format!("microfoon: {e}"))?;
        let system = match Capture::start(
            Source::SystemOutput,
            batching(
                session.clone(),
                Speaker::Others,
                batches[1].clone(),
                Some(heard_system.clone()),
            ),
        ) {
            Ok(capture) => Some(capture),
            Err(e) => {
                LOG.error(format!("systeemgeluid niet beschikbaar: {e}"));
                None
            }
        };
        LOG.info(format!(
            "meeting gestart, systeemgeluid {}",
            if system.is_some() { "aan" } else { "uit" }
        ));
        Ok(Self {
            session,
            title,
            started: Instant::now(),
            mic: Some(mic),
            system,
            batches,
            dictating_since: Mutex::new(None),
            heard_system,
        })
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn has_system_audio(&self) -> bool {
        self.system.is_some()
    }

    pub fn heard_system_audio(&self) -> bool {
        self.heard_system.load(Ordering::Relaxed)
    }

    /// While the user dictates into another app their microphone is silenced in the meeting,
    /// and the period is marked so echoes of it are filtered from the transcript.
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

    /// Stops capturing and waits until every chunk is transcribed; can take a while.
    pub fn stop(mut self) -> Result<String, String> {
        if let Some(mic) = self.mic.take() {
            mic.stop();
        }
        if let Some(system) = self.system.take() {
            system.stop();
        }
        for (speaker, batch) in [Speaker::Me, Speaker::Others]
            .into_iter()
            .zip(&self.batches)
        {
            let rest = std::mem::take(&mut *batch.lock().unwrap());
            if !rest.is_empty() {
                self.session.push_audio(speaker, rest);
            }
        }
        self.session.stop().map_err(|e| e.to_string())?;
        LOG.info(format!(
            "meeting gestopt na {} s",
            self.started.elapsed().as_secs()
        ));
        Ok(self.session.id())
    }
}
