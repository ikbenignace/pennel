use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::MusliError;

struct Loaded {
    path: PathBuf,
    context: WhisperContext,
    last_used: Instant,
}

/// whisper.cpp model kept in memory between dictations and released after inactivity.
#[derive(Default)]
pub struct LocalWhisper {
    loaded: Mutex<Option<Loaded>>,
}

impl LocalWhisper {
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
        prompt: Option<&str>,
    ) -> Result<String, MusliError> {
        if !model_path.exists() {
            return Err(MusliError::ModelNotDownloaded);
        }
        let mut loaded = self.loaded.lock().unwrap();
        if loaded.as_ref().is_none_or(|m| m.path != model_path) {
            let path = model_path.to_str().ok_or(MusliError::ModelNotDownloaded)?;
            let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
                .map_err(|e| MusliError::InvalidResponse { message: format!("model laden mislukt: {e}") })?;
            *loaded = Some(Loaded { path: model_path.to_owned(), context, last_used: Instant::now() });
        }
        let model = loaded.as_mut().expect("loaded above");

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(language.unwrap_or("auto")));
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
        params.set_n_threads(threads as i32);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_no_timestamps(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        if let Some(prompt) = prompt {
            params.set_initial_prompt(prompt);
        }

        let mut state = model
            .context
            .create_state()
            .map_err(|e| MusliError::InvalidResponse { message: e.to_string() })?;
        state
            .full(params, samples)
            .map_err(|e| MusliError::InvalidResponse { message: e.to_string() })?;
        let text: String = state.as_iter().map(|segment| segment.to_str_lossy().unwrap_or_default().into_owned()).collect();
        model.last_used = Instant::now();
        Ok(text.trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unloaded_model_is_not_released_again() {
        let whisper = LocalWhisper::default();
        assert!(!whisper.is_loaded());
        assert!(!whisper.unload_if_idle(Duration::ZERO, Instant::now()));
    }

    #[test]
    fn missing_model_file_is_model_not_downloaded() {
        let whisper = LocalWhisper::default();
        let err = whisper.transcribe(Path::new("/nonexistent/model.bin"), &[0.0; 16000], None, None).unwrap_err();
        assert_eq!(err, MusliError::ModelNotDownloaded);
    }

    /// Needs the small model in the app's model folder; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_small_model_transcribes() {
        let home = std::env::var("HOME").unwrap();
        let model = PathBuf::from(home).join("Library/Application Support/Musli/models/whisper-small-q5_1.bin");
        let wav = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hello.wav")).unwrap();
        let (samples, _) = crate::wav::decode_wav(&wav).unwrap();
        let whisper = LocalWhisper::default();
        let started = Instant::now();
        let text = whisper.transcribe(&model, &samples, None, None).unwrap();
        eprintln!("transcribed in {:?}: {text}", started.elapsed());
        assert!(text.to_lowercase().contains("test"), "{text}");
        assert!(whisper.is_loaded());
        assert!(whisper.unload_if_idle(Duration::ZERO, Instant::now()));
        assert!(!whisper.is_loaded());
    }
}
