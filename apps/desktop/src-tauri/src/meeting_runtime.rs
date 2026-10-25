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
        RecordingPhase::Recording | RecordingPhase::Stopping => {}
        RecordingPhase::Starting => {
            return Err("Wacht tot de opname is gestart om de titel te wijzigen.".into());
        }
        RecordingPhase::Finalizing => {
            return Err(
                "De opname wordt al afgerond. Hernoem de meeting daarna vanuit de bibliotheek."
                    .into(),
            );
        }
        RecordingPhase::Idle | RecordingPhase::Error => {
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

/// Runs on a worker: native capture initialization may need the application's main queue.
pub fn start(
    shared: &Arc<Shared>,
    title: String,
    detected_app: Option<String>,
    attendees: Vec<String>,
) -> Result<String, String> {
    let token = {
        let mut lifecycle = shared.recording.lock().unwrap();
        if shared.quitting.load(Ordering::SeqCst) {
            return Err("Pennel wordt afgesloten. Open de app opnieuw om verder te gaan.".into());
        }
        if shared.updating.load(Ordering::SeqCst) {
            return Err("Pennel wordt bijgewerkt. Wacht tot de app opnieuw is gestart.".into());
        }
        let token = lifecycle
            .begin(detected_app.clone())
            .map_err(|error| error.to_string())?;
        shared.recording_cancel.store(false, Ordering::SeqCst);
        shared.meeting_context.lock().unwrap().take();
        token
    };
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
            let _ = session.stop();
            let _ = shared.engine.delete_meeting(id);
            shared.meeting_context.lock().unwrap().take();
            fail(shared, &token, &error);
            return Err(error);
        }
    };
    #[cfg(windows)]
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
    if shared.recording_cancel.load(Ordering::SeqCst) {
        stop_and_wait(shared)?;
    }
    Ok(id)
}

fn take_recorder(
    shared: &Shared,
    expected_call: Option<&str>,
) -> Result<Option<(String, MeetingRecorder)>, String> {
    let mut lifecycle = shared.recording.lock().unwrap();
    if expected_call.is_some_and(|app| !lifecycle.owns_call(app)) {
        return Ok(None);
    }
    let state = lifecycle.snapshot();
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

fn finish(shared: &Shared, token: &str, recorder: MeetingRecorder) -> Result<String, String> {
    let original_title = recorder.title.clone();
    let system_audio = recorder.has_system_audio();
    let heard_system_audio = recorder.heard_system_audio();
    let audio_warning = recorder.audio_warning();
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
        context.audio_warning = audio_warning;
        context.duration_secs = Some(context.started.elapsed().as_secs());
        desired_title = context.title.clone();
    }
    shared
        .recording
        .lock()
        .unwrap()
        .finalizing(token)
        .map_err(|error| error.to_string())?;
    let result = recorder
        .stop()
        .and_then(|id| rename_after_recording(&shared.engine, id, &original_title, &desired_title));
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
