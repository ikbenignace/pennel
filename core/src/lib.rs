uniffi::setup_scaffolding!();

mod call_detector;
mod catalog;
mod chunker;
mod claude_access;
mod cleanup;
mod cli_agent;
mod cloud;
mod command;
mod cpu_support;
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
mod localization;
mod meeting_chat;
mod meeting_index;
mod meetings;
mod models;
mod paste_queue;
mod provider;
mod provider_test;
mod recording;
mod remote_models;
mod retranscription;
mod rules;
mod session;
mod settings;
mod setup;
mod speakers;
mod speech;
mod status;
mod storage;
mod stt;
mod summary;
mod system_llm;
mod system_stt;
mod templates;
mod transcript_filter;
mod voices;
mod wav;

pub use call_detector::{CallDetector, CallEvent, CallObservation};
pub use cli_agent::{DetectedAgent, check_cli_agent, detect_cli_agents};
pub use cloud::SecretStore;
pub use cloud::account::{CloudAccount, CloudDevice, CloudLoginPoll, CloudLoginStart};
pub use cloud::relay::{RelayAnswer, RelayRequest};
pub use cloud::sync::{MASS_DELETE, MeetingSyncState, SyncItemState, SyncReport, SyncStatus};
pub use cpu_support::{UNSUPPORTED_PROCESSOR, local_models_unavailable_reason};
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
pub use localization::{
    InterfaceLanguage, InterfaceLanguagePreference, localize_ui_text, resolve_interface_language,
    ui_text,
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
pub use provider_test::{ProviderTaskTestResult, test_provider_task};
pub use recording::{RecordingLifecycle, RecordingPhase, RecordingState};
pub use remote_models::{RemoteModel, provider_models};
pub use retranscription::{
    MeetingRetranscriptionPlan, MeetingRetranscriptionStatus, RetranscriptionPhase,
    RetranscriptionProgress,
};
pub use rules::{
    AppRule, CalendarRule, EventRule, RecordAction, browser_apps, consent_sentence,
    decide_recording, known_call_apps,
};
pub use session::{MeetingHealth, MeetingRecognition, MeetingSession};
pub use settings::{
    AppearancePreference, DictationMode, HotkeyTrigger, ModelChoice, OnboardingGoal,
    OnboardingProgress, OnboardingStatus, OnboardingStep, Settings, SpeakerRecognitionMode, Style,
    StyleRule, default_settings,
};
pub use setup::{SetupAccess, SetupFacts, SetupReadiness, dictation_readiness, meeting_readiness};
pub use speakers::{MeetingVoice, SpeakerRecognition};
pub use status::{CommandIntent, DictationStatus, PillState};
pub use system_llm::{LanguageModelMessage, SystemLanguageModel};
pub use system_stt::SystemTranscriber;
pub use templates::MeetingTemplate;
pub use voices::{KnownVoice, VoiceProfile};
