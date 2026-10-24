use serde::{Deserialize, Serialize};

/// A platform observation of an application currently using an audio input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallObservation {
    pub app_id: String,
    pub is_call: bool,
    pub is_playing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CallEvent {
    Started { app_id: String },
    Ended { app_id: String },
}

pub struct CallDetector {
    own_app_id: String,
    candidate: Option<(String, f64)>,
    active: Option<String>,
    last_seen: f64,
}

impl CallDetector {
    pub fn new(own_app_id: impl Into<String>) -> Self {
        Self {
            own_app_id: own_app_id.into(),
            candidate: None,
            active: None,
            last_seen: 0.0,
        }
    }

    pub fn active_call(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// `at` is monotonic elapsed time, shared by all observations.
    pub fn update(&mut self, users: &[CallObservation], at: f64) -> Vec<CallEvent> {
        if !at.is_finite() || at < 0.0 {
            return Vec::new();
        }
        if let Some(active) = self.active.as_ref() {
            // A browser changing tabs must not end an otherwise active call.
            if users.iter().any(|user| user.app_id == *active) {
                self.last_seen = at;
                return Vec::new();
            }
            if at - self.last_seen < 30.0 {
                return Vec::new();
            }
            let app_id = self.active.take().expect("active call");
            self.candidate = None;
            return vec![CallEvent::Ended { app_id }];
        }

        let eligible = |user: &&CallObservation| {
            user.is_call && user.is_playing && user.app_id != self.own_app_id
        };
        if let Some((candidate, since)) = &self.candidate
            && users
                .iter()
                .filter(eligible)
                .any(|user| user.app_id == *candidate)
        {
            if at - since < 5.0 {
                return Vec::new();
            }
            let app_id = candidate.clone();
            self.active = Some(app_id.clone());
            self.last_seen = at;
            self.candidate = None;
            return vec![CallEvent::Started { app_id }];
        }
        self.candidate = users
            .iter()
            .filter(eligible)
            .map(|user| user.app_id.clone())
            .min()
            .map(|app_id| (app_id, at));
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(app: &str, is_call: bool, is_playing: bool) -> CallObservation {
        CallObservation {
            app_id: app.into(),
            is_call,
            is_playing,
        }
    }

    fn teams() -> CallObservation {
        observation("teams", true, true)
    }

    #[test]
    fn short_or_interrupted_input_does_not_start() {
        let mut detector = CallDetector::new("pennel");
        assert!(detector.update(&[teams()], 0.0).is_empty());
        assert!(detector.update(&[teams()], 3.0).is_empty());
        assert!(detector.update(&[], 4.0).is_empty());
        assert!(detector.update(&[teams()], 6.0).is_empty());
        assert_eq!(detector.active_call(), None);
    }

    #[test]
    fn sustained_call_starts_once_and_ends_after_thirty_seconds() {
        let mut detector = CallDetector::new("pennel");
        detector.update(&[teams()], 0.0);
        assert_eq!(
            detector.update(&[teams()], 5.0),
            [CallEvent::Started {
                app_id: "teams".into()
            }]
        );
        assert!(detector.update(&[teams()], 10.0).is_empty());
        assert!(detector.update(&[], 39.0).is_empty());
        assert_eq!(
            detector.update(&[], 40.0),
            [CallEvent::Ended {
                app_id: "teams".into()
            }]
        );
        assert!(detector.update(&[], 50.0).is_empty());
    }

    #[test]
    fn mute_recovery_resets_end_debounce() {
        let mut detector = CallDetector::new("pennel");
        detector.update(&[teams()], 0.0);
        detector.update(&[teams()], 5.0);
        assert!(detector.update(&[], 30.0).is_empty());
        assert!(detector.update(&[teams()], 34.0).is_empty());
        assert!(detector.update(&[], 60.0).is_empty());
        assert_eq!(detector.active_call(), Some("teams"));
    }

    #[test]
    fn own_input_voice_messages_and_non_calls_are_ignored() {
        let mut detector = CallDetector::new("pennel");
        let users = [
            observation("pennel", true, true),
            observation("memo", false, false),
            observation("whatsapp", true, false),
        ];
        detector.update(&users, 0.0);
        assert!(detector.update(&users, 10.0).is_empty());
        assert_eq!(detector.active_call(), None);
    }

    #[test]
    fn active_browser_survives_tab_switch_and_second_call() {
        let mut detector = CallDetector::new("pennel");
        detector.update(&[observation("chrome", true, true)], 0.0);
        detector.update(&[observation("chrome", true, true)], 5.0);
        assert!(
            detector
                .update(&[observation("chrome", false, false), teams()], 60.0)
                .is_empty()
        );
        assert_eq!(detector.active_call(), Some("chrome"));
    }

    #[test]
    fn simultaneous_candidates_have_a_deterministic_order() {
        let mut detector = CallDetector::new("pennel");
        detector.update(&[teams(), observation("zoom", true, true)], 0.0);
        assert_eq!(
            detector.update(&[observation("zoom", true, true), teams()], 5.0),
            [CallEvent::Started {
                app_id: "teams".into()
            }]
        );
    }
}
