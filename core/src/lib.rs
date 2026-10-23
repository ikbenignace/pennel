uniffi::setup_scaffolding!();

mod error;
mod http;
mod provider;
mod settings;
mod speech;
mod stt;
mod wav;

pub use error::MusliError;
pub use provider::{preset, ProviderConfig, ProviderKind, ProviderTask};
pub use settings::{default_settings, DictationMode, HotkeyTrigger, Settings, Style, StyleRule};
