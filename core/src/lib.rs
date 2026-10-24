uniffi::setup_scaffolding!();

mod catalog;
mod chunker;
mod cleanup;
mod cli_agent;
mod command;
mod diarize;
mod engine;
mod error;
mod export;
mod history;
mod http;
mod llm;
mod local_engine;
mod meeting_chat;
mod meeting_index;
mod meetings;
mod models;
mod provider;
mod remote_models;
mod rules;
mod session;
mod settings;
mod speech;
mod stt;
mod summary;
mod system_stt;
mod templates;
mod transcript_filter;
mod wav;

pub use engine::{ApiKeys, CleanupStatus, DictationRequest, DictationResult, PennelEngine};
pub use error::PennelError;
pub use history::{HistoryEntry, HistoryStatus};
pub use meetings::{MeetingInfo, NoteLine, Segment, Speaker, SpeakerName, mark_lines};
pub use models::{DownloadListener, LocalModel, ModelState};
pub use provider::{
    ProviderAccount, ProviderKind, ProviderTask, default_base_url, default_model, new_account,
    requires_key, supports,
};
pub use remote_models::{RemoteModel, provider_models};
pub use rules::{
    AppRule, CalendarRule, EventRule, RecordAction, browser_apps, decide_recording, known_call_apps,
};
pub use session::MeetingSession;
pub use settings::{
    DictationMode, HotkeyTrigger, ModelChoice, Settings, Style, StyleRule, default_settings,
};
pub use templates::MeetingTemplate;
