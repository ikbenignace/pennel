uniffi::setup_scaffolding!();

mod chunker;
mod cleanup;
mod engine;
mod error;
mod history;
mod http;
mod llm;
mod meetings;
mod local_stt;
mod models;
mod provider;
mod session;
mod settings;
mod speech;
mod stt;
mod wav;

pub use engine::{ApiKeys, CleanupStatus, DictationRequest, DictationResult, MusliEngine};
pub use error::MusliError;
pub use history::{HistoryEntry, HistoryStatus};
pub use session::MeetingSession;
pub use meetings::{mark_lines, MeetingInfo, NoteLine, Segment, Speaker};
pub use models::{local_models, DownloadListener, LocalModel, ModelState};
pub use provider::{default_base_url, default_model, new_account, requires_key, supports, ProviderAccount, ProviderKind, ProviderTask};
pub use settings::{default_settings, DictationMode, HotkeyTrigger, ModelChoice, Settings, Style, StyleRule};
