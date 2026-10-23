uniffi::setup_scaffolding!();

mod cleanup;
mod engine;
mod error;
mod history;
mod http;
mod llm;
mod provider;
mod settings;
mod speech;
mod stt;
mod wav;

pub use engine::{ApiKeys, CleanupStatus, DictationRequest, DictationResult, MusliEngine};
pub use error::MusliError;
pub use history::{HistoryEntry, HistoryStatus};
pub use provider::{preset, ProviderConfig, ProviderKind, ProviderTask};
pub use settings::{default_settings, DictationMode, HotkeyTrigger, Settings, Style, StyleRule};
