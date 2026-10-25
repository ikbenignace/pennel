//! Port of `DictationController.swift`: hotkey gestures drive the recorder, the core transcribes,
//! results are pasted in the order the recordings started.

use std::collections::HashMap;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use pennel_core::DictationPreviewMode;
use pennel_core::{
    CleanupStatus, DictationPreviewObserver, DictationPreviewUpdate, DictationRequest,
    DictationResult, LiveDictationSession, PennelError,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{self, CaptureError, Recorder};
#[cfg(not(target_os = "macos"))]
use crate::clipboard::{self, Clipboard, LateCopyWatch};
#[cfg(not(target_os = "macos"))]
use crate::clipboard_system::SystemClipboard;
use crate::diagnostics::Log;
use crate::external_paste::{self, ExternalTarget};
use crate::foreground::{self, ForegroundApp};
use crate::gesture::{GestureAction, HotkeyGesture};
use crate::hotkey::{self, HotkeyEvent};
use crate::keys;
#[cfg(not(target_os = "macos"))]
use crate::keys::Shortcut;
use crate::owned_dictation::{self, OwnedCapture, SessionKind, SessionPhase};
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
    OwnedToggle(String),
    OwnedCancel(String),
    OwnedClosed(String),
    OwnedDeliveryError(OwnedCapture, String),
    RefreshOwned,
    DeliveryNotice(String),
    Preview {
        update: DictationPreviewUpdate,
        owned: Option<OwnedCapture>,
    },
    Finished {
        seq: u64,
        outcome: Box<Result<DictationResult, Failure>>,
        released: Instant,
        is_command: bool,
        destination: ExternalDestination,
        preview_id: String,
        pending: DictationJob,
        owned: Option<OwnedCapture>,
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
        if shared
            .dictation_active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
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

struct PasteText<T = ExternalTarget> {
    text: String,
    released: Instant,
    destination: ExternalDestination<T>,
}

struct Delivery {
    destination: ExternalDestination,
    preview_id: String,
    owned: Option<OwnedCapture>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExternalDestination<T = ExternalTarget> {
    CopyOnly,
    Captured(T),
    Unavailable(String),
}

fn capture_destination<T>(
    copy_only: bool,
    capture: impl FnOnce() -> Result<T, String>,
) -> ExternalDestination<T> {
    if copy_only {
        ExternalDestination::CopyOnly
    } else {
        match capture() {
            Ok(target) => ExternalDestination::Captured(target),
            Err(error) => ExternalDestination::Unavailable(error),
        }
    }
}

fn cancel_destination(destination: &ExternalDestination) {
    if let ExternalDestination::Captured(target) = destination {
        target.cancel();
    }
}

#[derive(Default)]
struct PreviewCursor {
    session_id: Option<String>,
    owner_id: Option<String>,
    revision: u64,
}

impl PreviewCursor {
    fn begin(&mut self, session_id: String, owner_id: Option<String>) {
        self.session_id = Some(session_id);
        self.owner_id = owner_id;
        self.revision = 0;
    }

    fn accept(&mut self, update: &DictationPreviewUpdate, owner_valid: bool) -> bool {
        if !owner_valid
            || self.session_id.as_deref() != Some(update.session_id.as_str())
            || update.revision <= self.revision
        {
            return false;
        }
        self.revision = update.revision;
        true
    }

    fn clear_session(&mut self, id: &str) -> bool {
        if self.session_id.as_deref() != Some(id) {
            return false;
        }
        *self = Self::default();
        true
    }

    fn clear_owner(&mut self, id: &str) -> Option<String> {
        if self.owner_id.as_deref() != Some(id) {
            return None;
        }
        let session_id = self.session_id.take();
        *self = Self::default();
        session_id
    }
}

struct PendingOwnedSession {
    owner_id: String,
    session: Arc<LiveDictationSession>,
}

#[derive(Default)]
struct PendingOwnedSessions(HashMap<String, PendingOwnedSession>);

impl PendingOwnedSessions {
    fn track(&mut self, owner_id: &str, session: Arc<LiveDictationSession>) {
        self.0.insert(
            session.id(),
            PendingOwnedSession {
                owner_id: owner_id.into(),
                session,
            },
        );
    }

    fn cancel_owner(&mut self, owner_id: &str) {
        self.0.retain(|_, pending| {
            if pending.owner_id == owner_id {
                pending.session.cancel();
                false
            } else {
                true
            }
        });
    }

    fn complete(&mut self, session_id: &str) {
        self.0.remove(session_id);
    }
}

impl Drop for PendingOwnedSessions {
    fn drop(&mut self) {
        for pending in self.0.values() {
            pending.session.cancel();
        }
    }
}

struct StoppedContext {
    command_pid: Option<u32>,
    app_bundle_id: Option<String>,
}

impl StoppedContext {
    fn new(
        app: Option<&ForegroundApp>,
        owned_app_id: Option<&str>,
        captured_app_id: Option<&str>,
    ) -> Self {
        Self {
            command_pid: if owned_app_id.is_none() {
                app.map(|app| app.pid)
            } else {
                None
            },
            app_bundle_id: owned_app_id.or(captured_app_id).map(str::to_owned),
        }
    }
}

struct PreviewObserver {
    sender: Sender<Msg>,
    owned: Option<OwnedCapture>,
}

impl DictationPreviewObserver for PreviewObserver {
    fn changed(&self, update: DictationPreviewUpdate) {
        let _ = self.sender.send(Msg::Preview {
            update,
            owned: self.owned.clone(),
        });
    }
}

enum CompletedDelivery {
    External(PasteText),
    Owned { capture: OwnedCapture, text: String },
}

fn target_matches(target_pid: Option<u32>, current_pid: Option<u32>) -> bool {
    target_pid.is_none() || target_pid == current_pid
}

fn command_field_matches(
    target_pid: Option<u32>,
    current_pid: Option<u32>,
    captured_field_current: bool,
) -> bool {
    captured_field_current && target_matches(target_pid, current_pid)
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

enum DeliveryFailure {
    Missing,
    Changed,
    Permission,
    Other,
}

fn unavailable_destination(shared: &Shared, failure: DeliveryFailure) {
    let notice = match failure {
        DeliveryFailure::Missing => "Geen actief tekstdoel — je tekst staat in Geschiedenis.",
        DeliveryFailure::Changed => {
            "Niet geplakt — het tekstdoel is gewijzigd. Je tekst staat in Geschiedenis."
        }
        DeliveryFailure::Permission => {
            "Geen toegang om te plakken. Je tekst staat in Geschiedenis."
        }
        DeliveryFailure::Other => "Niet geplakt. Je tekst staat in Geschiedenis.",
    };
    let _ = shared.dictation.send(Msg::DeliveryNotice(notice.into()));
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
        target: ExternalTarget,
        pending: CountedWork<Arc<Shared>>,
    },
    Owned {
        capture: OwnedCapture,
        text: String,
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
                if matches!(result.destination, ExternalDestination::CopyOnly) {
                    let copied = copy_persistent(&result.text, &mut persistent_clipboard);
                    let notice = delivery_notice(copied);
                    let _ = shared.dictation.send(Msg::DeliveryNotice(notice));
                    continue;
                }
                let ExternalDestination::Captured(target) = &result.destination else {
                    unavailable_destination(&shared, DeliveryFailure::Missing);
                    continue;
                };
                keys::wait_for_release();
                if !external_paste::availability().0 {
                    target.cancel();
                    unavailable_destination(&shared, DeliveryFailure::Permission);
                    continue;
                }
                if external_paste::prepare(target).is_err()
                    || !external_paste::still_selected(target)
                {
                    target.cancel();
                    unavailable_destination(&shared, DeliveryFailure::Changed);
                    continue;
                }
                let token = clipboard::begin(&mut clipboard, &result.text);
                let pasted =
                    external_paste::still_selected(target) && keys::send(Shortcut::Paste).is_ok();
                std::thread::sleep(clipboard::RESTORE_DELAY);
                clipboard::end(&mut clipboard, token);
                target.cancel();
                if pasted {
                    LOG.info(format!(
                        "release→paste {} ms",
                        result.released.elapsed().as_millis()
                    ));
                } else {
                    unavailable_destination(&shared, DeliveryFailure::Other);
                }
            }
            Some(PasteJob::ReadSelection {
                reply,
                target_pid,
                target,
                pending,
            }) => {
                #[cfg(target_os = "linux")]
                if crate::linux_input::is_wayland() {
                    let _ = reply.send(None);
                    continue;
                }
                keys::wait_for_release();
                if !command_field_matches(
                    target_pid,
                    foreground::current().map(|app| app.pid),
                    external_paste::is_current(&target),
                ) {
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
            Some(PasteJob::Owned {
                capture,
                text,
                pending,
            }) => {
                let _pending = pending;
                late = None;
                if !owned_dictation::external_allowed(&shared, &capture) {
                    continue;
                }
                let Some(target) = &capture.external else {
                    continue;
                };
                let result = crate::external_paste::prepare(target).and_then(|()| {
                    keys::wait_for_release();
                    if !owned_dictation::external_allowed(&shared, &capture)
                        || !crate::external_paste::still_selected(target)
                    {
                        return Err(
                            "Extern plakken is geannuleerd. Je tekst staat in Pennel.".into()
                        );
                    }
                    let token = clipboard::begin(&mut clipboard, &text);
                    let result = if crate::external_paste::still_selected(target) {
                        keys::send(Shortcut::Paste)
                    } else {
                        Err("De doelapp is gewijzigd. Je tekst staat in Pennel.".into())
                    };
                    std::thread::sleep(clipboard::RESTORE_DELAY);
                    clipboard::end(&mut clipboard, token);
                    result
                });
                if let Err(error) = result {
                    let _ = shared
                        .dictation
                        .send(Msg::OwnedDeliveryError(capture, error));
                }
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
                if matches!(result.destination, ExternalDestination::CopyOnly) {
                    let notice = delivery_notice(crate::macos::copy_text(&result.text, None));
                    let _ = shared.dictation.send(Msg::DeliveryNotice(notice));
                    continue;
                }
                let ExternalDestination::Captured(target) = &result.destination else {
                    unavailable_destination(&shared, DeliveryFailure::Missing);
                    continue;
                };
                if !external_paste::availability().0 {
                    target.cancel();
                    unavailable_destination(&shared, DeliveryFailure::Permission);
                    continue;
                }
                if !external_paste::is_current(target) {
                    target.cancel();
                    unavailable_destination(&shared, DeliveryFailure::Changed);
                    continue;
                }
                let pasted = external_paste::paste(&result.text, target).is_ok();
                target.cancel();
                if pasted {
                    LOG.info(format!(
                        "release→paste {} ms",
                        result.released.elapsed().as_millis()
                    ));
                } else {
                    unavailable_destination(&shared, DeliveryFailure::Other);
                }
            }
            PasteJob::ReadSelection {
                reply,
                target_pid,
                target,
                pending,
            } => {
                let _pending = pending;
                if !command_field_matches(
                    target_pid,
                    foreground::current().map(|app| app.pid),
                    external_paste::is_current(&target),
                ) {
                    let _ = reply.send(None);
                    continue;
                }
                let selection = crate::macos::read_selection().ok().flatten();
                let _ = reply.send(
                    external_paste::is_current(&target)
                        .then_some(selection)
                        .flatten(),
                );
            }
            PasteJob::Owned {
                capture,
                text,
                pending,
            } => {
                let _pending = pending;
                if !owned_dictation::external_allowed(&shared, &capture) {
                    continue;
                }
                let Some(target) = &capture.external else {
                    continue;
                };
                if let Err(error) = crate::external_paste::paste(&text, target) {
                    let _ = shared
                        .dictation
                        .send(Msg::OwnedDeliveryError(capture, error));
                }
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
    queue: PasteQueue<CompletedDelivery>,
    recorder: Option<Recorder>,
    recording_session: Option<Arc<LiveDictationSession>>,
    pending_owned: PendingOwnedSessions,
    preview_cursor: PreviewCursor,
    #[cfg(target_os = "macos")]
    apple_preview: Option<Arc<Mutex<Option<crate::macos::AppleSpeechPreview>>>>,
    capture_activity: Option<CaptureActivity>,
    recording_copy_only: bool,
    requested_owned: Option<String>,
    recording_owned: Option<OwnedCapture>,
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
        recording_session: None,
        pending_owned: PendingOwnedSessions::default(),
        preview_cursor: PreviewCursor::default(),
        #[cfg(target_os = "macos")]
        apple_preview: None,
        capture_activity: None,
        recording_copy_only: false,
        requested_owned: None,
        recording_owned: None,
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
                        self.requested_owned = owned_dictation::focused_session(&self.app);
                    }
                    let actions = self.gesture.manual_toggle(hotkey::now());
                    self.perform(actions);
                }
                Some(Msg::OwnedToggle(id)) => {
                    if self.recorder.is_some()
                        && self
                            .recording_owned
                            .as_ref()
                            .is_none_or(|owned| owned.id != id)
                    {
                        owned_dictation::failed(
                            &self.app,
                            &id,
                            "Rond eerst je huidige dictaat af".into(),
                        );
                    } else {
                        if self.recorder.is_none() {
                            self.requested_owned = Some(id);
                            self.recording_copy_only = false;
                        }
                        let actions = self.gesture.manual_toggle(hotkey::now());
                        self.perform(actions);
                    }
                }
                Some(Msg::OwnedCancel(id) | Msg::OwnedClosed(id)) => {
                    self.pending_owned.cancel_owner(&id);
                    if self
                        .recording_owned
                        .as_ref()
                        .is_some_and(|owned| owned.id == id)
                    {
                        self.cancel();
                        self.gesture = HotkeyGesture::default();
                    }
                    self.clear_owned_preview(&id);
                }
                Some(Msg::DeliveryNotice(notice)) => self.notify(&notice),
                Some(Msg::Preview { update, owned }) => {
                    let owner_valid = owned.as_ref().is_none_or(OwnedCapture::valid);
                    if self.preview_cursor.accept(&update, owner_valid) {
                        self.pill.update_preview(update);
                    } else if !owner_valid {
                        self.clear_preview(&update.session_id);
                    }
                }
                Some(Msg::OwnedDeliveryError(capture, error)) => {
                    owned_dictation::delivery_error(&self.app, &capture, error)
                }
                Some(Msg::RefreshOwned) => {
                    owned_dictation::refresh_idle_pads(&self.app, &self.shared)
                }
                Some(Msg::Finished {
                    seq,
                    outcome,
                    released,
                    is_command,
                    destination,
                    preview_id,
                    pending,
                    owned,
                }) => self.finish(
                    seq,
                    *outcome,
                    released,
                    is_command,
                    Delivery {
                        destination,
                        preview_id,
                        owned,
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
                    self.requested_owned = owned_dictation::focused_session(&self.app);
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
                        && !self.recording_copy_only
                        && self.recording_owned.is_none(),
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
        let route = owned_dictation::CaptureRoute::resolve(
            owned_dictation::own_app_foreground(&self.app),
            self.requested_owned.take(),
        );
        let allows_command = route.allows_command();
        if route == owned_dictation::CaptureRoute::Unavailable {
            self.gesture = HotkeyGesture::default();
            self.command.reset();
            return self.notify("Kies eerst een tekstveld in een andere app");
        }
        if let owned_dictation::CaptureRoute::Owned(id) = route {
            match owned_dictation::begin_capture(&self.app, &self.shared, &id) {
                Ok(capture) => self.recording_owned = Some(capture),
                Err(error) => {
                    self.gesture = HotkeyGesture::default();
                    owned_dictation::failed(&self.app, &id, error);
                    return;
                }
            }
        } else {
            let readiness = owned_dictation::readiness(&self.shared, "dictation");
            if !readiness.can_record {
                self.gesture = HotkeyGesture::default();
                return self.notify(
                    readiness
                        .reason
                        .as_deref()
                        .unwrap_or("Stel eerst spraakherkenning in"),
                );
            }
        }
        let Some(activity) = CaptureActivity::begin(self.shared.clone()) else {
            self.gesture = HotkeyGesture::default();
            if let Some(capture) = self.recording_owned.take() {
                owned_dictation::failed(
                    &self.app,
                    &capture.id,
                    "Pennel is nog bezig. Probeer daarna opnieuw.".into(),
                );
            }
            return self.notify("Pennel wordt bijgewerkt — probeer daarna opnieuw");
        };
        let session = self.shared.engine.clone().begin_dictation_session(
            Arc::new(PreviewObserver {
                sender: self.tx.clone(),
                owned: self.recording_owned.clone(),
            }),
            self.recording_owned
                .as_ref()
                .is_some_and(|capture| capture.kind == SessionKind::Practice),
        );
        let preview_id = session.id();
        self.recording_session = Some(session.clone());
        self.preview_cursor.begin(
            preview_id.clone(),
            self.recording_owned
                .as_ref()
                .map(|capture| capture.id.clone()),
        );
        self.status.recording_started(self.gesture.is_hands_free());
        self.command.reset();
        self.command.shift(
            hotkey::shift_held(),
            allows_command
                && !self.gesture.is_manual()
                && !self.recording_copy_only
                && self.recording_owned.is_none(),
        );
        if self.command.is_command() {
            self.status.mark_command();
        }
        self.render();
        self.pill.begin_preview(preview_id);

        #[cfg(target_os = "macos")]
        let apple_preview = {
            let service = if session.mode() == DictationPreviewMode::System {
                let session = session.clone();
                crate::macos::AppleSpeechPreview::start(
                    session.language().as_deref(),
                    move |text| {
                        session.update_system_preview(text);
                    },
                )
                .ok()
            } else {
                None
            };
            let preview = Arc::new(Mutex::new(service));
            self.apple_preview = Some(preview.clone());
            preview
        };
        let pill = self.pill.clone();
        match Recorder::start(
            move |level| pill.level(level),
            move |samples| {
                session.feed_audio(samples.to_vec());
                #[cfg(target_os = "macos")]
                if let Some(preview) = apple_preview.lock().unwrap().as_ref() {
                    preview.feed(samples);
                }
            },
        ) {
            Ok(recorder) => self.recorder = Some(recorder),
            Err(CaptureError::NoDevice) => {
                self.discard_recording_preview();
                self.status.recording_cancelled();
                self.gesture = HotkeyGesture::default();
                if let Some(capture) = self.recording_owned.take() {
                    owned_dictation::failed(
                        &self.app,
                        &capture.id,
                        "Geen microfoon gevonden".into(),
                    );
                }
                return self.notify("Geen microfoon gevonden");
            }
            Err(e) => {
                self.discard_recording_preview();
                self.status.recording_cancelled();
                LOG.error(format!("opnemen starten mislukt: {e}"));
                self.gesture = HotkeyGesture::default();
                if let Some(capture) = self.recording_owned.take() {
                    owned_dictation::failed(
                        &self.app,
                        &capture.id,
                        "Opnemen lukt niet. Controleer de microfoontoegang.".into(),
                    );
                }
                return self.notify("Opnemen lukt niet");
            }
        }
        self.capture_activity = Some(activity);
        self.shared.set_meeting_dictating(true);
    }

    fn stop_recording(&mut self) -> Vec<f32> {
        let samples = self.recorder.take().map(Recorder::stop).unwrap_or_default();
        #[cfg(target_os = "macos")]
        if let Some(preview) = self.apple_preview.take()
            && let Some(preview) = preview.lock().unwrap().take()
        {
            preview.stop();
        }
        if let Some(session) = &self.recording_session {
            session.stop_preview();
        }
        self.shared.set_meeting_dictating(false);
        self.capture_activity.take();
        samples
    }

    fn cancel(&mut self) {
        self.stop_recording();
        self.discard_recording_preview();
        if let Some(capture) = self.recording_owned.take() {
            owned_dictation::cancelled(&self.app, &capture);
        }
        self.requested_owned = None;
        self.command.reset();
        self.status.recording_cancelled();
        self.render();
    }

    fn discard_recording_preview(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(preview) = self.apple_preview.take()
            && let Some(preview) = preview.lock().unwrap().take()
        {
            preview.stop();
        }
        if let Some(session) = self.recording_session.take() {
            session.cancel();
            self.clear_preview(&session.id());
        }
    }

    fn clear_preview(&mut self, id: &str) {
        if self.preview_cursor.clear_session(id) {
            self.pill.clear_preview(Some(id));
        }
    }

    fn clear_owned_preview(&mut self, owner_id: &str) {
        if let Some(id) = self.preview_cursor.clear_owner(owner_id) {
            self.pill.clear_preview(Some(&id));
        }
    }

    fn submit(&mut self) {
        if self.recorder.is_none() {
            return;
        }
        let released = Instant::now();
        // Transfer ownership before stopping capture, so the updater never sees an idle gap.
        let pending = DictationJob::new(self.shared.clone());
        let owned = self.recording_owned.take();
        let stopping_app = foreground::current();
        let destination = if owned.is_some() {
            ExternalDestination::CopyOnly
        } else {
            capture_destination(self.recording_copy_only, || {
                external_paste::capture_current(&self.app)
            })
        };
        let samples = self.stop_recording();
        if samples.is_empty() {
            cancel_destination(&destination);
            self.discard_recording_preview();
            if let Some(capture) = &owned {
                owned_dictation::failed(
                    &self.app,
                    &capture.id,
                    "Geen spraak opgenomen. Probeer opnieuw.".into(),
                );
            }
            self.status.recording_cancelled();
            return self.render();
        }
        if audio::is_digital_silence(&samples) {
            cancel_destination(&destination);
            self.discard_recording_preview();
            LOG.error("microfoon leverde enkel stilte (exacte nullen)");
            self.command.reset();
            self.status.recording_cancelled();
            if let Some(capture) = &owned {
                owned_dictation::failed(
                    &self.app,
                    &capture.id,
                    "Je microfoon geeft geen geluid. Controleer je invoerapparaat.".into(),
                );
            }
            return self.notify("Microfoon geeft geen geluid — kies een ander invoerapparaat");
        }
        let session = self
            .recording_session
            .take()
            .expect("a recording owns its recognition session");
        if let Some(capture) = &owned {
            self.pending_owned.track(&capture.id, session.clone());
            if !capture.valid() {
                self.pending_owned.cancel_owner(&capture.id);
            }
        }
        let preview_id = session.id();
        let context = StoppedContext::new(
            stopping_app.as_ref(),
            owned
                .as_ref()
                .map(|_| self.app.config().identifier.as_str()),
            match &destination {
                ExternalDestination::Captured(target) => Some(target.app_id.as_str()),
                _ => None,
            },
        );
        let command_pid = context.command_pid;
        let seq = self.queue.reserve();
        let request = DictationRequest {
            samples,
            sample_rate: TARGET_RATE,
            app_bundle_id: context.app_bundle_id,
            keys: self.shared.keys(),
        };
        let copy_only = self.recording_copy_only;
        let is_command = self.command.is_command() && !copy_only && owned.is_none();
        if let Some(capture) = &owned {
            owned_dictation::phase(&self.app, capture, SessionPhase::Working);
        }
        self.command.reset();
        self.status.recording_submitted();
        self.render();
        let tx = self.tx.clone();
        if is_command {
            let pid = command_pid;
            let paster = self.paster.clone();
            let shared = self.shared.clone();
            std::thread::spawn(move || {
                let still_there = || {
                    command_field_matches(
                        pid,
                        foreground::current().map(|app| app.pid),
                        matches!(&destination, ExternalDestination::Captured(target) if external_paste::is_current(target)),
                    )
                };
                let outcome = if !still_there() {
                    Err(Failure::AppChanged)
                } else {
                    let ExternalDestination::Captured(target) = &destination else {
                        unreachable!("the current command field has a captured target")
                    };
                    let (reply, selection) = mpsc::channel();
                    let selection = paster
                        .send(PasteJob::ReadSelection {
                            reply,
                            target_pid: pid,
                            target: target.clone(),
                            pending: paste_pending(&shared),
                        })
                        .ok()
                        .and_then(|()| selection.recv().ok())
                        .flatten();
                    if !still_there() {
                        Err(Failure::AppChanged)
                    } else {
                        match session.finish_command(request, selection) {
                            Ok(_) if !still_there() => Err(Failure::AppChanged),
                            other => other.map_err(Failure::Core),
                        }
                    }
                };
                let _ = tx.send(Msg::Finished {
                    seq,
                    outcome: Box::new(outcome),
                    released,
                    is_command: true,
                    destination,
                    preview_id,
                    pending,
                    owned: None,
                });
            });
            return;
        }
        std::thread::spawn(move || {
            let outcome = session.finish(request).map_err(Failure::Core);
            let _ = tx.send(Msg::Finished {
                seq,
                outcome: Box::new(outcome),
                released,
                is_command: false,
                destination,
                preview_id,
                pending,
                owned,
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
            destination,
            preview_id,
            owned,
        } = delivery;
        self.pending_owned.complete(&preview_id);
        let copy_only = matches!(destination, ExternalDestination::CopyOnly);
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
                    Some(match &owned {
                        Some(capture) => CompletedDelivery::Owned {
                            capture: capture.clone(),
                            text: result.text,
                        },
                        None => CompletedDelivery::External(PasteText {
                            text: result.text,
                            released,
                            destination,
                        }),
                    }),
                    notice_for_cleanup(&result.cleanup).map(|notice| {
                        if copy_only || owned.is_some() {
                            notice.replace("geplakt", "beschikbaar")
                        } else {
                            notice
                        }
                    }),
                )
            }
            Err(failure) => {
                cancel_destination(&destination);
                if let Some(capture) = &owned
                    && capture.valid()
                {
                    let message = match &failure {
                        Failure::Core(PennelError::NoSpeech) => {
                            "Geen spraak herkend. Probeer opnieuw.".into()
                        }
                        Failure::Core(error) => error.to_string(),
                        Failure::AppChanged => "De dicteersessie is gewijzigd.".into(),
                    };
                    owned_dictation::failed(&self.app, &capture.id, message);
                }
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
        self.clear_preview(&preview_id);
        if notice.is_some() {
            self.notice_until = Some(Instant::now() + NOTICE_DURATION);
        }
        self.status.dictation_finished(notice);
        self.render();
        let _ = self.app.emit("history-changed", ());
        for result in self.queue.fulfill(seq, text) {
            match result {
                CompletedDelivery::External(result) => {
                    let _ = self.paster.send(PasteJob::Paste {
                        result,
                        pending: paste_pending(&self.shared),
                    });
                }
                CompletedDelivery::Owned { capture, text } => {
                    if owned_dictation::complete(&self.app, &self.shared, &capture, text.clone())
                        && owned_dictation::external_allowed(&self.shared, &capture)
                    {
                        let _ = self.paster.send(PasteJob::Owned {
                            capture,
                            text,
                            pending: paste_pending(&self.shared),
                        });
                    }
                }
            }
        }
        let _ = self.tx.send(Msg::RefreshOwned);
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
        self.discard_recording_preview();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct PendingObserver(std::sync::Mutex<Vec<DictationPreviewUpdate>>);

    impl DictationPreviewObserver for PendingObserver {
        fn changed(&self, update: DictationPreviewUpdate) {
            self.0.lock().unwrap().push(update);
        }
    }

    struct PendingSpeech {
        entered: Sender<()>,
        release: std::sync::Mutex<Receiver<()>>,
    }

    impl pennel_core::SystemTranscriber for PendingSpeech {
        fn transcribe(
            &self,
            _: Vec<f32>,
            _: u32,
            _: Option<String>,
        ) -> Result<String, PennelError> {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            Ok("Een laat resultaat".into())
        }
    }

    fn pending_engine() -> (tempfile::TempDir, Arc<pennel_core::PennelEngine>) {
        let directory = tempfile::tempdir().unwrap();
        let engine =
            pennel_core::PennelEngine::new_preview(directory.path().to_string_lossy().into_owned())
                .unwrap();
        let mut settings = engine.settings();
        settings.providers.push(pennel_core::ProviderAccount {
            id: "apple-pending-test".into(),
            kind: pennel_core::ProviderKind::AppleSpeech,
            name: "Apple".into(),
            base_url: String::new(),
        });
        settings.dictation_stt = pennel_core::ModelChoice {
            provider_id: "apple-pending-test".into(),
            model: "dictation".into(),
        };
        settings.mode = pennel_core::DictationMode::Fast;
        engine.update_settings(settings).unwrap();
        (directory, engine)
    }

    #[test]
    fn closing_a_finalizing_owner_cancels_core_before_any_history_or_result_commits() {
        for practice in [false, true] {
            let (_directory, engine) = pending_engine();
            let (entered, ready) = mpsc::channel();
            let (release, waiting) = mpsc::channel();
            engine.set_system_transcriber(Arc::new(PendingSpeech {
                entered,
                release: std::sync::Mutex::new(waiting),
            }));
            let observer = Arc::new(PendingObserver::default());
            let session = engine
                .clone()
                .begin_dictation_session(observer.clone(), practice);
            let weak = Arc::downgrade(&session);
            let mut pending = PendingOwnedSessions::default();
            pending.track("closed-owner", session.clone());
            let worker = std::thread::spawn(move || {
                session.finish(DictationRequest {
                    samples: vec![0.1; 16_000],
                    sample_rate: 16_000,
                    app_bundle_id: Some("pennel.test.owned".into()),
                    keys: pennel_core::ApiKeys {
                        by_provider: std::collections::HashMap::new(),
                    },
                })
            });
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
            pending.cancel_owner("closed-owner");
            assert!(pending.0.is_empty());
            release.send(()).unwrap();
            assert_eq!(worker.join().unwrap(), Err(PennelError::Cancelled));
            assert!(observer.0.lock().unwrap().is_empty());
            assert!(engine.history(10).unwrap().is_empty());
            assert!(
                weak.upgrade().is_none(),
                "the finished worker must release its session"
            );
        }
    }

    #[test]
    fn owner_cancellation_reaches_all_pending_captures_without_touching_another_owner() {
        let (_directory, engine) = pending_engine();
        let observer = Arc::new(PendingObserver::default());
        let old = engine
            .clone()
            .begin_dictation_session(observer.clone(), false);
        let newer = engine
            .clone()
            .begin_dictation_session(observer.clone(), false);
        let other = engine.begin_dictation_session(observer.clone(), false);
        let mut pending = PendingOwnedSessions::default();
        pending.track("pad", old.clone());
        pending.track("pad", newer.clone());
        pending.track("practice", other.clone());
        pending.cancel_owner("pad");
        old.update_system_preview("oud".into());
        newer.update_system_preview("ook oud".into());
        other.update_system_preview("nog actief".into());
        assert_eq!(observer.0.lock().unwrap().len(), 1);
        assert_eq!(observer.0.lock().unwrap()[0].session_id, other.id());
        assert_eq!(pending.0.len(), 1);
        pending.complete(&other.id());
        assert!(pending.0.is_empty());
        other.update_system_preview("bewaarde actieve sessie".into());
        assert_eq!(observer.0.lock().unwrap().len(), 2);
    }

    #[test]
    fn commands_and_style_rules_use_the_app_at_stop_while_owned_modes_keep_pennel() {
        let start = ForegroundApp {
            pid: 24,
            executable: "outlook.exe".into(),
        };
        let stop = ForegroundApp {
            pid: 42,
            executable: "slack.exe".into(),
        };
        let start_context = StoppedContext::new(Some(&start), None, Some("com.microsoft.Outlook"));
        let stop_context =
            StoppedContext::new(Some(&stop), None, Some("com.tinyspeck.slackmacgap"));
        assert_ne!(start_context.command_pid, stop_context.command_pid);
        assert_eq!(stop_context.command_pid, Some(42));
        assert_eq!(
            stop_context.app_bundle_id.as_deref(),
            Some("com.tinyspeck.slackmacgap")
        );
        assert!(command_field_matches(
            stop_context.command_pid,
            Some(42),
            true
        ));
        assert!(!command_field_matches(
            stop_context.command_pid,
            Some(24),
            true
        ));
        assert!(!command_field_matches(
            stop_context.command_pid,
            Some(42),
            false
        ));
        let racing_query =
            StoppedContext::new(Some(&start), None, Some("com.tinyspeck.slackmacgap"));
        assert_eq!(racing_query.app_bundle_id, stop_context.app_bundle_id);
        assert_eq!(
            StoppedContext::new(Some(&stop), None, None).app_bundle_id,
            None
        );
        let owned = StoppedContext::new(
            Some(&stop),
            Some("dev.pennel.preview"),
            Some("com.tinyspeck.slackmacgap"),
        );
        assert_eq!(owned.command_pid, None);
        assert_eq!(owned.app_bundle_id.as_deref(), Some("dev.pennel.preview"));
    }

    #[test]
    fn global_delivery_captures_the_destination_once_and_never_for_copy_only() {
        let calls = std::cell::Cell::new(0);
        let destination = capture_destination(false, || {
            calls.set(calls.get() + 1);
            Ok(42u32)
        });
        assert_eq!(destination, ExternalDestination::Captured(42));
        assert_eq!(calls.get(), 1);
        let copied = capture_destination(true, || {
            calls.set(calls.get() + 1);
            Ok(99u32)
        });
        assert_eq!(copied, ExternalDestination::CopyOnly);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            capture_destination::<u32>(false, || Err("no current field".into())),
            ExternalDestination::Unavailable("no current field".into())
        );
    }

    #[test]
    fn replaced_and_cancelled_recordings_reject_late_preview_updates() {
        let mut cursor = PreviewCursor::default();
        cursor.begin("first".into(), Some("pad".into()));
        assert!(cursor.accept(&preview("first", 1), true));
        assert!(!cursor.accept(&preview("first", 1), true));
        assert!(!cursor.accept(&preview("first", 0), true));
        cursor.begin("second".into(), None);
        assert!(!cursor.accept(&preview("first", 9), true));
        assert!(!cursor.clear_session("first"));
        assert!(cursor.accept(&preview("second", 1), true));
        assert!(cursor.clear_session("second"));
        assert!(!cursor.accept(&preview("second", 2), true));
    }

    #[test]
    fn closed_owners_cannot_publish_preview_text_into_a_different_session() {
        let mut cursor = PreviewCursor::default();
        cursor.begin("practice-capture".into(), Some("practice".into()));
        assert!(!cursor.accept(&preview("practice-capture", 1), false));
        assert_eq!(cursor.clear_owner("other"), None);
        assert_eq!(
            cursor.clear_owner("practice").as_deref(),
            Some("practice-capture")
        );
        assert!(!cursor.accept(&preview("practice-capture", 2), true));
        cursor.begin("pad-capture".into(), Some("pad".into()));
        assert_eq!(cursor.clear_owner("practice"), None);
        assert!(cursor.accept(&preview("pad-capture", 1), true));
    }

    fn preview(id: &str, revision: u64) -> DictationPreviewUpdate {
        DictationPreviewUpdate {
            session_id: id.into(),
            revision,
            text: "Voorlopige tekst".into(),
        }
    }

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
        assert!(command_field_matches(Some(42), Some(42), true));
        assert!(!command_field_matches(Some(42), Some(42), false));
        assert!(!command_field_matches(Some(42), Some(9), true));
        assert!(!command_field_matches(None, Some(9), false));
    }

    #[test]
    fn ordered_paste_results_keep_their_own_target_and_release_time() {
        let mut queue: PasteQueue<PasteText<u32>> = PasteQueue::default();
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
                        destination: ExternalDestination::Captured(42),
                    })
                )
                .is_empty()
        );
        let ready = queue.fulfill(
            first,
            Some(PasteText {
                text: "first".into(),
                released: released - Duration::from_secs(1),
                destination: ExternalDestination::Captured(24),
            }),
        );
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].destination, ExternalDestination::Captured(24));
        assert_eq!(ready[1].destination, ExternalDestination::Captured(42));
        assert_eq!(ready[1].released, released);
        assert_eq!(ready[0].released, released - Duration::from_secs(1));
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
