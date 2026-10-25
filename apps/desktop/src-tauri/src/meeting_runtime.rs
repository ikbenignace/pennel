//! Recording ownership outlives windows and remains busy until audio and transcript writes finish.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use pennel_core::{MeetingSession, ModelState, RecordingPhase};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::diagnostics::Log;
use crate::meeting::MeetingRecorder;
use crate::state::Shared;

const LOG: Log = Log::new("meeting");

pub struct MeetingContext {
    pub token: String,
    pub session: Arc<MeetingSession>,
    pub title: String,
    pub started: Instant,
    pub duration_secs: Option<u64>,
    pub system_audio: bool,
    pub heard_system_audio: bool,
    pub audio_warning: Option<String>,
    pub consent_copied: bool,
}

pub struct LibraryJob(Arc<Shared>);

impl LibraryJob {
    pub fn new(shared: &Arc<Shared>) -> Result<Self, String> {
        let _recording = shared.recording.lock().unwrap();
        if shared.quitting.load(Ordering::SeqCst) {
            return Err("Pennel wordt afgesloten. Open de app opnieuw om verder te gaan.".into());
        }
        if shared.updating.load(Ordering::SeqCst) {
            return Err("Pennel wordt bijgewerkt. Wacht tot de app opnieuw is gestart.".into());
        }
        shared.library_jobs.fetch_add(1, Ordering::SeqCst);
        Ok(Self(shared.clone()))
    }
}

impl Drop for LibraryJob {
    fn drop(&mut self) {
        self.0.library_jobs.fetch_sub(1, Ordering::SeqCst);
    }
}

fn fail(shared: &Shared, token: &str, error: &str) {
    let _ = shared
        .recording
        .lock()
        .unwrap()
        .fail(token, error.to_owned());
}

pub fn set_title(shared: &Shared, title: String, session_id: &str) -> Result<(), String> {
    let lifecycle = shared.recording.lock().unwrap();
    let state = lifecycle.snapshot();
    match state.phase {
        RecordingPhase::Recording | RecordingPhase::Stopping | RecordingPhase::Error => {}
        RecordingPhase::Starting => {
            return Err("Wacht tot de opname is gestart om de titel te wijzigen.".into());
        }
        RecordingPhase::Finalizing => {
            return Err(
                "De opname wordt al afgerond. Hernoem de meeting daarna vanuit de bibliotheek."
                    .into(),
            );
        }
        RecordingPhase::Idle => {
            return Err("Er is geen actieve opname waarvan je de titel kunt wijzigen.".into());
        }
    }
    if state.session_id.as_deref() != Some(session_id) {
        return Err("Deze opnamesessie is niet meer actief.".into());
    }
    let mut context = shared.meeting_context.lock().unwrap();
    let context = context
        .as_mut()
        .filter(|context| context.token == session_id)
        .ok_or("Deze opnamesessie is niet meer actief.")?;
    context.title = title;
    #[cfg(target_os = "macos")]
    crate::macos_tray::changed();
    Ok(())
}

fn rename_after_recording(
    engine: &pennel_core::PennelEngine,
    id: String,
    original_title: &str,
    desired_title: &str,
) -> Result<String, String> {
    let wanted = desired_title.trim();
    if wanted.is_empty() || wanted == original_title {
        return Ok(id);
    }
    engine
        .rename_meeting(id, wanted.to_owned())
        .map_err(|error| format!("De opnametitel kon niet worden opgeslagen: {error}"))
}

fn copy_consent(
    enabled: bool,
    copy: impl FnOnce(&str) -> Result<(), String>,
) -> Result<bool, String> {
    if !enabled {
        return Ok(false);
    }
    copy(&pennel_core::consent_sentence())?;
    Ok(true)
}

fn begin_recording(shared: &Shared, detected_app: Option<String>) -> Result<String, String> {
    let finalized_error = {
        let lifecycle = shared.recording.lock().unwrap();
        if shared.quitting.load(Ordering::SeqCst) {
            return Err("Pennel wordt afgesloten. Open de app opnieuw om verder te gaan.".into());
        }
        if shared.updating.load(Ordering::SeqCst) {
            return Err("Pennel wordt bijgewerkt. Wacht tot de app opnieuw is gestart.".into());
        }
        (lifecycle.snapshot().phase == RecordingPhase::Error)
            .then(|| {
                shared
                    .meeting_context
                    .lock()
                    .unwrap()
                    .as_ref()
                    .filter(|context| context.session.is_finalized())
                    .map(|context| context.token.clone())
            })
            .flatten()
    };
    if let Some(token) = finalized_error {
        retry_saved_session(shared, &token)?;
    }
    let mut lifecycle = shared.recording.lock().unwrap();
    if shared.quitting.load(Ordering::SeqCst) {
        return Err("Pennel wordt afgesloten. Open de app opnieuw om verder te gaan.".into());
    }
    if shared.updating.load(Ordering::SeqCst) {
        return Err("Pennel wordt bijgewerkt. Wacht tot de app opnieuw is gestart.".into());
    }
    let mut context = shared.meeting_context.lock().unwrap();
    if context
        .as_ref()
        .is_some_and(|context| !context.session.is_finalized())
    {
        return Err("De vorige opname is nog niet volledig bewaard. Open de opname en probeer opnieuw te bewaren.".into());
    }
    let token = lifecycle
        .begin(detected_app)
        .map_err(|error| error.to_string())?;
    shared.recording_cancel.store(false, Ordering::SeqCst);
    context.take();
    Ok(token)
}

/// Runs on a worker: native capture initialization may need the application's main queue.
pub fn start(
    shared: &Arc<Shared>,
    title: String,
    detected_app: Option<String>,
    attendees: Vec<String>,
) -> Result<String, String> {
    let token = begin_recording(shared, detected_app.clone())?;
    #[cfg(target_os = "macos")]
    let _tray_update = crate::macos_tray::UpdateOnDrop;
    #[cfg(target_os = "macos")]
    crate::macos_tray::changed();
    let title = if title.trim().is_empty() {
        "Meeting".to_owned()
    } else {
        title.trim().to_owned()
    };
    let settings = shared.settings();
    let keys = shared.keys();
    if let Err(error) = shared.engine.validate_meeting_setup(keys.clone()) {
        let error = error.to_string();
        fail(shared, &token, &error);
        return Err(error);
    }
    #[cfg(target_os = "macos")]
    if settings
        .resolve(&settings.meeting_stt)
        .is_some_and(|(provider, _)| provider.kind == pennel_core::ProviderKind::AppleSpeech)
        && !crate::macos::permissions()
            .get("apple_speech")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    {
        let error = "Apple-dicteren vereist macOS 26 of nieuwer. Kies een andere spraakprovider.";
        fail(shared, &token, error);
        return Err(error.into());
    }
    let session = match shared
        .engine
        .clone()
        .start_meeting_with(title.clone(), attendees, keys)
    {
        Ok(session) => session,
        Err(error) => {
            let error = error.to_string();
            fail(shared, &token, &error);
            return Err(error);
        }
    };
    let id = session.id();
    *shared.meeting_context.lock().unwrap() = Some(MeetingContext {
        token: token.clone(),
        session: session.clone(),
        title: title.clone(),
        started: Instant::now(),
        duration_secs: None,
        system_audio: false,
        heard_system_audio: false,
        audio_warning: None,
        consent_copied: false,
    });
    let recorder = match MeetingRecorder::start(
        session.clone(),
        title,
        detected_app,
        settings.echo_cancellation,
    ) {
        Ok(recorder) => recorder,
        Err(error) => {
            if session.stop().is_ok() {
                let _ = shared.engine.delete_meeting(id);
                shared.meeting_context.lock().unwrap().take();
            }
            fail(shared, &token, &error);
            return Err(error);
        }
    };
    #[cfg(any(windows, target_os = "macos"))]
    LOG.info(format!(
        "echo-onderdrukking actief={}",
        recorder.echo_cancellation_active()
    ));
    let consent_copied = match copy_consent(settings.consent_enabled, |text| {
        crate::platform_clipboard::copy(text, None)
    }) {
        Ok(copied) => copied,
        Err(_) => {
            LOG.error("De opnamemelding kon niet worden gekopieerd. De opname gaat door.");
            false
        }
    };
    let system_audio = recorder.has_system_audio();
    if let Some(context) = shared.meeting_context.lock().unwrap().as_mut() {
        context.started = recorder.started_at();
        context.system_audio = system_audio;
        context.audio_warning = recorder.audio_warning();
        context.consent_copied = consent_copied;
    }
    *shared.meeting.lock().unwrap() = Some(recorder);
    shared
        .recording
        .lock()
        .unwrap()
        .recording(&token)
        .map_err(|error| error.to_string())?;
    monitor_health(shared.clone(), token);
    if shared.recording_cancel.load(Ordering::SeqCst) {
        stop_and_wait(shared)?;
    }
    Ok(id)
}

fn take_recorder(
    shared: &Shared,
    expected_call: Option<&str>,
) -> Result<Option<(String, MeetingRecorder)>, String> {
    take_recorder_matching(shared, expected_call, None)
}

fn take_recorder_matching(
    shared: &Shared,
    expected_call: Option<&str>,
    expected_session: Option<&str>,
) -> Result<Option<(String, MeetingRecorder)>, String> {
    let mut lifecycle = shared.recording.lock().unwrap();
    if expected_call.is_some_and(|app| !lifecycle.owns_call(app)) {
        return Ok(None);
    }
    let state = lifecycle.snapshot();
    if expected_session.is_some_and(|id| state.session_id.as_deref() != Some(id)) {
        return Ok(None);
    }
    match state.phase {
        RecordingPhase::Starting => {
            shared.recording_cancel.store(true, Ordering::SeqCst);
            return Ok(None);
        }
        RecordingPhase::Idle
        | RecordingPhase::Error
        | RecordingPhase::Stopping
        | RecordingPhase::Finalizing => return Ok(None),
        RecordingPhase::Recording => {}
    }
    let token = state.session_id.ok_or("De opnamesessie ontbreekt.")?;
    lifecycle
        .stopping(&token)
        .map_err(|error| error.to_string())?;
    let recorder = match shared.meeting.lock().unwrap().take() {
        Some(recorder) => recorder,
        None => {
            let error = "De audio-opname is niet beschikbaar.";
            let _ = lifecycle.fail(&token, error.into());
            return Err(error.into());
        }
    };
    Ok(Some((token, recorder)))
}

fn monitor_health(shared: Arc<Shared>, token: String) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let state = shared.recording.lock().unwrap().snapshot();
            if state.session_id.as_deref() != Some(&token)
                || state.phase != RecordingPhase::Recording
            {
                return;
            }
            let terminal = shared
                .meeting
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|recorder| {
                    recorder
                        .session
                        .health()
                        .terminal_error
                        .or_else(|| recorder.terminal_error())
                });
            if terminal.is_some() {
                if let Ok(Some((token, recorder))) =
                    take_recorder_matching(&shared, None, Some(&token))
                {
                    LOG.error("De opname wordt gestopt na een audio- of opslagfout.");
                    let _ = finish(&shared, &token, recorder);
                }
                return;
            }
        }
    });
}

fn finish(shared: &Shared, token: &str, recorder: MeetingRecorder) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let _tray_update = crate::macos_tray::UpdateOnDrop;
    let original_title = recorder.title.clone();
    let system_audio = recorder.has_system_audio();
    let heard_system_audio = recorder.heard_system_audio();
    let audio_warning = recorder.audio_warning();
    let terminal_error = recorder
        .terminal_error()
        .or_else(|| recorder.session.health().terminal_error);
    let mut desired_title = original_title.clone();
    if let Some(context) = shared
        .meeting_context
        .lock()
        .unwrap()
        .as_mut()
        .filter(|context| context.token == token)
    {
        context.system_audio = system_audio;
        context.heard_system_audio = heard_system_audio;
        context.audio_warning = audio_warning.or_else(|| terminal_error.clone());
        context.duration_secs = Some(context.started.elapsed().as_secs());
        desired_title = context.title.clone();
    }
    shared
        .recording
        .lock()
        .unwrap()
        .finalizing(token)
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    crate::macos_tray::changed();
    finish_after_capture(
        shared,
        token,
        recorder.stop(),
        terminal_error,
        &original_title,
        &desired_title,
    )
}

fn finish_after_capture(
    shared: &Shared,
    token: &str,
    captured: Result<String, String>,
    terminal_error: Option<String>,
    original_title: &str,
    desired_title: &str,
) -> Result<String, String> {
    let result = captured.and_then(|id| {
        if let Some(error) = terminal_error {
            return Err(error);
        }
        rename_after_recording(&shared.engine, id, original_title, desired_title)
    });
    finish_context(shared, token, result)
}

fn finish_context(
    shared: &Shared,
    token: &str,
    result: Result<String, String>,
) -> Result<String, String> {
    match result {
        Ok(id) => {
            let mut lifecycle = shared.recording.lock().unwrap();
            lifecycle
                .complete(token)
                .map_err(|error| error.to_string())?;
            shared.meeting_context.lock().unwrap().take();
            Ok(id)
        }
        Err(error) => {
            fail(shared, token, &error);
            Err(error)
        }
    }
}

fn retry_saved_session(shared: &Shared, session_id: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let _tray_update = crate::macos_tray::UpdateOnDrop;
    let (session, desired_title) = {
        let mut lifecycle = shared.recording.lock().unwrap();
        let state = lifecycle.snapshot();
        if state.phase != RecordingPhase::Error || state.session_id.as_deref() != Some(session_id) {
            return Err("Deze opname wacht niet meer op herstel.".into());
        }
        if shared.meeting.lock().unwrap().is_some() {
            return Err("Rond eerst de audio-opname af voordat je opnieuw bewaart.".into());
        }
        let context = shared.meeting_context.lock().unwrap();
        let context = context
            .as_ref()
            .filter(|context| context.token == session_id)
            .ok_or("Deze opnamesessie is niet meer beschikbaar.")?;
        lifecycle
            .retry_finalizing(session_id)
            .map_err(|error| error.to_string())?;
        (context.session.clone(), context.title.clone())
    };
    let id = session.id();
    let original_title = shared
        .engine
        .meeting_info(id.clone())
        .map_or_else(|| desired_title.clone(), |info| info.title);
    let saved = if session.is_finalized() {
        Ok(())
    } else {
        session.retry_persistence()
    };
    let result = saved
        .map_err(|error| error.to_string())
        .and_then(|()| rename_after_recording(&shared.engine, id, &original_title, &desired_title));
    finish_context(shared, session_id, result)
}

/// Saves the retained failed session only; successful speech recognition is never repeated here.
pub fn retry_persistence(
    app: &AppHandle,
    shared: &Arc<Shared>,
    session_id: &str,
) -> Result<(), String> {
    let _job = LibraryJob::new(shared)?;
    let result = retry_saved_session(shared, session_id);
    let _ = app.emit(
        "meeting-finished",
        MeetingFinished {
            id: result.clone().ok(),
            session_id: session_id.to_owned(),
            error: result.as_ref().err().cloned(),
            flush_warning: None,
        },
    );
    if let Ok(id) = &result {
        let _ = app.emit("meeting-stopped", Some(id));
        let _ = app.emit("meeting-updated", id);
        crate::cloud::poke(app);
    }
    result.map(|_| ())
}

#[derive(Clone, Serialize)]
struct MeetingFinished {
    id: Option<String>,
    session_id: String,
    error: Option<String>,
    flush_warning: Option<String>,
}

/// `expected_call` keeps an ended call from stopping a manual or different recording.
pub fn stop(
    app: AppHandle,
    shared: Arc<Shared>,
    expected_call: Option<&str>,
) -> Result<(), String> {
    let Some((token, recorder)) = take_recorder(&shared, expected_call)? else {
        return Ok(());
    };
    #[cfg(target_os = "macos")]
    crate::macos_tray::changed();
    std::thread::spawn(move || {
        let library_job = LibraryJob::new(&shared).ok();
        let flush = crate::flush_frontends(&app, "prepare-recording-stop");
        let flush_warning = flush.result.err().map(|_| {
            "Niet alle conceptwijzigingen konden worden opgeslagen. De opname is afgerond; controleer je bewaarde concept.".to_owned()
        });
        if flush_warning.is_some() {
            LOG.error(
                "Conceptwijzigingen konden niet volledig worden opgeslagen voor het stoppen.",
            );
        }
        let result = finish(&shared, &token, recorder);
        if result.is_err() {
            LOG.error("Meeting afronden mislukt; de opname blijft zichtbaar in Pennel.");
        }
        let finished = MeetingFinished {
            id: result.clone().ok(),
            session_id: token,
            error: result.as_ref().err().cloned(),
            flush_warning,
        };
        let _ = app.emit("meeting-finished", finished);
        crate::cloud::poke(&app);
        let _ = app.emit("meeting-stopped", result.clone().ok());
        let _ = app.emit("recording-flush-complete", flush.request_id);
        if let Ok(id) = result
            && library_job.is_some()
            && !shared.quitting.load(Ordering::SeqCst)
        {
            postprocess(app, shared, id);
        }
    });
    Ok(())
}

fn postprocess(app: AppHandle, shared: Arc<Shared>, id: String) {
    if shared.quitting.load(Ordering::SeqCst) {
        return;
    }
    let settings = shared.settings();
    if settings.diarization
        && shared.engine.model_state(shared.engine.diarizer_model_id()) == ModelState::Downloaded
    {
        let path =
            std::path::PathBuf::from(shared.engine.meeting_dir(id.clone())).join("anderen.m4a");
        match crate::platform_audio::decode_audio(&path).and_then(|samples| {
            shared
                .engine
                .diarize_meeting(id.clone(), samples)
                .map_err(|error| error.to_string())
        }) {
            Ok(_) => {
                let _ = app.emit("meeting-updated", &id);
            }
            Err(_) => LOG.error(
                "Automatisch sprekers herkennen mislukt; probeer opnieuw vanuit de meeting.",
            ),
        }
    }
    if shared.quitting.load(Ordering::SeqCst) {
        return;
    }
    let settings = shared.settings();
    let keys = shared.keys();
    let apple_available = settings.resolve(&settings.llm).is_some_and(|(account, _)| {
        account.kind != pennel_core::ProviderKind::AppleIntelligence
            || crate::capabilities::snapshot(None)
                .apple_intelligence
                .status
                == crate::capabilities::CapabilityStatus::Available
    });
    if optional_summary(
        summary_configuration_ready(&settings, &keys, apple_available),
        || {
            shared
                .engine
                .summarize_meeting(id.clone(), settings.default_template, keys)
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
    )
    .is_err()
    {
        LOG.error("Automatisch samenvatten mislukt; transcript en notities blijven bewaard.");
    }
    let _ = app.emit("meeting-updated", id);
    crate::cloud::poke(&app);
}

fn summary_configuration_ready(
    settings: &pennel_core::Settings,
    keys: &pennel_core::ApiKeys,
    apple_available: bool,
) -> bool {
    let Some((account, model)) = settings.resolve(&settings.llm) else {
        return false;
    };
    if model.trim().is_empty()
        || !pennel_core::supports(account.kind, pennel_core::ProviderTask::Llm)
    {
        return false;
    }
    if pennel_core::requires_key(account.kind)
        && keys
            .by_provider
            .get(&account.id)
            .is_none_or(|key| key.trim().is_empty())
    {
        return false;
    }
    if account.kind.is_cli_agent() && account.base_url.trim().is_empty() {
        return false;
    }
    account.kind != pennel_core::ProviderKind::AppleIntelligence || apple_available
}

fn optional_summary(
    configured: bool,
    run: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    if !configured {
        return Ok(false);
    }
    run().map(|()| true)
}

/// Worker-only shutdown path. The host keeps its event loop alive while native audio is released.
pub fn stop_and_wait(shared: &Arc<Shared>) -> Result<Option<String>, String> {
    loop {
        let phase = shared.recording.lock().unwrap().snapshot().phase;
        match phase {
            RecordingPhase::Starting => {
                shared.recording_cancel.store(true, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(20));
            }
            RecordingPhase::Stopping | RecordingPhase::Finalizing => {
                std::thread::sleep(Duration::from_millis(20))
            }
            RecordingPhase::Recording => {
                if let Some((token, recorder)) = take_recorder(shared, None)? {
                    return finish(shared, &token, recorder).map(Some);
                }
            }
            RecordingPhase::Error => {
                if shared.meeting.lock().unwrap().is_none() {
                    let token = shared
                        .meeting_context
                        .lock()
                        .unwrap()
                        .as_ref()
                        .map(|context| context.token.clone());
                    if let Some(token) = token {
                        return retry_saved_session(shared, &token).map(Some);
                    }
                    return Ok(None);
                }
                let message = shared
                    .recording
                    .lock()
                    .unwrap()
                    .snapshot()
                    .error
                    .unwrap_or_else(|| "De opname is niet afgerond.".into());
                return Err(message);
            }
            RecordingPhase::Idle => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    #[test]
    fn an_unsaved_failed_meeting_blocks_a_new_recording_and_keeps_its_context() {
        let (_root, shared, session, token) = failed_persistence();
        assert!(start(&shared, "Nieuwe meeting".into(), None, Vec::new()).is_err());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().session_id,
            Some(token.clone())
        );
        assert_eq!(
            shared
                .meeting_context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .session
                .id(),
            session.id()
        );
        assert!(!session.is_finalized());
    }

    #[test]
    fn quitting_retries_storage_but_cannot_drop_an_unsaved_failed_meeting() {
        let (_root, shared, session, token) = failed_persistence();
        assert!(stop_and_wait(&shared).is_err());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Error
        );
        assert_eq!(
            shared
                .meeting_context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .token,
            token
        );
        assert!(!session.is_finalized());
        std::fs::remove_dir(std::path::PathBuf::from(session.dir()).join("transcript.md")).unwrap();
        assert_eq!(stop_and_wait(&shared).unwrap(), Some(session.id()));
        assert!(session.is_finalized());
        assert!(shared.meeting_context.lock().unwrap().is_none());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Idle
        );
    }

    #[test]
    fn save_retry_rejects_a_stale_token_before_changing_the_failed_session() {
        let (_root, shared, session, token) = failed_persistence();
        assert!(retry_saved_session(&shared, "vorige-sessie").is_err());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().session_id,
            Some(token)
        );
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Error
        );
        assert!(!session.is_finalized());
    }

    #[test]
    fn save_retry_finishes_the_same_session_and_preserves_its_title_and_notes() {
        let (_root, shared, session, token) = failed_persistence();
        shared
            .meeting_context
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .title = "Herstelde titel".into();
        std::fs::remove_dir(std::path::PathBuf::from(session.dir()).join("transcript.md")).unwrap();
        let id = retry_saved_session(&shared, &token).unwrap();
        assert_ne!(id, session.id());
        assert_eq!(
            shared.engine.meeting_info(id.clone()).unwrap().title,
            "Herstelde titel"
        );
        assert_eq!(shared.engine.meeting_notes(id), "Een bewaarde afspraak.");
        assert!(session.is_finalized());
        assert!(shared.meeting_context.lock().unwrap().is_none());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Idle
        );
    }

    #[test]
    fn a_stale_health_watcher_cannot_stop_a_newer_recording() {
        let (_root, shared, session, token) = recording();
        assert!(
            take_recorder_matching(&shared, None, Some("vorige-sessie"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().session_id,
            Some(token)
        );
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Recording
        );
        session.stop().unwrap();
    }

    #[test]
    fn a_durable_capture_failure_can_be_acknowledged_by_starting_again() {
        let (_root, shared, session, token) = recording();
        session.stop().unwrap();
        shared.recording.lock().unwrap().stopping(&token).unwrap();
        shared.recording.lock().unwrap().finalizing(&token).unwrap();
        assert_eq!(
            finish_after_capture(
                &shared,
                &token,
                Ok(session.id()),
                Some("Microfoon onderbroken".into()),
                "Originele titel",
                "Originele titel"
            ),
            Err("Microfoon onderbroken".into())
        );
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Error
        );
        assert!(shared.meeting_context.lock().unwrap().is_some());
        let next = begin_recording(&shared, None).unwrap();
        assert_ne!(next, token);
        assert!(shared.meeting_context.lock().unwrap().is_none());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Starting
        );
    }

    #[test]
    fn late_title_drafts_after_capture_failure_are_saved_before_quitting() {
        let (_root, shared, session, token) = recording();
        session.stop().unwrap();
        shared
            .recording
            .lock()
            .unwrap()
            .fail(&token, "Microfoon onderbroken".into())
            .unwrap();
        set_title(&shared, "Laatste titel".into(), &token).unwrap();
        let id = stop_and_wait(&shared).unwrap().unwrap();
        assert_eq!(
            shared.engine.meeting_info(id.clone()).unwrap().title,
            "Laatste titel"
        );
        assert_eq!(shared.engine.meeting_notes(id), "Een bewaarde afspraak.");
        assert!(shared.meeting_context.lock().unwrap().is_none());
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Idle
        );
    }

    #[test]
    fn starting_again_saves_the_finalized_failed_context_before_replacing_it() {
        let (_root, shared, session, token) = recording();
        session.stop().unwrap();
        shared
            .recording
            .lock()
            .unwrap()
            .fail(&token, "Microfoon onderbroken".into())
            .unwrap();
        set_title(&shared, "Titel na onderbreking".into(), &token).unwrap();
        let next = begin_recording(&shared, None).unwrap();
        assert_ne!(next, token);
        assert_eq!(shared.engine.meetings()[0].title, "Titel na onderbreking");
        assert!(shared.meeting_context.lock().unwrap().is_none());
    }

    fn failed_persistence() -> (tempfile::TempDir, Arc<Shared>, Arc<MeetingSession>, String) {
        let (root, shared, session, token) = recording();
        shared.recording.lock().unwrap().stopping(&token).unwrap();
        shared.recording.lock().unwrap().finalizing(&token).unwrap();
        std::fs::create_dir(std::path::PathBuf::from(session.dir()).join("transcript.md")).unwrap();
        let error = session.stop().unwrap_err().to_string();
        fail(&shared, &token, &error);
        (root, shared, session, token)
    }

    #[test]
    fn fresh_local_fast_meeting_finishes_without_a_missing_llm_failure() {
        let (_root, shared, session, _token) = recording();
        assert_eq!(shared.settings().mode, pennel_core::DictationMode::Fast);
        session.stop().unwrap();
        let settings = shared.settings();
        let keys = pennel_core::ApiKeys {
            by_provider: HashMap::new(),
        };
        let configured = summary_configuration_ready(&settings, &keys, false);
        assert!(!configured);
        assert!(
            !optional_summary(configured, || panic!(
                "An unconfigured LLM must not be called"
            ))
            .unwrap()
        );
        assert_eq!(
            shared.engine.meeting_notes(session.id()),
            "Een bewaarde afspraak."
        );
        assert!(shared.engine.meeting_info(session.id()).is_some());
        assert!(shared.engine.meeting_markdown(session.id()).is_none());
    }

    #[test]
    fn configured_summary_errors_remain_recoverable_errors() {
        let mut settings = pennel_core::default_settings();
        let account = pennel_core::new_account(
            pennel_core::ProviderKind::Ollama,
            settings.providers.clone(),
        );
        settings.llm = pennel_core::ModelChoice {
            provider_id: account.id.clone(),
            model: "configured-model".into(),
        };
        settings.providers.push(account);
        let keys = pennel_core::ApiKeys {
            by_provider: HashMap::new(),
        };
        let configured = summary_configuration_ready(&settings, &keys, false);
        assert!(configured);
        assert_eq!(
            optional_summary(configured, || Err(
                "Provider tijdelijk niet bereikbaar".into()
            )),
            Err("Provider tijdelijk niet bereikbaar".into())
        );
        settings.llm.model.clear();
        assert!(!summary_configuration_ready(&settings, &keys, false));
    }

    #[test]
    fn consent_setting_off_never_changes_the_clipboard() {
        let copied = copy_consent(false, |_| panic!("disabled setting must not copy")).unwrap();
        assert!(!copied);
    }

    #[test]
    fn enabled_consent_reports_success_only_after_copying_the_canonical_sentence() {
        let mut observed = String::new();
        let copied = copy_consent(true, |text| {
            observed = text.to_owned();
            Ok(())
        })
        .unwrap();
        assert!(copied);
        assert_eq!(observed, pennel_core::consent_sentence());
    }

    #[test]
    fn failed_consent_copy_does_not_report_success() {
        assert!(copy_consent(true, |_| Err("Klembord niet beschikbaar.".into())).is_err());
    }

    #[test]
    fn failed_setup_without_capture_does_not_prevent_quitting() {
        let root = tempfile::tempdir().unwrap();
        let shared = Shared::testing(root.path().join("preview"));
        let token = shared.recording.lock().unwrap().begin(None).unwrap();
        shared
            .recording
            .lock()
            .unwrap()
            .fail(&token, "Geen spraakmodel ingesteld".into())
            .unwrap();
        assert_eq!(stop_and_wait(&shared).unwrap(), None);
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Error
        );
    }

    fn recording() -> (tempfile::TempDir, Arc<Shared>, Arc<MeetingSession>, String) {
        let root = tempfile::tempdir().unwrap();
        let data_dir = root.path().join("preview");
        let engine =
            pennel_core::PennelEngine::new_preview(data_dir.to_string_lossy().into_owned())
                .unwrap();
        let session = engine
            .clone()
            .start_meeting(
                "Originele titel".into(),
                pennel_core::ApiKeys {
                    by_provider: HashMap::new(),
                },
            )
            .unwrap();
        session.set_notes("Een bewaarde afspraak.".into()).unwrap();
        let mut lifecycle = pennel_core::RecordingLifecycle::default();
        let token = lifecycle.begin(None).unwrap();
        lifecycle.recording(&token).unwrap();
        let (dictation, _) = std::sync::mpsc::channel();
        let shared = Arc::new(Shared {
            engine,
            version: "0.2.0-test".into(),
            preview: true,
            migration_warning: None,
            secrets: Arc::new(crate::secrets::Secrets::in_memory()),
            data_dir,
            dictation,
            hotkey: Mutex::new(None),
            hotkey_error: Mutex::new(None),
            meeting: Mutex::new(None),
            recording: Mutex::new(lifecycle),
            recording_cancel: AtomicBool::new(false),
            library_jobs: AtomicUsize::new(0),
            updating: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
            dictation_active: AtomicBool::new(false),
            dictation_jobs: AtomicUsize::new(0),
            paste_jobs: AtomicUsize::new(0),
            meeting_context: Mutex::new(Some(MeetingContext {
                token: token.clone(),
                session: session.clone(),
                title: "Originele titel".into(),
                started: Instant::now(),
                duration_secs: None,
                system_audio: false,
                heard_system_audio: false,
                audio_warning: None,
                consent_copied: false,
            })),
        });
        (root, shared, session, token)
    }

    #[test]
    fn editing_title_defers_folder_rename_until_session_flush_and_preserves_files() {
        let (_root, shared, session, token) = recording();
        let original_id = session.id();
        let original_dir = std::path::PathBuf::from(session.dir());
        set_title(&shared, "Nieuwe titel".into(), &token).unwrap();
        assert!(original_dir.exists());
        assert_eq!(
            shared
                .engine
                .meeting_info(original_id.clone())
                .unwrap()
                .title,
            "Originele titel"
        );
        assert_eq!(
            shared
                .meeting_context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .title,
            "Nieuwe titel"
        );

        shared.recording.lock().unwrap().stopping(&token).unwrap();
        shared.recording.lock().unwrap().finalizing(&token).unwrap();
        session.stop().unwrap();
        std::fs::write(
            original_dir.join("transcript.md"),
            b"[00:00] **Ik:** Een afgerond transcript.\n",
        )
        .unwrap();
        std::fs::write(original_dir.join("ik.m4a"), b"fixture me archive").unwrap();
        std::fs::write(original_dir.join("anderen.m4a"), b"fixture others archive").unwrap();
        let id = rename_after_recording(
            &shared.engine,
            original_id.clone(),
            "Originele titel",
            "Nieuwe titel",
        )
        .unwrap();
        let directory = std::path::PathBuf::from(shared.engine.meeting_dir(id.clone()));
        assert_ne!(id, original_id);
        assert!(!original_dir.exists());
        assert_eq!(
            shared.engine.meeting_info(id.clone()).unwrap().title,
            "Nieuwe titel"
        );
        assert_eq!(
            shared.engine.meeting_notes(id.clone()),
            "Een bewaarde afspraak."
        );
        assert_eq!(
            shared.engine.meeting_transcript(id)[0].text,
            "Een afgerond transcript."
        );
        assert_eq!(
            std::fs::read(directory.join("transcript.md")).unwrap(),
            b"[00:00] **Ik:** Een afgerond transcript.\n"
        );
        assert_eq!(
            std::fs::read(directory.join("ik.m4a")).unwrap(),
            b"fixture me archive"
        );
        assert_eq!(
            std::fs::read(directory.join("anderen.m4a")).unwrap(),
            b"fixture others archive"
        );
        assert_eq!(
            shared.recording.lock().unwrap().snapshot().phase,
            RecordingPhase::Finalizing
        );
    }

    #[test]
    fn blank_title_stays_editable_and_does_not_reset_the_stored_title() {
        let (_root, shared, session, token) = recording();
        set_title(&shared, "  ".into(), &token).unwrap();
        assert_eq!(
            shared
                .meeting_context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .title,
            "  "
        );
        session.stop().unwrap();
        let id =
            rename_after_recording(&shared.engine, session.id(), "Originele titel", "  ").unwrap();
        assert_eq!(id, session.id());
        assert_eq!(
            shared.engine.meeting_info(id).unwrap().title,
            "Originele titel"
        );
    }

    #[test]
    fn stale_session_and_finalizing_edits_cannot_change_the_live_title() {
        let (_root, shared, session, token) = recording();
        assert!(set_title(&shared, "Vertraagde wijziging".into(), "vorige-sessie").is_err());
        assert_eq!(
            shared
                .meeting_context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .title,
            "Originele titel"
        );
        shared.recording.lock().unwrap().stopping(&token).unwrap();
        set_title(&shared, "Laatste conceptwijziging".into(), &token).unwrap();
        shared.recording.lock().unwrap().finalizing(&token).unwrap();
        assert!(set_title(&shared, "Nog later".into(), &token).is_err());
        assert_eq!(
            shared
                .meeting_context
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .title,
            "Laatste conceptwijziging"
        );
        session.stop().unwrap();
    }
}
