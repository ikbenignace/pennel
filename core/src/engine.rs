use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cleanup::{accept_cleanup, cleanup_messages, resolve_style, strip_wrapping};
use crate::error::MusliError;
use crate::history::{HistoryEntry, HistoryStore};
use crate::llm::chat_complete;
use crate::local_engine::LocalEngine;
use crate::meetings::{MeetingInfo, MeetingStore, Segment};
use crate::session::MeetingSession;
use crate::settings::ModelChoice;
use crate::meeting_chat::{ChatTurn, MeetingAnswer};
use crate::export::MeetingExport;
use crate::templates::{MeetingTemplate, TemplateStore};
use crate::meeting_index::{MeetingIndex, Passage, PassageKind};
use crate::models::{CancelFlag, DownloadListener, LocalModel, ModelState, ModelStore};
use crate::provider::{requires_key, ProviderAccount, ProviderConfig, ProviderKind};
use crate::settings::{DictationMode, Settings};
use crate::speech::{has_speech, is_hallucination};
use crate::stt::transcribe;
use crate::wav::{decode_wav, encode_wav};

const MAX_PROMPT_CHARS: usize = 800;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ApiKeys {
    pub by_provider: HashMap<String, String>,
}

impl ApiKeys {
    fn get(&self, account: &ProviderAccount) -> Option<&str> {
        self.by_provider.get(&account.id).map(String::as_str).filter(|k| !k.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DictationRequest {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub app_bundle_id: Option<String>,
    pub keys: ApiKeys,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CleanupStatus {
    Applied,
    Skipped,
    Rejected,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DictationResult {
    pub history_id: String,
    pub text: String,
    pub raw: String,
    pub cleanup: CleanupStatus,
}

#[derive(uniffi::Object)]
pub struct MusliEngine {
    settings_path: PathBuf,
    settings: RwLock<Settings>,
    history: Mutex<HistoryStore>,
    models: ModelStore,
    downloads: Mutex<HashMap<String, CancelFlag>>,
    local: LocalEngine,
    index: Mutex<Option<MeetingIndex>>,
}

struct Transcript {
    raw: String,
    text: String,
    cleanup: CleanupStatus,
}

/// Leading dictionary terms whose comma-joined length fits in `max_chars`.
fn capped_terms(terms: &[String], max_chars: usize) -> Vec<String> {
    let mut used = 0;
    terms
        .iter()
        .take_while(|term| {
            used += term.chars().count() + 2;
            used <= max_chars + 2
        })
        .cloned()
        .collect()
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn endpoint(account: &ProviderAccount, model: String) -> ProviderConfig {
    ProviderConfig { kind: account.kind, base_url: account.base_url.clone(), model }
}

#[uniffi::export]
impl MusliEngine {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, MusliError> {
        let dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&dir).map_err(MusliError::storage)?;
        let settings_path = dir.join("settings.json");
        let models = ModelStore::new(&dir);
        let settings = Settings::load(&settings_path)?.normalized_with(&models.downloaded());
        let history = HistoryStore::open(&dir)?;
        history.prune(settings.history_retention_days, now_ms())?;
        Ok(Arc::new(Self {
            settings_path,
            settings: RwLock::new(settings),
            history: Mutex::new(history),
            models,
            downloads: Mutex::new(HashMap::new()),
            local: LocalEngine::default(),
            index: Mutex::new(open_index(&dir.join("index.sqlite"))),
        }))
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<(), MusliError> {
        let settings = settings.normalized_with(&self.models.downloaded());
        settings.save(&self.settings_path)?;
        *self.settings.write().unwrap() = settings;
        Ok(())
    }

    pub fn dictate(&self, request: DictationRequest) -> Result<DictationResult, MusliError> {
        if !has_speech(&request.samples, request.sample_rate) {
            return Err(MusliError::NoSpeech);
        }
        let settings = self.settings();
        let (account, _) = settings.resolve(&settings.dictation_stt).ok_or(MusliError::NotFound)?;
        if requires_key(account.kind) && request.keys.get(&account).is_none() {
            return Err(MusliError::MissingApiKey { task: "stt".into() });
        }
        let wav = encode_wav(&request.samples, request.sample_rate);
        let app = request.app_bundle_id.as_deref();
        let transcript = match self.process(&settings, &wav, app, &request.keys) {
            Err(
                e @ (MusliError::Http { .. }
                | MusliError::Network { .. }
                | MusliError::InvalidResponse { .. }
                | MusliError::ModelNotDownloaded),
            ) => {
                self.history.lock().unwrap().insert_failed(app, &wav, &e.to_string(), now_ms())?;
                return Err(e);
            }
            other => other?,
        };
        let history_id =
            self.history.lock().unwrap().insert_done(app, &transcript.raw, &transcript.text, now_ms())?;
        Ok(DictationResult { history_id, text: transcript.text, raw: transcript.raw, cleanup: transcript.cleanup })
    }

    /// Speaks an instruction that rewrites `selection`, or writes new text when nothing is selected.
    pub fn command(&self, request: DictationRequest, selection: Option<String>) -> Result<DictationResult, MusliError> {
        use crate::command::{command_messages, COMMAND_TIMEOUT_SECS, MAX_SELECTION_CHARS};

        if !has_speech(&request.samples, request.sample_rate) {
            return Err(MusliError::NoSpeech);
        }
        if selection.as_ref().is_some_and(|s| s.chars().count() > MAX_SELECTION_CHARS) {
            return Err(MusliError::InvalidResponse { message: "selection too long".into() });
        }
        let settings = self.settings();
        let (llm_account, llm_model) = settings.resolve(&settings.llm).ok_or(MusliError::NotFound)?;
        let llm_key = request.keys.get(&llm_account);
        if requires_key(llm_account.kind) && llm_key.is_none() {
            return Err(MusliError::MissingApiKey { task: "llm".into() });
        }
        let wav = encode_wav(&request.samples, request.sample_rate);
        let instruction = self.stt(&settings, &settings.dictation_stt, &wav, &request.keys)?;
        if instruction.trim().is_empty() || is_hallucination(&instruction) {
            return Err(MusliError::NoSpeech);
        }
        let selection = selection.filter(|s| !s.trim().is_empty());
        let messages = command_messages(&instruction, selection.as_deref(), &settings.dictionary);
        let output = crate::llm::chat_complete_within(&endpoint(&llm_account, llm_model), llm_key, &messages, COMMAND_TIMEOUT_SECS)?;
        let text = strip_wrapping(&output);
        if text.is_empty() {
            return Err(MusliError::InvalidResponse { message: "empty command result".into() });
        }
        let app = request.app_bundle_id.as_deref();
        let history_id = self.history.lock().unwrap().insert_done(app, &instruction, &text, now_ms())?;
        Ok(DictationResult { history_id, text, raw: instruction, cleanup: CleanupStatus::Applied })
    }

    pub fn retry(&self, history_id: String, keys: ApiKeys) -> Result<DictationResult, MusliError> {
        let settings = self.settings();
        let (wav, app) = {
            let history = self.history.lock().unwrap();
            (history.failed_audio(&history_id)?, history.app_bundle_id(&history_id)?)
        };
        let transcript = self.process(&settings, &wav, app.as_deref(), &keys)?;
        self.history.lock().unwrap().mark_done(&history_id, &transcript.raw, &transcript.text)?;
        Ok(DictationResult { history_id, text: transcript.text, raw: transcript.raw, cleanup: transcript.cleanup })
    }

    pub fn history(&self, limit: u32) -> Result<Vec<HistoryEntry>, MusliError> {
        self.history.lock().unwrap().list(limit)
    }

    pub fn delete_history(&self, id: String) -> Result<(), MusliError> {
        self.history.lock().unwrap().delete(&id)
    }

    /// Called periodically by the app so an idle local model does not stay in memory.
    pub fn unload_idle_models(&self) -> bool {
        let minutes = u64::from(self.settings.read().unwrap().local_unload_minutes);
        self.local.unload_if_idle(std::time::Duration::from_secs(minutes * 60), std::time::Instant::now())
    }

    pub fn start_meeting(self: Arc<Self>, title: String, keys: ApiKeys) -> Result<Arc<MeetingSession>, MusliError> {
        self.start_meeting_with(title, Vec::new(), keys)
    }

    pub fn start_meeting_with(
        self: Arc<Self>,
        title: String,
        attendees: Vec<String>,
        keys: ApiKeys,
    ) -> Result<Arc<MeetingSession>, MusliError> {
        let store = self.meeting_store();
        let id = store.create(&title, chrono::Local::now())?;
        store.set_attendees(&id, &attendees)?;
        Ok(MeetingSession::start(self.clone(), store, id, keys))
    }

    pub fn meetings(&self) -> Vec<MeetingInfo> {
        self.meeting_store().list()
    }

    /// Title matches first, then meetings in full-text rank order.
    pub fn search_meetings(&self, query: String) -> Vec<MeetingInfo> {
        self.search_library(query).meetings
    }

    pub fn search_passages(&self, query: String) -> Vec<Passage> {
        self.search_library(query).passages
    }

    /// Matching meetings plus up to two snippets each, from one index sync.
    pub fn search_library(&self, query: String) -> LibrarySearch {
        let store = self.meeting_store();
        if query.trim().is_empty() {
            return LibrarySearch { meetings: store.list(), passages: Vec::new() };
        }
        let Some(passages) = self.with_index(&store, |index| index.search_grouped(&store, &query, 2)) else {
            return LibrarySearch { meetings: store.search(&query), passages: Vec::new() };
        };
        let needle = query.trim().to_lowercase();
        let all = store.list();
        let mut meetings: Vec<MeetingInfo> = all.iter().filter(|m| m.title.to_lowercase().contains(&needle)).cloned().collect();
        for passage in &passages {
            if !meetings.iter().any(|m| m.id == passage.meeting_id)
                && let Some(info) = all.iter().find(|m| m.id == passage.meeting_id)
            {
                meetings.push(info.clone());
            }
        }
        LibrarySearch { meetings, passages }
    }

    pub fn meeting_info(&self, id: String) -> Option<MeetingInfo> {
        self.meeting_store().info(&id)
    }

    pub fn meeting_notes(&self, id: String) -> String {
        self.meeting_store().read_notes(&id)
    }

    pub fn meeting_markdown(&self, id: String) -> Option<String> {
        self.meeting_store().read_meeting(&id)
    }

    pub fn meeting_transcript(&self, id: String) -> Vec<Segment> {
        self.meeting_store().read_transcript(&id)
    }

    pub fn meeting_dir(&self, id: String) -> String {
        self.meeting_store().dir_of(&id).to_string_lossy().into_owned()
    }

    pub fn rename_meeting(&self, id: String, title: String) -> Result<String, MusliError> {
        self.meeting_store().rename(&id, &title)
    }

    pub fn delete_meeting(&self, id: String) -> Result<(), MusliError> {
        self.meeting_store().delete(&id)
    }

    pub fn prune_meeting_audio(&self) -> u32 {
        let days = self.settings.read().unwrap().meeting_audio_retention_days;
        self.meeting_store().prune_audio(days, std::time::SystemTime::now())
    }

    /// Retries chunks whose transcription failed during the meeting; returns how many were recovered.
    pub fn retranscribe_failed(&self, id: String, keys: ApiKeys) -> Result<u32, MusliError> {
        let store = self.meeting_store();
        let settings = self.settings();
        let mut segments = store.read_transcript(&id);
        let mut recovered = 0;
        for segment in segments.iter_mut().filter(|s| s.failed) {
            let path = crate::session::failed_path(&store.dir_of(&id), segment.speaker, segment.start_ms);
            let Ok(wav) = std::fs::read(&path) else { continue };
            if let Ok(text) = self.stt(&settings, &settings.meeting_stt, &wav, &keys) {
                segment.text = if is_hallucination(&text) { String::new() } else { text };
                segment.failed = false;
                let _ = std::fs::remove_file(&path);
                recovered += 1;
            }
        }
        segments.retain(|s| s.failed || !s.text.is_empty());
        store.write_transcript(&id, &segments)?;
        Ok(recovered)
    }

    /// Writes `meeting.md`: enhanced notes when the user typed any, otherwise a summary.
    pub fn summarize_meeting(&self, id: String, template_id: String, keys: ApiKeys) -> Result<String, MusliError> {
        use crate::summary::{blocks, chunk_messages, summary_messages, transcript_text, BLOCK_CHARS, MAP_REDUCE_ABOVE};
        const SUMMARY_TIMEOUT_SECS: u64 = 180;

        let store = self.meeting_store();
        let info = store.info(&id).ok_or(MusliError::NotFound)?;
        let notes = store.read_notes(&id);
        let transcript = transcript_text(&store.read_transcript(&id));
        if transcript.trim().is_empty() && notes.trim().is_empty() {
            return Err(MusliError::NoSpeech);
        }
        let settings = self.settings();
        let (account, model) = settings.resolve(&settings.llm).ok_or(MusliError::NotFound)?;
        let key = keys.get(&account);
        if requires_key(account.kind) && key.is_none() {
            return Err(MusliError::MissingApiKey { task: "llm".into() });
        }
        let provider = endpoint(&account, model);
        let ask = |messages: Vec<crate::llm::ChatMessage>| {
            crate::llm::chat_complete_within(&provider, key, &messages, SUMMARY_TIMEOUT_SECS)
        };
        let source = if transcript.chars().count() > MAP_REDUCE_ABOVE {
            blocks(&transcript, BLOCK_CHARS)
                .iter()
                .map(|part| ask(chunk_messages(part)))
                .collect::<Result<Vec<_>, _>>()?
                .join("\n\n")
        } else {
            transcript
        };
        let spec = self.template_store().spec(&template_id);
        let body = ask(summary_messages(&spec, &notes, &source))?;
        store.write_meeting(&id, info.duration_min, &template_id, &body)?;
        Ok(body.trim().to_owned())
    }

    /// Answers from one meeting (`meeting_id`) or all meetings, citing the passages it used.
    pub fn ask_meetings(
        &self,
        question: String,
        history: Vec<ChatTurn>,
        meeting_id: Option<String>,
        keys: ApiKeys,
    ) -> Result<MeetingAnswer, MusliError> {
        use crate::meeting_chat::*;

        let settings = self.settings();
        let (account, model) = settings.resolve(&settings.llm).ok_or(MusliError::NotFound)?;
        let key = keys.get(&account);
        if requires_key(account.kind) && key.is_none() {
            return Err(MusliError::MissingApiKey { task: "llm".into() });
        }
        let store = self.meeting_store();
        let scope = meeting_id.as_deref();
        let terms = search_terms(&question);
        let sources = self
            .with_index(&store, |index| {
                if let Some(id) = scope {
                    let all = index.recent(&store, Some(id), u32::MAX)?;
                    if all.iter().map(|p| p.text.len()).sum::<usize>() <= WHOLE_MEETING_CHARS {
                        return Ok(all);
                    }
                    let mut picked: Vec<Passage> =
                        all.iter().filter(|p| p.kind == PassageKind::Summary).cloned().collect();
                    let found = index.search_any(&store, &terms, Some(id), MEETING_PASSAGES)?;
                    picked.extend(if found.is_empty() { index.recent(&store, Some(id), MEETING_PASSAGES)? } else { found });
                    return Ok(picked);
                }
                let found = index.search_any(&store, &terms, None, ALL_MEETINGS_PASSAGES)?;
                if found.is_empty() { index.recent(&store, None, ALL_MEETINGS_PASSAGES) } else { Ok(found) }
            })
            .ok_or(MusliError::Storage { message: "zoekindex niet beschikbaar".into() })?;
        let answer = crate::llm::chat_complete_within(
            &endpoint(&account, model),
            key,
            &chat_messages(&question, &history, &sources),
            ASK_TIMEOUT_SECS,
        )?;
        Ok(MeetingAnswer { citations: cited(&answer, &sources), text: answer })
    }

    pub fn export_meeting(&self, id: String, include_transcript: bool) -> Result<MeetingExport, MusliError> {
        let (title, markdown) =
            crate::export::export_markdown(&self.meeting_store(), &id, include_transcript).ok_or(MusliError::NotFound)?;
        Ok(MeetingExport { html: crate::export::markdown_to_html(&markdown), title, markdown })
    }

    pub fn meeting_templates(&self) -> Vec<MeetingTemplate> {
        self.template_store().list()
    }

    pub fn template_source(&self, id: String) -> Option<String> {
        self.template_store().source(&id)
    }

    pub fn save_template(&self, id: Option<String>, name: String, body: String) -> Result<String, MusliError> {
        self.template_store().save(id.as_deref(), &name, &body)
    }

    pub fn delete_template(&self, id: String) -> Result<(), MusliError> {
        self.template_store().delete(&id)
    }

    pub fn local_models(&self, language: Option<String>, supported_only: bool) -> Vec<LocalModel> {
        self.models.local_models(language.as_deref(), supported_only)
    }

    pub fn model_state(&self, id: String) -> ModelState {
        self.models.state(&id)
    }

    /// Blocks until the download finishes, fails or is cancelled via `cancel_download`.
    pub fn download_model(&self, id: String, listener: Arc<dyn DownloadListener>) -> Result<(), MusliError> {
        let cancel = CancelFlag::default();
        self.downloads.lock().unwrap().insert(id.clone(), cancel.clone());
        let result = self.models.download(&id, listener.as_ref(), &cancel);
        self.downloads.lock().unwrap().remove(&id);
        result
    }

    pub fn cancel_download(&self, id: String) {
        if let Some(flag) = self.downloads.lock().unwrap().get(&id) {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Also moves dictation off the deleted model when another local model is available.
    pub fn delete_model(&self, id: String) -> Result<(), MusliError> {
        self.models.delete(&id)?;
        let settings = self.settings();
        self.update_settings(settings)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LibrarySearch {
    pub meetings: Vec<MeetingInfo>,
    pub passages: Vec<Passage>,
}

fn open_index(path: &std::path::Path) -> Option<MeetingIndex> {
    MeetingIndex::open(path).ok().or_else(|| {
        let _ = std::fs::remove_file(path);
        MeetingIndex::open(path).ok()
    })
}

impl MusliEngine {
    /// Syncs the index with the meeting files first; `None` when the index is unavailable.
    pub(crate) fn with_index<T>(
        &self,
        store: &MeetingStore,
        query: impl FnOnce(&MeetingIndex) -> Result<T, MusliError>,
    ) -> Option<T> {
        let mut guard = self.index.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let index = guard.as_mut()?;
        index.sync(store).ok()?;
        query(index).ok()
    }

    fn process(&self, settings: &Settings, wav: &[u8], app: Option<&str>, keys: &ApiKeys) -> Result<Transcript, MusliError> {
        let raw = self.stt(settings, &settings.dictation_stt, wav, keys)?;
        if is_hallucination(&raw) {
            return Err(MusliError::NoSpeech);
        }
        let (text, cleanup) = match settings.mode {
            DictationMode::Fast => (raw.clone(), CleanupStatus::Skipped),
            DictationMode::Clean => clean(settings, &raw, app, keys),
        };
        Ok(Transcript { raw, text, cleanup })
    }

    /// Speech-to-text for one WAV via the provider in `choice`, cloud or local.
    pub(crate) fn stt(
        &self,
        settings: &Settings,
        choice: &ModelChoice,
        wav: &[u8],
        keys: &ApiKeys,
    ) -> Result<String, MusliError> {
        let prompt: Option<String> = (!settings.dictionary.is_empty())
            .then(|| settings.dictionary.join(", ").chars().take(MAX_PROMPT_CHARS).collect());
        let (account, model) = settings.resolve(choice).ok_or(MusliError::NotFound)?;
        if account.kind == ProviderKind::Local {
            let path = self.models.path(&model).ok_or(MusliError::ModelNotDownloaded)?;
            let (samples, _) = decode_wav(wav).ok_or(MusliError::InvalidResponse { message: "bad audio".into() })?;
            let vocabulary = capped_terms(&settings.dictionary, MAX_PROMPT_CHARS);
            return self.local.transcribe(&path, &samples, settings.language.as_deref(), &vocabulary);
        }
        let key = keys.get(&account);
        if requires_key(account.kind) && key.is_none() {
            return Err(MusliError::MissingApiKey { task: "stt".into() });
        }
        transcribe(&endpoint(&account, model), key, wav, prompt.as_deref(), settings.language.as_deref())
    }

    fn template_store(&self) -> TemplateStore {
        TemplateStore::new(&self.meeting_store().root())
    }

    pub(crate) fn meeting_store(&self) -> MeetingStore {
        let configured = self.settings.read().unwrap().meetings_dir.clone();
        let dir = if configured.is_empty() {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Musli")
        } else {
            PathBuf::from(configured)
        };
        MeetingStore::new(&dir)
    }
}

fn clean(settings: &Settings, raw: &str, app: Option<&str>, keys: &ApiKeys) -> (String, CleanupStatus) {
    let Some((account, model)) = settings.resolve(&settings.llm) else {
        return (raw.to_owned(), CleanupStatus::Failed { message: "no llm provider".into() });
    };
    let key = keys.get(&account);
    if requires_key(account.kind) && key.is_none() {
        return (raw.to_owned(), CleanupStatus::Failed { message: "missing llm key".into() });
    }
    let style = resolve_style(app, &settings.style_rules);
    match chat_complete(&endpoint(&account, model), key, &cleanup_messages(raw, style, &settings.dictionary)) {
        Ok(output) => {
            let cleaned = strip_wrapping(&output);
            if accept_cleanup(raw, &cleaned) {
                (cleaned, CleanupStatus::Applied)
            } else {
                (raw.to_owned(), CleanupStatus::Rejected)
            }
        }
        Err(e) => (raw.to_owned(), CleanupStatus::Failed { message: e.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryStatus;
    use mockito::Matcher;

    fn speech() -> Vec<f32> {
        (0..16000).map(|i| 0.2 * (i as f32 * 440.0 * std::f32::consts::TAU / 16000.0).sin()).collect()
    }

    fn keys() -> ApiKeys {
        ApiKeys { by_provider: HashMap::from([("groq".to_owned(), "k".to_owned())]) }
    }

    fn request(samples: Vec<f32>, app: Option<&str>) -> DictationRequest {
        DictationRequest { samples, sample_rate: 16000, app_bundle_id: app.map(str::to_owned), keys: keys() }
    }

    fn engine(dir: &tempfile::TempDir, server: &mockito::Server, mode: DictationMode) -> Arc<MusliEngine> {
        let engine = MusliEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.providers[0].base_url = server.url();
        settings.mode = mode;
        engine.update_settings(settings).unwrap();
        engine
    }

    fn stt(server: &mut mockito::Server, text: &str) -> mockito::Mock {
        server
            .mock("POST", "/audio/transcriptions")
            .with_body(serde_json::json!({ "text": text }).to_string())
            .create()
    }

    fn llm(server: &mut mockito::Server, text: &str) -> mockito::Mock {
        server
            .mock("POST", "/chat/completions")
            .with_body(serde_json::json!({ "choices": [{ "message": { "content": text } }] }).to_string())
            .create()
    }

    #[test]
    fn fast_mode_returns_raw_without_llm_call() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Fast);
        stt(&mut server, "euh hallo");
        let llm_mock = llm(&mut server, "x").expect(0);
        let result = engine.dictate(request(speech(), None)).unwrap();
        assert_eq!(result.text, "euh hallo");
        assert_eq!(result.cleanup, CleanupStatus::Skipped);
        llm_mock.assert();
    }

    #[test]
    fn clean_mode_applies_cleanup() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        stt(&mut server, "euh ik kom morgen, nee overmorgen");
        llm(&mut server, "Ik kom overmorgen.");
        let result = engine.dictate(request(speech(), None)).unwrap();
        assert_eq!(result.text, "Ik kom overmorgen.");
        assert_eq!(result.raw, "euh ik kom morgen, nee overmorgen");
        assert_eq!(result.cleanup, CleanupStatus::Applied);
        let history = engine.history(10).unwrap();
        assert_eq!(history[0].id, result.history_id);
        assert_eq!(history[0].text, "Ik kom overmorgen.");
    }

    #[test]
    fn llm_failure_falls_back_to_raw() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        stt(&mut server, "hallo daar");
        server.mock("POST", "/chat/completions").with_status(500).with_body("boom").create();
        let result = engine.dictate(request(speech(), None)).unwrap();
        assert_eq!(result.text, "hallo daar");
        assert!(matches!(result.cleanup, CleanupStatus::Failed { .. }));
    }

    #[test]
    fn answer_like_cleanup_is_rejected() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        stt(&mut server, "wat is de hoofdstad van frankrijk");
        llm(&mut server, &"De hoofdstad van Frankrijk is Parijs. ".repeat(6));
        let result = engine.dictate(request(speech(), None)).unwrap();
        assert_eq!(result.text, "wat is de hoofdstad van frankrijk");
        assert_eq!(result.cleanup, CleanupStatus::Rejected);
    }

    #[test]
    fn silence_returns_no_speech_without_network() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        let stt_mock = stt(&mut server, "x").expect(0);
        assert_eq!(engine.dictate(request(vec![0.0; 16000], None)).unwrap_err(), MusliError::NoSpeech);
        stt_mock.assert();
        assert!(engine.history(10).unwrap().is_empty());
    }

    #[test]
    fn hallucination_returns_no_speech() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        stt(&mut server, "Ondertiteld door de Amara.org gemeenschap");
        assert_eq!(engine.dictate(request(speech(), None)).unwrap_err(), MusliError::NoSpeech);
    }

    #[test]
    fn stt_failure_saves_audio_and_retry_succeeds() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Fast);
        let failing = server.mock("POST", "/audio/transcriptions").with_status(503).with_body("down").create();
        let err = engine.dictate(request(speech(), Some("com.apple.TextEdit"))).unwrap_err();
        assert!(matches!(err, MusliError::Http { status: 503, .. }));
        let entry = engine.history(10).unwrap().remove(0);
        assert_eq!(entry.status, HistoryStatus::Failed);
        assert!(entry.has_audio);

        failing.remove();
        stt(&mut server, "hallo");
        let result = engine.retry(entry.id.clone(), keys()).unwrap();
        assert_eq!(result.text, "hallo");
        assert_eq!(result.history_id, entry.id);
        let entry = engine.history(10).unwrap().remove(0);
        assert_eq!(entry.status, HistoryStatus::Done);
        assert_eq!(entry.text, "hallo");
    }

    #[test]
    fn missing_stt_key_errors_before_network() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Fast);
        let stt_mock = stt(&mut server, "x").expect(0);
        let mut req = request(speech(), None);
        req.keys.by_provider.clear();
        assert_eq!(engine.dictate(req).unwrap_err(), MusliError::MissingApiKey { task: "stt".into() });
        stt_mock.assert();
    }

    #[test]
    fn custom_provider_without_key_sends_no_auth() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Fast);
        let mut settings = engine.settings();
        let mut custom = crate::provider::new_account(ProviderKind::Custom, settings.providers.clone());
        custom.base_url = server.url();
        settings.dictation_stt = crate::settings::ModelChoice { provider_id: custom.id.clone(), model: "w".into() };
        settings.providers.push(custom);
        engine.update_settings(settings).unwrap();
        let mock = server
            .mock("POST", "/audio/transcriptions")
            .match_header("authorization", Matcher::Missing)
            .with_body(r#"{"text":"lokaal"}"#)
            .create();
        let mut req = request(speech(), None);
        req.keys.by_provider.clear();
        assert_eq!(engine.dictate(req).unwrap().text, "lokaal");
        mock.assert();
    }

    #[test]
    fn local_without_model_is_model_not_downloaded() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Fast);
        let mut settings = engine.settings();
        settings.dictation_stt =
            crate::settings::ModelChoice { provider_id: "local".into(), model: "whisper-small-q5_1".into() };
        engine.update_settings(settings).unwrap();
        assert_eq!(engine.dictate(request(speech(), None)).unwrap_err(), MusliError::ModelNotDownloaded);
        let entry = engine.history(1).unwrap().remove(0);
        assert_eq!(entry.status, HistoryStatus::Failed);
        assert!(entry.has_audio);
    }

    struct Silent;
    impl crate::models::DownloadListener for Silent {
        fn on_progress(&self, _: u64, _: u64) {}
    }

    #[test]
    fn engine_model_api_reports_state_and_unknown_ids() {
        let dir = tempfile::tempdir().unwrap();
        let engine = MusliEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(engine.model_state("whisper-small-q5_1".into()), crate::models::ModelState::NotDownloaded);
        assert_eq!(engine.download_model("nope".into(), Arc::new(Silent)).unwrap_err(), MusliError::NotFound);
        engine.cancel_download("whisper-small-q5_1".into());
        engine.delete_model("whisper-small-q5_1".into()).unwrap();
    }

    #[test]
    fn unload_idle_models_without_loaded_model_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let engine = MusliEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(!engine.unload_idle_models());
    }

    #[test]
    fn style_follows_app_bundle() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        stt(&mut server, "hoi");
        let llm_mock = server
            .mock("POST", "/chat/completions")
            .match_body(Matcher::Regex("casual chat message".into()))
            .with_body(r#"{"choices":[{"message":{"content":"Hoi"}}]}"#)
            .create();
        engine.dictate(request(speech(), Some("com.tinyspeck.slackmacgap"))).unwrap();
        llm_mock.assert();
    }

    #[test]
    fn dictionary_is_sent_as_stt_prompt() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Fast);
        let mut settings = engine.settings();
        settings.dictionary = vec!["Musli".into(), "Q8".into()];
        engine.update_settings(settings).unwrap();
        let stt_mock = server
            .mock("POST", "/audio/transcriptions")
            .match_body(Matcher::Regex("name=\"prompt\"\r\n\r\nMusli, Q8".into()))
            .with_body(r#"{"text":"ok"}"#)
            .create();
        engine.dictate(request(speech(), None)).unwrap();
        stt_mock.assert();
    }

    #[test]
    fn settings_persist_across_engines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let engine = MusliEngine::new(path.clone()).unwrap();
        let mut settings = engine.settings();
        settings.mode = DictationMode::Fast;
        engine.update_settings(settings).unwrap();
        assert_eq!(MusliEngine::new(path).unwrap().settings().mode, DictationMode::Fast);
    }

    #[test]
    fn local_vocabulary_is_capped() {
        let terms: Vec<String> = (0..300).map(|i| format!("term{i:03}")).collect();
        let capped = capped_terms(&terms, MAX_PROMPT_CHARS);
        assert!(capped.join(", ").chars().count() <= MAX_PROMPT_CHARS);
        assert_eq!(capped[0], "term000");
        assert!(capped.len() > 50);
        assert_eq!(capped_terms(&terms[..2], MAX_PROMPT_CHARS), terms[..2]);
    }

    fn meeting_engine(dir: &tempfile::TempDir, server: &mockito::Server) -> Arc<MusliEngine> {
        let engine = engine(dir, server, DictationMode::Clean);
        let mut settings = engine.settings();
        settings.meetings_dir = dir.path().join("meetings").to_string_lossy().into_owned();
        engine.update_settings(settings).unwrap();
        engine
    }

    fn add_meeting(engine: &MusliEngine, title: &str, hour: u32, said: &str) -> String {
        use chrono::TimeZone;
        let store = engine.meeting_store();
        std::fs::create_dir_all(engine.settings().meetings_dir).unwrap();
        let id = store.create(title, chrono::Local.with_ymd_and_hms(2026, 10, 20, hour, 0, 0).unwrap()).unwrap();
        let segment = crate::meetings::Segment {
            start_ms: 1000,
            end_ms: 2000,
            speaker: crate::meetings::Speaker::Others,
            text: said.into(),
            failed: false,
        };
        store.write_transcript(&id, &[segment]).unwrap();
        id
    }

    #[test]
    fn search_meetings_uses_titles_then_full_text() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = meeting_engine(&dir, &server);
        let a = add_meeting(&engine, "Budget review", 9, "niets bijzonders");
        let b = add_meeting(&engine, "Weekly", 10, "het budget is goedgekeurd");
        add_meeting(&engine, "Lunch", 11, "pasta");
        let ids: Vec<String> = engine.search_meetings("budget".into()).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, [a, b]);
        let hits = engine.search_passages("goedgekeurd".into());
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("«goedgekeurd»"));
    }

    #[test]
    fn no_terms_uses_recent_passages() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = meeting_engine(&dir, &server);
        add_meeting(&engine, "Oud", 8, "oude kiwi");
        add_meeting(&engine, "Nieuw", 11, "verse mango");
        let mock = server
            .mock("POST", "/chat/completions")
            .match_body(mockito::Matcher::Regex("verse mango".into()))
            .with_body(r#"{"choices":[{"message":{"content":"Mango [1]."}}]}"#)
            .create();
        let answer = engine.ask_meetings("wat?".into(), vec![], None, keys()).unwrap();
        mock.assert();
        assert_eq!(answer.text, "Mango [1].");
        assert_eq!(answer.citations.len(), 1);
        assert_eq!(answer.citations[0].passage.title, "Nieuw");
    }

    #[test]
    fn single_meeting_sends_whole_meeting() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = meeting_engine(&dir, &server);
        let id = add_meeting(&engine, "Sync", 9, "we verhuizen naar Gent");
        add_meeting(&engine, "Ander", 10, "geheime ananas");
        engine.meeting_store().write_meeting(&id, 30, "algemeen", "## Besluiten\n- Verhuis naar Gent").unwrap();
        let mock = server
            .mock("POST", "/chat/completions")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex("we verhuizen naar Gent".into()),
                mockito::Matcher::Regex("Besluiten".into()),
            ]))
            .with_body(r#"{"choices":[{"message":{"content":"Gent."}}]}"#)
            .create();
        let other = server.mock("POST", "/chat/completions").match_body(mockito::Matcher::Regex("ananas".into())).expect(0).create();
        let answer = engine.ask_meetings("Waar verhuizen we naartoe?".into(), vec![], Some(id), keys()).unwrap();
        mock.assert();
        other.assert();
        assert!(answer.citations.is_empty());
    }

    #[test]
    fn command_output_is_unwrapped_and_recorded() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        stt(&mut server, "maak hier een mail van");
        let mock = server
            .mock("POST", "/chat/completions")
            .match_body(mockito::Matcher::Regex("selection-[0-9a-f]{8}>.{2}ik ben later".into()))
            .with_body(r#"{"choices":[{"message":{"content":"\"Beste Jan, ik ben later.\""}}]}"#)
            .create();
        let result = engine.command(request(speech(), Some("com.apple.mail")), Some("ik ben later".into())).unwrap();
        mock.assert();
        assert_eq!(result.text, "Beste Jan, ik ben later.");
        assert_eq!(result.raw, "maak hier een mail van");
        let entry = &engine.history(1).unwrap()[0];
        assert_eq!((entry.raw.as_str(), entry.text.as_str()), ("maak hier een mail van", "Beste Jan, ik ben later."));
    }

    #[test]
    fn command_without_speech_is_no_speech() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        let calls = server.mock("POST", mockito::Matcher::Any).expect(0).create();
        let err = engine.command(request(vec![0.0; 16000], None), Some("tekst".into())).unwrap_err();
        assert_eq!(err, MusliError::NoSpeech);
        calls.assert();
    }

    #[test]
    fn empty_instruction_or_result_pastes_nothing() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        let stt_mock = stt(&mut server, "  ");
        let llm_mock = llm(&mut server, "x").expect(0);
        assert_eq!(engine.command(request(speech(), None), Some("tekst".into())).unwrap_err(), MusliError::NoSpeech);
        stt_mock.remove();
        llm_mock.assert();
        llm_mock.remove();
        stt(&mut server, "maak korter");
        llm(&mut server, "\"\"");
        assert!(matches!(engine.command(request(speech(), None), Some("tekst".into())), Err(MusliError::InvalidResponse { .. })));
    }

    #[test]
    fn command_rejects_huge_selection() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server, DictationMode::Clean);
        let huge = "x".repeat(crate::command::MAX_SELECTION_CHARS + 1);
        assert!(matches!(engine.command(request(speech(), None), Some(huge)), Err(MusliError::InvalidResponse { .. })));
    }
}
