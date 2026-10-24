use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use pennel_core::{ApiKeys, PennelEngine, Settings};

use crate::dictation::Msg;
use crate::hotkey::Listener;
use crate::meeting::MeetingRecorder;
use crate::secrets::Secrets;

pub struct Shared {
    pub engine: Arc<PennelEngine>,
    pub secrets: Secrets,
    pub data_dir: PathBuf,
    pub dictation: Sender<Msg>,
    pub hotkey: Mutex<Option<Listener>>,
    pub hotkey_error: Mutex<Option<String>>,
    pub meeting: Mutex<Option<MeetingRecorder>>,
}

impl Shared {
    pub fn settings(&self) -> Settings {
        self.engine.settings()
    }

    pub fn keys(&self) -> ApiKeys {
        self.secrets.keys(&self.engine.settings().providers)
    }

    pub fn set_meeting_dictating(&self, dictating: bool) {
        if let Some(meeting) = self.meeting.lock().unwrap().as_ref() {
            meeting.set_dictating(dictating);
        }
    }
}
