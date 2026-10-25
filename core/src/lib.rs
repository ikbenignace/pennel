uniffi::setup_scaffolding!();

mod call_detector;
mod catalog;
mod chunker;
mod cleanup;
mod cli_agent;
mod command;
mod diarize;
mod dictation_pad;
mod engine;
mod error;
mod export;
mod gesture;
mod history;
mod http;
mod live_dictation;
mod llm;
mod local_engine;
mod meeting_chat;
mod meeting_index;
mod meetings;
mod models;
mod paste_queue;
mod provider;
mod recording;
mod remote_models;
mod rules;
mod session;
mod settings;
mod setup;
mod speech;
mod status;
mod storage;
mod stt;
mod summary;
mod system_llm;
mod system_stt;
mod templates;
mod transcript_filter;
mod wav;

pub use call_detector::{CallDetector, CallEvent, CallObservation};
pub use cli_agent::{DetectedAgent, check_cli_agent, detect_cli_agents};
pub use engine::{
    ApiKeys, CleanupStatus, DictationRequest, DictationResult, LibrarySearch, PennelEngine,
};
pub use error::PennelError;
pub use export::MeetingExport;
pub use gesture::{GestureAction, HotkeyGesture, KeyEdge};
pub use history::{HistoryEntry, HistoryStatus};
pub use live_dictation::{
    DictationPreviewMode, DictationPreviewObserver, DictationPreviewUpdate, LiveDictationSession,
};
pub use meeting_chat::{ChatTurn, Citation, MeetingAnswer};
pub use meeting_index::{Passage, PassageKind};
pub use meetings::{MeetingInfo, NoteLine, Segment, Speaker, SpeakerName, mark_lines};
pub use models::{DownloadListener, LocalModel, ModelState};
pub use paste_queue::PasteQueue;
pub use provider::{
    ProviderAccount, ProviderKind, ProviderTask, default_base_url, default_model, new_account,
    requires_key, supports,
};
pub use recording::{RecordingLifecycle, RecordingPhase, RecordingState};
pub use remote_models::{RemoteModel, provider_models};
pub use rules::{
    AppRule, CalendarRule, EventRule, RecordAction, browser_apps, consent_sentence,
    decide_recording, known_call_apps,
};
pub use session::MeetingSession;
pub use settings::{
    AppearancePreference, DictationMode, HotkeyTrigger, ModelChoice, OnboardingProgress,
    OnboardingStatus, OnboardingStep, Settings, Style, StyleRule, default_settings,
};
pub use setup::{SetupAccess, SetupFacts, SetupReadiness, dictation_readiness, meeting_readiness};
pub use status::{CommandIntent, DictationStatus, PillState};
pub use system_llm::{LanguageModelMessage, SystemLanguageModel};
pub use system_stt::SystemTranscriber;
pub use templates::MeetingTemplate;
