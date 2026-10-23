use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cleanup::{accept_cleanup, cleanup_messages, resolve_style, strip_wrapping};
use crate::error::MusliError;
use crate::history::{HistoryEntry, HistoryStore};
use crate::llm::chat_complete;
use crate::provider::{requires_key, ProviderAccount, ProviderConfig, ProviderKind};
use crate::settings::{DictationMode, Settings};
use crate::speech::{has_speech, is_hallucination};
use crate::stt::transcribe;
use crate::wav::encode_wav;

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
}

struct Transcript {
    raw: String,
    text: String,
    cleanup: CleanupStatus,
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
        let settings = Settings::load(&settings_path)?;
        let history = HistoryStore::open(&dir)?;
        history.prune(settings.history_retention_days, now_ms())?;
        Ok(Arc::new(Self { settings_path, settings: RwLock::new(settings), history: Mutex::new(history) }))
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn update_settings(&self, settings: Settings) -> Result<(), MusliError> {
        let settings = settings.normalized();
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
}

impl MusliEngine {
    fn process(&self, settings: &Settings, wav: &[u8], app: Option<&str>, keys: &ApiKeys) -> Result<Transcript, MusliError> {
        let prompt: Option<String> = (!settings.dictionary.is_empty())
            .then(|| settings.dictionary.join(", ").chars().take(MAX_PROMPT_CHARS).collect());
        let (account, model) = settings.resolve(&settings.dictation_stt).ok_or(MusliError::NotFound)?;
        if account.kind == ProviderKind::Local {
            return Err(MusliError::ModelNotDownloaded);
        }
        let raw = transcribe(
            &endpoint(&account, model),
            keys.get(&account),
            wav,
            prompt.as_deref(),
            settings.language.as_deref(),
        )?;
        if is_hallucination(&raw) {
            return Err(MusliError::NoSpeech);
        }
        let (text, cleanup) = match settings.mode {
            DictationMode::Fast => (raw.clone(), CleanupStatus::Skipped),
            DictationMode::Clean => clean(settings, &raw, app, keys),
        };
        Ok(Transcript { raw, text, cleanup })
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
}
