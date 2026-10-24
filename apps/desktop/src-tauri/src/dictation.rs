//! Port of `DictationController.swift`: hotkey gestures drive the recorder, the core transcribes,
//! results are pasted in the order the recordings started.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use pennel_core::{CleanupStatus, DictationRequest, DictationResult, PennelError};
use tauri::{AppHandle, Emitter};

use crate::audio::{self, CaptureError, Recorder};
use crate::clipboard::{self, Clipboard, LateCopyWatch};
use crate::clipboard_system::SystemClipboard;
use crate::diagnostics::Log;
use crate::foreground::{self, ForegroundApp};
use crate::gesture::{GestureAction, HotkeyGesture};
use crate::hotkey::{self, HotkeyEvent};
use crate::keys::{self, Shortcut};
use crate::paste_queue::PasteQueue;
use crate::pill::Pill;
use crate::resample::TARGET_RATE;
use crate::state::Shared;
use crate::status::{CommandIntent, DictationStatus};

const LOG: Log = Log::new("dictation");
const NOTICE_DURATION: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_secs(1);

pub enum Msg {
    Hotkey(HotkeyEvent),
    Finished {
        seq: u64,
        outcome: Result<DictationResult, Failure>,
        released: Instant,
        is_command: bool,
    },
    TriggerChanged,
}

#[derive(Debug)]
pub enum Failure {
    Core(PennelError),
    AppChanged,
}

pub fn notice_for_cleanup(cleanup: &CleanupStatus) -> Option<String> {
    match cleanup {
        CleanupStatus::Failed { message }
            if message.to_lowercase().contains("timed out")
                || message.to_lowercase().contains("timeout") =>
        {
            Some("Opkuisen duurde te lang — ruwe tekst geplakt (zie logboek)".into())
        }
        CleanupStatus::Failed { .. } => {
            Some("Opkuisen mislukt — ruwe tekst geplakt (zie logboek)".into())
        }
        CleanupStatus::Rejected => {
            Some("Model antwoordde i.p.v. op te kuisen — ruwe tekst geplakt".into())
        }
        _ => None,
    }
}

pub fn notice_for_error(error: &PennelError) -> Option<String> {
    Some(
        match error {
            PennelError::NoSpeech => return None,
            PennelError::MissingApiKey { .. } => "Geen API-sleutel ingesteld",
            PennelError::ModelNotDownloaded => {
                "Model nog niet gedownload — bewaard in geschiedenis"
            }
            PennelError::Http { status, .. } if *status == 401 || *status == 403 => {
                "API-sleutel geweigerd — bewaard in geschiedenis"
            }
            PennelError::Network { .. } => "Geen verbinding — bewaard in geschiedenis",
            PennelError::Http { .. } | PennelError::InvalidResponse { .. } => {
                "Mislukt — bewaard in geschiedenis"
            }
            _ => "Mislukt",
        }
        .into(),
    )
}

/// Commands are not kept in history when they fail, so these notices don't promise a retry.
pub fn notice_for_command(failure: &Failure) -> Option<String> {
    let error = match failure {
        Failure::AppChanged => return Some("Andere app actief — opdracht niet geplakt".into()),
        Failure::Core(error) => error,
    };
    Some(
        match error {
            PennelError::NoSpeech => return None,
            PennelError::MissingApiKey { .. } => "Geen API-sleutel ingesteld",
            PennelError::NotFound => "Geen taalmodel ingesteld",
            PennelError::Http { status, .. } if *status == 401 || *status == 403 => {
                "API-sleutel geweigerd"
            }
            PennelError::InvalidResponse { message } if message == "selection too long" => {
                "Selectie te lang voor een opdracht"
            }
            PennelError::Network { .. } => "Geen verbinding",
            _ => "Opdracht mislukt",
        }
        .into(),
    )
}

fn describe(cleanup: &CleanupStatus) -> String {
    match cleanup {
        CleanupStatus::Applied => "toegepast".into(),
        CleanupStatus::Skipped => "overgeslagen (snelle modus)".into(),
        CleanupStatus::Rejected => "afgewezen (model antwoordde in plaats van op te kuisen)".into(),
        CleanupStatus::Failed { message } => format!("mislukt: {message}"),
    }
}

enum PasteJob {
    Paste { text: String, released: Instant },
    ReadSelection(Sender<Option<String>>),
}

/// Owns the clipboard; pastes and selection reads run strictly one after another so a command
/// never copies Pennel's own text that is still waiting to be restored.
fn paste_worker(jobs: Receiver<PasteJob>) {
    let mut clipboard = SystemClipboard::new();
    let mut late: Option<LateCopyWatch<<SystemClipboard as Clipboard>::Snapshot>> = None;
    let mut nonce = 0u64;
    loop {
        let job = if late.is_some() {
            match jobs.recv_timeout(Duration::from_millis(50)) {
                Ok(job) => Some(job),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match jobs.recv() {
                Ok(job) => Some(job),
                Err(_) => return,
            }
        };
        if let Some(watch) = &late
            && (watch.check(&mut clipboard) || watch.expired(Instant::now()))
        {
            late = None;
        }
        match job {
            None => {}
            Some(PasteJob::Paste { text, released }) => {
                late = None;
                keys::wait_for_release();
                let token = clipboard::begin(&mut clipboard, &text);
                if let Err(e) = keys::send(Shortcut::Paste) {
                    LOG.error(format!("plakken mislukt: {e}"));
                }
                std::thread::sleep(clipboard::RESTORE_DELAY);
                clipboard::end(&mut clipboard, token);
                LOG.info(format!(
                    "release→paste {} ms",
                    released.elapsed().as_millis()
                ));
            }
            Some(PasteJob::ReadSelection(reply)) => {
                keys::wait_for_release();
                nonce += 1;
                let read = clipboard::read_selection(
                    &mut clipboard,
                    || {
                        if let Err(e) = keys::send(Shortcut::Copy) {
                            LOG.error(format!("kopiëren mislukt: {e}"));
                        }
                    },
                    nonce,
                );
                late = read.late_watch;
                let _ = reply.send(read.text);
            }
        }
    }
}

struct Controller {
    app: AppHandle,
    shared: Arc<Shared>,
    pill: Arc<Pill>,
    tx: Sender<Msg>,
    paster: Sender<PasteJob>,
    gesture: HotkeyGesture,
    command: CommandIntent,
    status: DictationStatus,
    queue: PasteQueue<String>,
    recorder: Option<Recorder>,
    recording_app: Option<ForegroundApp>,
    notice_until: Option<Instant>,
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>, rx: Receiver<Msg>, tx: Sender<Msg>) {
    let (paster, jobs) = mpsc::channel();
    std::thread::Builder::new()
        .name("pennel-paste".into())
        .spawn(move || paste_worker(jobs))
        .expect("paste thread");
    let pill = Arc::new(Pill::new(app.clone()));
    let mut controller = Controller {
        app,
        shared,
        pill,
        tx,
        paster,
        gesture: HotkeyGesture::default(),
        command: CommandIntent::default(),
        status: DictationStatus::default(),
        queue: PasteQueue::default(),
        recorder: None,
        recording_app: None,
        notice_until: None,
    };
    std::thread::Builder::new()
        .name("pennel-dictation".into())
        .spawn(move || controller.run(rx))
        .expect("dictation thread");
}

impl Controller {
    fn run(&mut self, rx: Receiver<Msg>) {
        loop {
            let wait = self
                .next_wake()
                .map(|at| at.saturating_duration_since(Instant::now()));
            let msg = match wait {
                Some(wait) => match rx.recv_timeout(wait) {
                    Ok(msg) => Some(msg),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                },
                None => match rx.recv() {
                    Ok(msg) => Some(msg),
                    Err(_) => return,
                },
            };
            match msg {
                Some(Msg::Hotkey(event)) => self.hotkey(event),
                Some(Msg::Finished {
                    seq,
                    outcome,
                    released,
                    is_command,
                }) => self.finish(seq, outcome, released, is_command),
                Some(Msg::TriggerChanged) => self.gesture = HotkeyGesture::default(),
                None => {}
            }
            if self.recorder.is_some() {
                let actions = self.gesture.tick(hotkey::now());
                self.perform(actions);
            }
            if self.notice_until.is_some_and(|t| Instant::now() >= t) {
                self.notice_until = None;
                self.status.notice_expired();
                self.render();
            }
        }
    }

    fn next_wake(&self) -> Option<Instant> {
        let tick = self.recorder.is_some().then(|| Instant::now() + TICK);
        match (tick, self.notice_until) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn hotkey(&mut self, event: HotkeyEvent) {
        match event {
            HotkeyEvent::Edge(edge, time) => {
                let actions = self.gesture.handle(edge, time);
                self.perform(actions);
            }
            HotkeyEvent::Interrupt => {
                let actions = self.gesture.interrupt();
                self.perform(actions);
            }
            HotkeyEvent::Shift(down) => {
                let was = self.command.is_command();
                self.command
                    .shift(down, self.recorder.is_some() && hotkey::trigger_held());
                if self.command.is_command() && !was {
                    self.status.mark_command();
                    self.render();
                }
            }
        }
    }

    fn perform(&mut self, actions: Vec<GestureAction>) {
        for action in actions {
            match action {
                GestureAction::Start => self.begin(),
                GestureAction::Cancel => self.cancel(),
                GestureAction::Submit => self.submit(),
            }
        }
    }

    fn begin(&mut self) {
        if self.recorder.is_some() {
            return;
        }
        self.recording_app = foreground::current();
        let pill = self.pill.clone();
        match Recorder::start(move |level| pill.level(level)) {
            Ok(recorder) => self.recorder = Some(recorder),
            Err(CaptureError::NoDevice) => return self.notify("Geen microfoon gevonden"),
            Err(e) => {
                LOG.error(format!("opnemen starten mislukt: {e}"));
                return self.notify("Opnemen lukt niet");
            }
        }
        self.shared.set_meeting_dictating(true);
        self.status.recording_started(self.gesture.is_hands_free());
        self.command.reset();
        self.command.shift(hotkey::shift_held(), true);
        if self.command.is_command() {
            self.status.mark_command();
        }
        self.render();
    }

    fn stop_recording(&mut self) -> Vec<f32> {
        let Some(recorder) = self.recorder.take() else {
            return Vec::new();
        };
        self.shared.set_meeting_dictating(false);
        recorder.stop()
    }

    fn cancel(&mut self) {
        self.stop_recording();
        self.command.reset();
        self.status.recording_cancelled();
        self.render();
    }

    fn submit(&mut self) {
        let samples = self.stop_recording();
        if samples.is_empty() {
            self.status.recording_cancelled();
            return self.render();
        }
        if audio::is_digital_silence(&samples) {
            LOG.error("microfoon leverde enkel stilte (exacte nullen)");
            self.command.reset();
            self.status.recording_cancelled();
            return self.notify("Microfoon geeft geen geluid — kies een ander invoerapparaat");
        }
        let seq = self.queue.reserve();
        let request = DictationRequest {
            samples,
            sample_rate: TARGET_RATE,
            app_bundle_id: self
                .recording_app
                .as_ref()
                .map(|app| foreground::style_id(&app.executable)),
            keys: self.shared.keys(),
        };
        let released = Instant::now();
        let is_command = self.command.is_command();
        self.command.reset();
        self.status.recording_submitted();
        self.render();
        let engine = self.shared.engine.clone();
        let tx = self.tx.clone();
        if is_command {
            let pid = self.recording_app.as_ref().map(|a| a.pid);
            let paster = self.paster.clone();
            std::thread::spawn(move || {
                let still_there = || pid.is_none() || foreground::current().map(|a| a.pid) == pid;
                let outcome = if !still_there() {
                    Err(Failure::AppChanged)
                } else {
                    let (reply, selection) = mpsc::channel();
                    let selection = paster
                        .send(PasteJob::ReadSelection(reply))
                        .ok()
                        .and_then(|()| selection.recv().ok())
                        .flatten();
                    match engine.command(request, selection) {
                        Ok(_) if !still_there() => Err(Failure::AppChanged),
                        other => other.map_err(Failure::Core),
                    }
                };
                let _ = tx.send(Msg::Finished {
                    seq,
                    outcome,
                    released,
                    is_command: true,
                });
            });
            return;
        }
        std::thread::spawn(move || {
            let outcome = engine.dictate(request).map_err(Failure::Core);
            let _ = tx.send(Msg::Finished {
                seq,
                outcome,
                released,
                is_command: false,
            });
        });
    }

    fn finish(
        &mut self,
        seq: u64,
        outcome: Result<DictationResult, Failure>,
        released: Instant,
        is_command: bool,
    ) {
        let kind = if is_command { "opdracht" } else { "dictaat" };
        let elapsed = released.elapsed().as_millis();
        let (text, notice) = match outcome {
            Ok(result) => {
                let line = format!(
                    "{kind} klaar in {elapsed} ms: {} → {} tekens, opkuis {}",
                    result.raw.chars().count(),
                    result.text.chars().count(),
                    describe(&result.cleanup)
                );
                if matches!(result.cleanup, CleanupStatus::Failed { .. }) {
                    LOG.error(line);
                } else {
                    LOG.info(line);
                }
                (Some(result.text), notice_for_cleanup(&result.cleanup))
            }
            Err(failure) => {
                match &failure {
                    Failure::Core(PennelError::NoSpeech) => {
                        LOG.info(format!("{kind}: geen spraak herkend ({elapsed} ms)"));
                    }
                    other => LOG.error(format!("{kind} mislukt na {elapsed} ms: {other:?}")),
                }
                let notice = match &failure {
                    _ if is_command => notice_for_command(&failure),
                    Failure::Core(error) => notice_for_error(error),
                    Failure::AppChanged => notice_for_command(&failure),
                };
                (None, notice)
            }
        };
        if notice.is_some() {
            self.notice_until = Some(Instant::now() + NOTICE_DURATION);
        }
        self.status.dictation_finished(notice);
        self.render();
        let _ = self.app.emit("history-changed", ());
        for text in self.queue.fulfill(seq, text) {
            let _ = self.paster.send(PasteJob::Paste { text, released });
        }
    }

    fn notify(&mut self, message: &str) {
        self.status.show_notice(message);
        self.notice_until = Some(Instant::now() + NOTICE_DURATION);
        self.render();
    }

    fn render(&self) {
        self.pill.show(self.status.pill());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_match_the_mac_app() {
        assert_eq!(notice_for_error(&PennelError::NoSpeech), None);
        assert_eq!(
            notice_for_error(&PennelError::Http {
                status: 401,
                message: String::new()
            })
            .as_deref(),
            Some("API-sleutel geweigerd — bewaard in geschiedenis")
        );
        assert_eq!(
            notice_for_cleanup(&CleanupStatus::Failed {
                message: "request Timed Out".into()
            })
            .as_deref(),
            Some("Opkuisen duurde te lang — ruwe tekst geplakt (zie logboek)")
        );
        assert_eq!(notice_for_cleanup(&CleanupStatus::Applied), None);
        assert_eq!(
            notice_for_command(&Failure::AppChanged).as_deref(),
            Some("Andere app actief — opdracht niet geplakt")
        );
        assert_eq!(
            notice_for_command(&Failure::Core(PennelError::InvalidResponse {
                message: "selection too long".into()
            }))
            .as_deref(),
            Some("Selectie te lang voor een opdracht")
        );
    }
}
