//! Port of `DictationController.swift`: hotkey gestures drive the recorder, the core transcribes,
//! results are pasted in the order the recordings started.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use pennel_core::{CleanupStatus, DictationRequest, DictationResult, PennelError};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{self, CaptureError, Recorder};
#[cfg(not(target_os = "macos"))]
use crate::clipboard::{self, Clipboard, LateCopyWatch};
#[cfg(not(target_os = "macos"))]
use crate::clipboard_system::SystemClipboard;
use crate::diagnostics::Log;
use crate::foreground::{self, ForegroundApp};
use crate::gesture::{GestureAction, HotkeyGesture};
use crate::hotkey::{self, HotkeyEvent};
use crate::keys;
#[cfg(not(target_os = "macos"))]
use crate::keys::Shortcut;
use crate::paste_queue::PasteQueue;
use crate::pill::Pill;
use crate::resample::TARGET_RATE;
use crate::state::Shared;
use crate::status::{CommandIntent, DictationStatus};

const LOG: Log = Log::new("dictation");
const NOTICE_DURATION: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_secs(1);

pub enum Msg {
    PrepareShutdown(Sender<()>),
    Shutdown(Sender<()>),
    Hotkey(HotkeyEvent),
    ManualToggle {
        copy_only: bool,
    },
    DeliveryNotice(String),
    Finished {
        seq: u64,
        outcome: Result<DictationResult, Failure>,
        released: Instant,
        is_command: bool,
        target_pid: Option<u32>,
        copy_only: bool,
        pending: DictationJob,
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
        CleanupStatus::Failed { .. } => "mislukt".into(),
    }
}

fn failure_category(failure: &Failure) -> String {
    match failure {
        Failure::AppChanged => "foreground_changed".into(),
        Failure::Core(PennelError::Http { status, .. }) => format!("provider_http_{status}"),
        Failure::Core(PennelError::Network { .. }) => "provider_network".into(),
        Failure::Core(PennelError::InvalidResponse { .. }) => "provider_response".into(),
        Failure::Core(PennelError::Storage { .. }) => "storage".into(),
        Failure::Core(PennelError::MissingApiKey { .. }) => "missing_key".into(),
        Failure::Core(PennelError::ModelNotDownloaded) => "model_not_downloaded".into(),
        Failure::Core(PennelError::NotFound) => "not_found".into(),
        Failure::Core(PennelError::Cancelled) => "cancelled".into(),
        Failure::Core(PennelError::NoSpeech) => "no_speech".into(),
        Failure::Core(PennelError::NotSignedIn) => "not_signed_in".into(),
        Failure::Core(PennelError::Cloud { .. }) => "cloud".into(),
        Failure::Core(PennelError::VaultLocked) => "vault_locked".into(),
    }
}

struct CountedWork<T> {
    owner: T,
    counter: fn(&T) -> &AtomicUsize,
}

impl<T> CountedWork<T> {
    fn new(owner: T, counter: fn(&T) -> &AtomicUsize) -> Self {
        counter(&owner).fetch_add(1, Ordering::SeqCst);
        Self { owner, counter }
    }
}

impl<T> Drop for CountedWork<T> {
    fn drop(&mut self) {
        (self.counter)(&self.owner).fetch_sub(1, Ordering::SeqCst);
    }
}

pub struct DictationJob {
    _pending: CountedWork<Arc<Shared>>,
}

impl DictationJob {
    fn new(shared: Arc<Shared>) -> Self {
        Self {
            _pending: CountedWork::new(shared, |shared| &shared.dictation_jobs),
        }
    }
}

struct CaptureActivity(Arc<Shared>);

impl CaptureActivity {
    fn begin(shared: Arc<Shared>) -> Option<Self> {
        shared.dictation_active.store(true, Ordering::SeqCst);
        if shared.updating.load(Ordering::SeqCst) || shared.quitting.load(Ordering::SeqCst) {
            shared.dictation_active.store(false, Ordering::SeqCst);
            return None;
        }
        Some(Self(shared))
    }
}

impl Drop for CaptureActivity {
    fn drop(&mut self) {
        self.0.dictation_active.store(false, Ordering::SeqCst);
    }
}

struct PasteText {
    text: String,
    released: Instant,
    target_pid: Option<u32>,
    copy_only: bool,
}

struct Delivery {
    target_pid: Option<u32>,
    copy_only: bool,
}

fn target_matches(target_pid: Option<u32>, current_pid: Option<u32>) -> bool {
    target_pid.is_none() || target_pid == current_pid
}

fn paste_pending(shared: &Arc<Shared>) -> CountedWork<Arc<Shared>> {
    CountedWork::new(shared.clone(), |shared| &shared.paste_jobs)
}

fn copy_only_delivery(requested: bool) -> bool {
    #[cfg(target_os = "linux")]
    return requested || crate::linux_input::is_wayland();
    #[cfg(not(target_os = "linux"))]
    requested
}

fn delivery_notice(result: Result<(), String>) -> String {
    match result {
        Ok(()) => "Tekst gekopieerd — plak met Ctrl+V".into(),
        Err(error) => {
            LOG.error(format!("klembord schrijven mislukt: {error}"));
            "Kopiëren mislukt — tekst staat in Geschiedenis".into()
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn copy_persistent(text: &str, clipboard: &mut Option<arboard::Clipboard>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if crate::linux_input::is_wayland() {
        return crate::linux_input::copy_wayland(text);
    }
    if clipboard.is_none() {
        *clipboard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
    }
    clipboard
        .as_mut()
        .expect("initialized clipboard")
        .set_text(text)
        .map_err(|error| error.to_string())
}

enum PasteJob {
    Paste {
        result: PasteText,
        pending: CountedWork<Arc<Shared>>,
    },
    ReadSelection {
        reply: Sender<Option<String>>,
        target_pid: Option<u32>,
        pending: CountedWork<Arc<Shared>>,
    },
}

/// Owns the clipboard; pastes and selection reads run strictly one after another so a command
/// never copies Pennel's own text that is still waiting to be restored.
#[cfg(not(target_os = "macos"))]
type LateCopyWork = (
    LateCopyWatch<<SystemClipboard as Clipboard>::Snapshot>,
    CountedWork<Arc<Shared>>,
);

#[cfg(not(target_os = "macos"))]
fn paste_worker(jobs: Receiver<PasteJob>, shared: Arc<Shared>) {
    let mut persistent_clipboard = None;
    let mut clipboard = SystemClipboard::new();
    let mut late: Option<LateCopyWork> = None;
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
        if let Some((watch, _)) = &late
            && (watch.check(&mut clipboard) || watch.expired(Instant::now()))
        {
            late = None;
        }
        match job {
            None => {}
            Some(PasteJob::Paste { result, pending }) => {
                let _pending = pending;
                late = None;
                if copy_only_delivery(result.copy_only) {
                    let copied = copy_persistent(&result.text, &mut persistent_clipboard);
                    let notice = delivery_notice(copied);
                    let _ = shared.dictation.send(Msg::DeliveryNotice(notice));
                    continue;
                }
                keys::wait_for_release();
                if !target_matches(result.target_pid, foreground::current().map(|app| app.pid)) {
                    LOG.info("andere app actief: opdracht niet geplakt");
                    continue;
                }
                let token = clipboard::begin(&mut clipboard, &result.text);
                if let Err(e) = keys::send(Shortcut::Paste) {
                    LOG.error(format!("plakken mislukt: {e}"));
                }
                std::thread::sleep(clipboard::RESTORE_DELAY);
                clipboard::end(&mut clipboard, token);
                LOG.info(format!(
                    "release→paste {} ms",
                    result.released.elapsed().as_millis()
                ));
            }
            Some(PasteJob::ReadSelection {
                reply,
                target_pid,
                pending,
            }) => {
                #[cfg(target_os = "linux")]
                if crate::linux_input::is_wayland() {
                    let _ = reply.send(None);
                    continue;
                }
                keys::wait_for_release();
                if !target_matches(target_pid, foreground::current().map(|app| app.pid)) {
                    let _ = reply.send(None);
                    continue;
                }
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
                late = read.late_watch.map(|watch| (watch, pending));
                let _ = reply.send(read.text);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn paste_worker(jobs: Receiver<PasteJob>, shared: Arc<Shared>) {
    for job in jobs {
        keys::wait_for_release();
        match job {
            PasteJob::Paste { result, pending } => {
                let _pending = pending;
                if result.copy_only {
                    let notice = delivery_notice(crate::macos::copy_text(&result.text, None));
                    let _ = shared.dictation.send(Msg::DeliveryNotice(notice));
                    continue;
                }
                if !target_matches(result.target_pid, foreground::current().map(|app| app.pid)) {
                    LOG.info("andere app actief: opdracht niet geplakt");
                    continue;
                }
                if let Err(error) = crate::macos::paste(&result.text) {
                    LOG.error(format!("plakken mislukt: {error}"));
                }
                LOG.info(format!(
                    "release→paste {} ms",
                    result.released.elapsed().as_millis()
                ));
            }
            PasteJob::ReadSelection {
                reply,
                target_pid,
                pending,
            } => {
                let _pending = pending;
                if !target_matches(target_pid, foreground::current().map(|app| app.pid)) {
                    let _ = reply.send(None);
                    continue;
                }
                let _ = reply.send(crate::macos::read_selection().ok().flatten());
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
    queue: PasteQueue<PasteText>,
    recorder: Option<Recorder>,
    capture_activity: Option<CaptureActivity>,
    recording_app: Option<ForegroundApp>,
    recording_copy_only: bool,
    notice_until: Option<Instant>,
    pending_shutdown: Option<Sender<()>>,
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>, rx: Receiver<Msg>, tx: Sender<Msg>) {
    let (paster, jobs) = mpsc::channel();
    let paste_shared = shared.clone();
    std::thread::Builder::new()
        .name("pennel-paste".into())
        .spawn(move || paste_worker(jobs, paste_shared))
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
        capture_activity: None,
        recording_app: None,
        recording_copy_only: false,
        notice_until: None,
        pending_shutdown: None,
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
                Some(Msg::PrepareShutdown(reply)) => {
                    if self.recorder.is_some() {
                        self.submit();
                    }
                    self.gesture = HotkeyGesture::default();
                    let _ = reply.send(());
                }
                Some(Msg::Shutdown(reply)) => {
                    if self.recorder.is_some() {
                        self.submit();
                    }
                    self.pending_shutdown = Some(reply);
                }
                Some(Msg::Hotkey(event)) => self.hotkey(event),
                Some(Msg::ManualToggle { copy_only }) => {
                    if self.recorder.is_none() {
                        self.recording_copy_only = copy_only_delivery(copy_only);
                    }
                    let actions = self.gesture.manual_toggle(hotkey::now());
                    self.perform(actions);
                }
                Some(Msg::DeliveryNotice(notice)) => self.notify(&notice),
                Some(Msg::Finished {
                    seq,
                    outcome,
                    released,
                    is_command,
                    target_pid,
                    copy_only,
                    pending,
                }) => self.finish(
                    seq,
                    outcome,
                    released,
                    is_command,
                    Delivery {
                        target_pid,
                        copy_only,
                    },
                    pending,
                ),
                Some(Msg::TriggerChanged) if !self.gesture.is_manual() => {
                    if self.recorder.is_some() {
                        self.cancel();
                    }
                    self.gesture = HotkeyGesture::default();
                }
                Some(Msg::TriggerChanged) | None => {}
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
            if self.pending_shutdown.is_some()
                && self.shared.dictation_jobs.load(Ordering::SeqCst) == 0
                && self.shared.paste_jobs.load(Ordering::SeqCst) == 0
            {
                if let Some(reply) = self.pending_shutdown.take() {
                    let _ = reply.send(());
                }
                return;
            }
        }
    }

    fn next_wake(&self) -> Option<Instant> {
        let tick = if self.pending_shutdown.is_some() {
            Some(Instant::now() + Duration::from_millis(20))
        } else {
            self.recorder.is_some().then(|| Instant::now() + TICK)
        };
        match (tick, self.notice_until) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn hotkey(&mut self, event: HotkeyEvent) {
        match event {
            HotkeyEvent::MeetingShortcut => {
                if self.shared.settings().record_shortcut_enabled {
                    crate::call_detection::toggle_recording(self.app.clone(), self.shared.clone());
                }
            }
            HotkeyEvent::Edge(edge, time) => {
                let actions = self.gesture.handle(edge, time);
                if actions.contains(&GestureAction::Start) {
                    self.recording_copy_only = copy_only_delivery(false);
                }
                self.perform(actions);
            }
            HotkeyEvent::Interrupt => {
                let actions = self.gesture.interrupt();
                self.perform(actions);
            }
            HotkeyEvent::Shift(down) => {
                let was = self.command.is_command();
                self.command.shift(
                    down,
                    self.recorder.is_some()
                        && hotkey::trigger_held()
                        && !self.gesture.is_manual()
                        && !self.recording_copy_only,
                );
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
        if self.recorder.is_some()
            || self
                .app
                .state::<crate::ExitState>()
                .requested
                .load(Ordering::SeqCst)
        {
            return;
        }
        if crate::needs_setup(&self.shared) {
            self.gesture = HotkeyGesture::default();
            return self.notify("Kies eerst een spraakmodel en stel de provider in");
        }
        let Some(activity) = CaptureActivity::begin(self.shared.clone()) else {
            self.gesture = HotkeyGesture::default();
            return self.notify("Pennel wordt bijgewerkt — probeer daarna opnieuw");
        };
        self.recording_app = foreground::current();
        let pill = self.pill.clone();
        match Recorder::start(move |level| pill.level(level)) {
            Ok(recorder) => self.recorder = Some(recorder),
            Err(CaptureError::NoDevice) => {
                self.gesture = HotkeyGesture::default();
                return self.notify("Geen microfoon gevonden");
            }
            Err(e) => {
                LOG.error(format!("opnemen starten mislukt: {e}"));
                self.gesture = HotkeyGesture::default();
                return self.notify("Opnemen lukt niet");
            }
        }
        self.capture_activity = Some(activity);
        self.shared.set_meeting_dictating(true);
        self.status.recording_started(self.gesture.is_hands_free());
        self.command.reset();
        self.command.shift(
            hotkey::shift_held(),
            !self.gesture.is_manual() && !self.recording_copy_only,
        );
        if self.command.is_command() {
            self.status.mark_command();
        }
        self.render();
    }

    fn stop_recording(&mut self) -> Vec<f32> {
        let Some(recorder) = self.recorder.take() else {
            self.capture_activity.take();
            return Vec::new();
        };
        let samples = recorder.stop();
        self.shared.set_meeting_dictating(false);
        self.capture_activity.take();
        samples
    }

    fn cancel(&mut self) {
        self.stop_recording();
        self.command.reset();
        self.status.recording_cancelled();
        self.render();
    }

    fn submit(&mut self) {
        if self.recorder.is_none() {
            return;
        }
        // Transfer ownership before stopping capture, so the updater never sees an idle gap.
        let pending = DictationJob::new(self.shared.clone());
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
        let copy_only = self.recording_copy_only;
        let is_command = self.command.is_command() && !copy_only;
        self.command.reset();
        self.status.recording_submitted();
        self.render();
        let engine = self.shared.engine.clone();
        let tx = self.tx.clone();
        if is_command {
            let pid = self.recording_app.as_ref().map(|a| a.pid);
            let paster = self.paster.clone();
            let shared = self.shared.clone();
            std::thread::spawn(move || {
                let still_there = || target_matches(pid, foreground::current().map(|a| a.pid));
                let outcome = if !still_there() {
                    Err(Failure::AppChanged)
                } else {
                    let (reply, selection) = mpsc::channel();
                    let selection = paster
                        .send(PasteJob::ReadSelection {
                            reply,
                            target_pid: pid,
                            pending: paste_pending(&shared),
                        })
                        .ok()
                        .and_then(|()| selection.recv().ok())
                        .flatten();
                    if !still_there() {
                        Err(Failure::AppChanged)
                    } else {
                        match engine.command(request, selection) {
                            Ok(_) if !still_there() => Err(Failure::AppChanged),
                            other => other.map_err(Failure::Core),
                        }
                    }
                };
                let _ = tx.send(Msg::Finished {
                    seq,
                    outcome,
                    released,
                    is_command: true,
                    target_pid: pid,
                    copy_only,
                    pending,
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
                target_pid: None,
                copy_only,
                pending,
            });
        });
    }

    fn finish(
        &mut self,
        seq: u64,
        outcome: Result<DictationResult, Failure>,
        released: Instant,
        is_command: bool,
        delivery: Delivery,
        pending: DictationJob,
    ) {
        let Delivery {
            target_pid,
            copy_only,
        } = delivery;
        let _pending = pending;
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
                (
                    Some(PasteText {
                        text: result.text,
                        released,
                        target_pid,
                        copy_only,
                    }),
                    notice_for_cleanup(&result.cleanup).map(|notice| {
                        if copy_only {
                            notice.replace("geplakt", "beschikbaar")
                        } else {
                            notice
                        }
                    }),
                )
            }
            Err(failure) => {
                match &failure {
                    Failure::Core(PennelError::NoSpeech) => {
                        LOG.info(format!("{kind}: geen spraak herkend ({elapsed} ms)"));
                    }
                    other => LOG.error(format!(
                        "{kind} mislukt na {elapsed} ms: {}",
                        failure_category(other)
                    )),
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
        for result in self.queue.fulfill(seq, text) {
            let _ = self.paster.send(PasteJob::Paste {
                result,
                pending: paste_pending(&self.shared),
            });
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

impl Drop for Controller {
    fn drop(&mut self) {
        self.stop_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_details_never_enter_diagnostic_labels() {
        let private = "sk-test-echoed-private-wording";
        assert_eq!(
            describe(&CleanupStatus::Failed {
                message: private.into()
            }),
            "mislukt"
        );
        assert_eq!(
            failure_category(&Failure::Core(PennelError::Http {
                status: 400,
                message: private.into()
            })),
            "provider_http_400"
        );
        assert_eq!(
            failure_category(&Failure::Core(PennelError::InvalidResponse {
                message: private.into()
            })),
            "provider_response"
        );
    }

    #[test]
    fn quit_and_update_barriers_release_the_capture_reservation() {
        let root = tempfile::tempdir().unwrap();
        let shared = Shared::testing(root.path().join("preview"));
        shared.quitting.store(true, Ordering::SeqCst);
        assert!(CaptureActivity::begin(shared.clone()).is_none());
        assert!(!shared.dictation_active.load(Ordering::SeqCst));
        shared.quitting.store(false, Ordering::SeqCst);
        shared.updating.store(true, Ordering::SeqCst);
        assert!(CaptureActivity::begin(shared.clone()).is_none());
        assert!(!shared.dictation_active.load(Ordering::SeqCst));
        shared.updating.store(false, Ordering::SeqCst);
        let active = CaptureActivity::begin(shared.clone()).unwrap();
        assert!(shared.dictation_active.load(Ordering::SeqCst));
        drop(active);
        assert!(!shared.dictation_active.load(Ordering::SeqCst));
    }

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

    #[test]
    fn copy_confirmation_requires_a_successful_clipboard_write() {
        assert_eq!(
            delivery_notice(Ok(())),
            "Tekst gekopieerd — plak met Ctrl+V"
        );
        assert_eq!(
            delivery_notice(Err("unavailable".into())),
            "Kopiëren mislukt — tekst staat in Geschiedenis"
        );
    }

    #[test]
    fn queued_commands_require_the_original_foreground_process() {
        assert!(target_matches(Some(42), Some(42)));
        assert!(!target_matches(Some(42), Some(9)));
        assert!(!target_matches(Some(42), None));
        assert!(target_matches(None, Some(9)));
    }

    #[test]
    fn ordered_paste_results_keep_their_own_target_and_release_time() {
        let mut queue = PasteQueue::default();
        let first = queue.reserve();
        let second = queue.reserve();
        let released = Instant::now();
        assert!(
            queue
                .fulfill(
                    second,
                    Some(PasteText {
                        text: "second".into(),
                        released,
                        target_pid: Some(42),
                        copy_only: true
                    })
                )
                .is_empty()
        );
        let ready = queue.fulfill(
            first,
            Some(PasteText {
                text: "first".into(),
                released: released - Duration::from_secs(1),
                target_pid: None,
                copy_only: false,
            }),
        );
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].target_pid, None);
        assert_eq!(ready[1].target_pid, Some(42));
        assert_eq!(ready[1].released, released);
        assert!(!ready[0].copy_only);
        assert!(ready[1].copy_only);
        assert!(!target_matches(ready[1].target_pid, Some(9)));
    }

    #[test]
    fn transcription_to_ordered_paste_transfer_has_no_idle_gap() {
        let dictations = Arc::new(AtomicUsize::new(0));
        let pastes = Arc::new(AtomicUsize::new(0));
        let first_job = CountedWork::new(dictations.clone(), |count| count);
        let second_job = CountedWork::new(dictations.clone(), |count| count);
        let mut queue = PasteQueue::default();
        let first = queue.reserve();
        let second = queue.reserve();
        assert!(queue.fulfill(second, Some("second")).is_empty());
        drop(second_job);
        assert_eq!(dictations.load(Ordering::SeqCst), 1);
        let queued: Vec<_> = queue
            .fulfill(first, Some("first"))
            .into_iter()
            .map(|text| (text, CountedWork::new(pastes.clone(), |count| count)))
            .collect();
        assert_eq!(dictations.load(Ordering::SeqCst), 1);
        assert_eq!(pastes.load(Ordering::SeqCst), 2);
        drop(first_job);
        assert_eq!(dictations.load(Ordering::SeqCst), 0);
        assert_eq!(pastes.load(Ordering::SeqCst), 2);
        drop(queued);
        assert_eq!(pastes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn disconnected_queue_releases_pending_operation_count() {
        let count = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let failed = sender.send(CountedWork::new(count.clone(), |count| count));
        assert!(failed.is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        drop(failed);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn closing_queue_releases_all_waiting_operations() {
        let count = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = mpsc::channel();
        assert!(
            sender
                .send(CountedWork::new(count.clone(), |count| count))
                .is_ok()
        );
        assert!(
            sender
                .send(CountedWork::new(count.clone(), |count| count))
                .is_ok()
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
        drop(receiver);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
