use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use transcribe_cpp::{Feature, Model, RunOptions, Session};

use crate::error::PennelError;

struct Loaded {
    path: PathBuf,
    session: Session,
    languages: Vec<String>,
    supports_vocabulary: bool,
    supports_context: bool,
    last_used: Instant,
}

/// Local speech model kept in memory between dictations and released after inactivity.
#[derive(Default)]
pub struct LocalEngine {
    loaded: Mutex<Option<Loaded>>,
}

fn engine_error(e: transcribe_cpp::Error) -> PennelError {
    PennelError::InvalidResponse {
        message: format!("lokaal model: {e}"),
    }
}

/// Only passes options the loaded model can use: an unsupported language falls back to autodetect,
/// and the dictionary goes wherever the model accepts custom terms.
pub(crate) fn run_options(
    language: Option<&str>,
    model_languages: &[String],
    vocabulary: &[String],
    supports_vocabulary: bool,
    supports_context: bool,
) -> RunOptions {
    let base = |code: &str| code.split(['-', '_']).next().unwrap_or(code).to_lowercase();
    let language = language.and_then(|lang| {
        if model_languages.is_empty() {
            return Some(lang.to_owned());
        }
        model_languages
            .iter()
            .find(|l| base(l) == base(lang))
            .cloned()
    });
    let mut options = RunOptions {
        language,
        ..Default::default()
    };
    if !vocabulary.is_empty() {
        if supports_vocabulary {
            options.vocabulary = vocabulary.to_vec();
        } else if supports_context {
            options.prompt = Some(vocabulary.join(", "));
        }
    }
    options
}

impl LocalEngine {
    #[cfg(test)]
    pub fn is_loaded(&self) -> bool {
        self.loaded.lock().unwrap().is_some()
    }

    pub fn unload_if_idle(&self, idle: Duration, now: Instant) -> bool {
        let mut loaded = self.loaded.lock().unwrap();
        match loaded.as_ref() {
            Some(model) if now.duration_since(model.last_used) >= idle => {
                *loaded = None;
                true
            }
            _ => false,
        }
    }

    pub fn transcribe(
        &self,
        model_path: &Path,
        samples: &[f32],
        language: Option<&str>,
        vocabulary: &[String],
    ) -> Result<String, PennelError> {
        if !model_path.exists() {
            return Err(PennelError::ModelNotDownloaded);
        }
        let mut loaded = self.loaded.lock().unwrap();
        if loaded.as_ref().is_none_or(|m| m.path != model_path) {
            *loaded = None;
            static LOGGING: std::sync::Once = std::sync::Once::new();
            LOGGING.call_once(transcribe_cpp::init_logging);
            let model = Model::load(model_path).map_err(engine_error)?;
            let languages = model.capabilities().map_err(engine_error)?.languages;
            let supports_vocabulary = model.supports(Feature::Vocabulary);
            let supports_context = model.supports(Feature::ContextPrompt);
            let session = model.session().map_err(engine_error)?;
            *loaded = Some(Loaded {
                path: model_path.to_owned(),
                session,
                languages,
                supports_vocabulary,
                supports_context,
                last_used: Instant::now(),
            });
        }
        let model = loaded.as_mut().expect("loaded above");
        let options = run_options(
            language,
            &model.languages,
            vocabulary,
            model.supports_vocabulary,
            model.supports_context,
        );
        let result = model.session.run(samples, &options);
        model.last_used = Instant::now();
        match result {
            Ok(transcript) => Ok(transcript.text.trim().to_owned()),
            // Repetition loops and truncation still carry the usable text decoded so far.
            Err(e) => match e.partial() {
                Some(partial) if !partial.text.trim().is_empty() => {
                    Ok(partial.text.trim().to_owned())
                }
                _ => Err(engine_error(e)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn unloaded_model_is_not_released_again() {
        let engine = LocalEngine::default();
        assert!(!engine.is_loaded());
        assert!(!engine.unload_if_idle(Duration::ZERO, Instant::now()));
    }

    #[test]
    fn missing_model_file_is_model_not_downloaded() {
        let engine = LocalEngine::default();
        let err = engine
            .transcribe(
                Path::new("/nonexistent/model.bin"),
                &[0.0; 16000],
                None,
                &[],
            )
            .unwrap_err();
        assert_eq!(err, PennelError::ModelNotDownloaded);
    }

    #[test]
    fn run_options_drops_unsupported_language() {
        assert_eq!(
            run_options(Some("nl"), &langs(&["en"]), &[], false, false).language,
            None
        );
        assert_eq!(
            run_options(Some("nl"), &langs(&["nl", "en"]), &[], false, false)
                .language
                .as_deref(),
            Some("nl")
        );
        assert_eq!(
            run_options(Some("nl"), &[], &[], false, false)
                .language
                .as_deref(),
            Some("nl")
        );
        assert_eq!(
            run_options(None, &langs(&["nl"]), &[], false, false).language,
            None
        );
        assert_eq!(
            run_options(Some("nl"), &langs(&["en-US", "nl-NL"]), &[], false, false)
                .language
                .as_deref(),
            Some("nl-NL")
        );
    }

    #[test]
    fn run_options_routes_vocabulary() {
        let terms = langs(&["Pennel"]);
        let vocab = run_options(None, &[], &terms, true, true);
        assert_eq!(vocab.vocabulary, terms);
        assert_eq!(vocab.prompt, None);
        let context = run_options(None, &[], &terms, false, true);
        assert!(context.vocabulary.is_empty());
        assert_eq!(context.prompt.as_deref(), Some("Pennel"));
        let neither = run_options(None, &[], &terms, false, false);
        assert!(neither.vocabulary.is_empty());
        assert_eq!(neither.prompt, None);
    }

    fn real_model_transcribes(file: &str) {
        let home = std::env::var("HOME").unwrap();
        let model = PathBuf::from(home)
            .join("Library/Application Support/Pennel/models")
            .join(file);
        let wav = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/hello.wav"
        ))
        .unwrap();
        let (samples, _) = crate::wav::decode_wav(&wav).unwrap();
        let engine = LocalEngine::default();
        let started = Instant::now();
        let text = engine
            .transcribe(&model, &samples, Some("en"), &[])
            .unwrap();
        eprintln!("{file} transcribed in {:?}: {text}", started.elapsed());
        assert!(text.to_lowercase().contains("test"), "{text}");
        assert!(engine.is_loaded());
        assert!(engine.unload_if_idle(Duration::ZERO, Instant::now()));
        assert!(!engine.is_loaded());
    }

    /// Downloads Parakeet v3 into the app's model folder if missing; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_parakeet_transcribes() {
        let home = std::env::var("HOME").unwrap();
        let store = crate::models::ModelStore::new(
            &PathBuf::from(home).join("Library/Application Support/Pennel"),
        );
        struct Quiet;
        impl crate::models::DownloadListener for Quiet {
            fn on_progress(&self, _: u64, _: u64) {}
        }
        let started = Instant::now();
        store
            .download(
                "parakeet-tdt-0.6b-v3",
                &Quiet,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap();
        eprintln!("download ready in {:?}", started.elapsed());
        real_model_transcribes("parakeet-tdt-0.6b-v3.gguf");
    }

    /// Needs the legacy small model in the app's model folder; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_legacy_whisper_transcribes() {
        real_model_transcribes("whisper-small-q5_1.bin");
    }
}
