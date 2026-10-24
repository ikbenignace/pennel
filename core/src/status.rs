//! Port of `PennelKit/DictationStatus.swift`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum PillState {
    Hidden,
    Listening,
    HandsFree,
    Command,
    Working,
    Error(String),
}

/// Derives what the pill shows from everything in flight: an active recording wins,
/// then a short-lived notice, then pending dictations.
#[derive(Debug, Default)]
pub struct DictationStatus {
    recording: Option<PillState>,
    in_flight: u32,
    notice: Option<String>,
}

impl DictationStatus {
    pub fn pill(&self) -> PillState {
        if let Some(recording) = &self.recording {
            return recording.clone();
        }
        if let Some(notice) = &self.notice {
            return PillState::Error(notice.clone());
        }
        if self.in_flight > 0 {
            PillState::Working
        } else {
            PillState::Hidden
        }
    }

    pub fn recording_started(&mut self, hands_free: bool) {
        self.recording = Some(if hands_free {
            PillState::HandsFree
        } else {
            PillState::Listening
        });
    }

    pub fn mark_command(&mut self) {
        if self.recording.is_some() {
            self.recording = Some(PillState::Command);
        }
    }

    pub fn recording_cancelled(&mut self) {
        self.recording = None;
    }

    pub fn recording_submitted(&mut self) {
        self.recording = None;
        self.in_flight += 1;
    }

    pub fn dictation_finished(&mut self, notice: Option<String>) {
        self.in_flight = self.in_flight.saturating_sub(1);
        if notice.is_some() {
            self.notice = notice;
        }
    }

    pub fn show_notice(&mut self, message: impl Into<String>) {
        self.notice = Some(message.into());
    }

    pub fn notice_expired(&mut self) {
        self.notice = None;
    }
}

/// Whether the current recording is a spoken command: Shift pressed at any point while the key is held.
#[derive(Debug, Default)]
pub struct CommandIntent {
    is_command: bool,
}

impl CommandIntent {
    pub fn is_command(&self) -> bool {
        self.is_command
    }

    pub fn shift(&mut self, down: bool, key_held: bool) {
        if down && key_held {
            self.is_command = true;
        }
    }

    pub fn reset(&mut self) {
        self.is_command = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_is_hidden() {
        assert_eq!(DictationStatus::default().pill(), PillState::Hidden);
    }

    #[test]
    fn recording_wins_over_work_and_notice() {
        let mut s = DictationStatus::default();
        s.recording_started(false);
        s.recording_submitted();
        s.show_notice("x");
        s.recording_started(true);
        assert_eq!(s.pill(), PillState::HandsFree);
        s.mark_command();
        assert_eq!(s.pill(), PillState::Command);
    }

    #[test]
    fn notice_shows_until_expired_then_work_resumes() {
        let mut s = DictationStatus::default();
        s.recording_started(false);
        s.recording_submitted();
        s.recording_started(false);
        s.recording_submitted();
        s.dictation_finished(Some("Mislukt".into()));
        assert_eq!(s.pill(), PillState::Error("Mislukt".into()));
        s.notice_expired();
        assert_eq!(s.pill(), PillState::Working);
        s.dictation_finished(None);
        assert_eq!(s.pill(), PillState::Hidden);
    }

    #[test]
    fn mark_command_without_recording_is_ignored() {
        let mut s = DictationStatus::default();
        s.mark_command();
        assert_eq!(s.pill(), PillState::Hidden);
    }

    #[test]
    fn finishing_more_than_started_never_underflows() {
        let mut s = DictationStatus::default();
        s.dictation_finished(None);
        assert_eq!(s.pill(), PillState::Hidden);
    }

    #[test]
    fn shift_only_counts_while_key_held() {
        let mut c = CommandIntent::default();
        c.shift(true, false);
        assert!(!c.is_command());
        c.shift(true, true);
        c.shift(false, true);
        assert!(c.is_command());
        c.reset();
        assert!(!c.is_command());
    }

    #[test]
    fn pill_state_serializes_for_the_webview() {
        assert_eq!(
            serde_json::to_string(&PillState::Error("x".into())).unwrap(),
            r#"{"state":"error","message":"x"}"#
        );
        assert_eq!(
            serde_json::to_string(&PillState::Listening).unwrap(),
            r#"{"state":"listening"}"#
        );
    }
}
