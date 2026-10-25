use serde::{Deserialize, Serialize};

use crate::PennelError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingPhase {
    #[default]
    Idle,
    Starting,
    Recording,
    Stopping,
    Finalizing,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingState {
    pub phase: RecordingPhase,
    pub session_id: Option<String>,
    pub detected_app: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct RecordingLifecycle {
    state: RecordingState,
}

impl RecordingLifecycle {
    pub fn snapshot(&self) -> RecordingState {
        self.state.clone()
    }

    pub fn is_busy(&self) -> bool {
        !matches!(
            self.state.phase,
            RecordingPhase::Idle | RecordingPhase::Error
        )
    }

    pub fn owns_call(&self, app_id: &str) -> bool {
        self.is_busy() && self.state.detected_app.as_deref() == Some(app_id)
    }

    pub fn begin(&mut self, detected_app: Option<String>) -> Result<String, PennelError> {
        if self.is_busy() {
            return Err(PennelError::Storage {
                message: "er wordt al een meeting opgenomen of afgerond".into(),
            });
        }
        let id = uuid::Uuid::now_v7().to_string();
        self.state = RecordingState {
            phase: RecordingPhase::Starting,
            session_id: Some(id.clone()),
            detected_app,
            error: None,
        };
        Ok(id)
    }

    pub fn recording(&mut self, id: &str) -> Result<(), PennelError> {
        self.advance(id, RecordingPhase::Starting, RecordingPhase::Recording)
    }

    pub fn stopping(&mut self, id: &str) -> Result<(), PennelError> {
        self.advance(id, RecordingPhase::Recording, RecordingPhase::Stopping)
    }

    pub fn finalizing(&mut self, id: &str) -> Result<(), PennelError> {
        self.advance(id, RecordingPhase::Stopping, RecordingPhase::Finalizing)
    }

    pub fn complete(&mut self, id: &str) -> Result<(), PennelError> {
        self.advance(id, RecordingPhase::Finalizing, RecordingPhase::Idle)?;
        self.state = RecordingState::default();
        Ok(())
    }

    pub fn retry_finalizing(&mut self, id: &str) -> Result<(), PennelError> {
        self.advance(id, RecordingPhase::Error, RecordingPhase::Finalizing)?;
        self.state.error = None;
        Ok(())
    }

    pub fn fail(&mut self, id: &str, message: String) -> Result<(), PennelError> {
        self.check_id(id)?;
        if !self.is_busy() {
            return Err(PennelError::Cancelled);
        }
        self.state.phase = RecordingPhase::Error;
        self.state.error = Some(message);
        Ok(())
    }

    fn check_id(&self, id: &str) -> Result<(), PennelError> {
        if self.state.session_id.as_deref() != Some(id) {
            return Err(PennelError::Cancelled);
        }
        Ok(())
    }

    fn advance(
        &mut self,
        id: &str,
        from: RecordingPhase,
        to: RecordingPhase,
    ) -> Result<(), PennelError> {
        self.check_id(id)?;
        if self.state.phase != from {
            return Err(PennelError::Cancelled);
        }
        self.state.phase = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_session_can_retry_finalization_without_starting_capture() {
        let mut lifecycle = RecordingLifecycle::default();
        let id = lifecycle.begin(None).unwrap();
        lifecycle.fail(&id, "bewaren mislukt".into()).unwrap();
        assert_eq!(
            lifecycle.retry_finalizing("stale"),
            Err(PennelError::Cancelled)
        );
        assert_eq!(lifecycle.snapshot().phase, RecordingPhase::Error);
        lifecycle.retry_finalizing(&id).unwrap();
        assert!(lifecycle.is_busy());
        assert_eq!(lifecycle.snapshot().error, None);
        lifecycle.complete(&id).unwrap();
        assert_eq!(lifecycle.snapshot(), RecordingState::default());
    }

    #[test]
    fn recording_stays_busy_until_finalization_completes() {
        let mut lifecycle = RecordingLifecycle::default();
        let id = lifecycle.begin(None).unwrap();
        for transition in [
            RecordingLifecycle::recording,
            RecordingLifecycle::stopping,
            RecordingLifecycle::finalizing,
        ] {
            transition(&mut lifecycle, &id).unwrap();
            assert!(lifecycle.is_busy());
            assert!(lifecycle.begin(None).is_err());
        }
        lifecycle.complete(&id).unwrap();
        assert!(!lifecycle.is_busy());
        assert_eq!(lifecycle.snapshot(), RecordingState::default());
    }

    #[test]
    fn stale_completion_cannot_modify_a_new_session() {
        let mut lifecycle = RecordingLifecycle::default();
        let old = lifecycle.begin(Some("teams".into())).unwrap();
        lifecycle.fail(&old, "microfoon geweigerd".into()).unwrap();
        let next = lifecycle.begin(None).unwrap();
        assert_ne!(old, next);
        assert_eq!(lifecycle.complete(&old), Err(PennelError::Cancelled));
        assert_eq!(
            lifecycle.fail(&old, "late failure".into()),
            Err(PennelError::Cancelled)
        );
        assert_eq!(
            lifecycle.snapshot().session_id.as_deref(),
            Some(next.as_str())
        );
    }

    #[test]
    fn detected_call_does_not_own_manual_recordings() {
        let mut lifecycle = RecordingLifecycle::default();
        lifecycle.begin(None).unwrap();
        assert!(!lifecycle.owns_call("teams"));
    }

    #[test]
    fn ending_one_call_cannot_stop_another() {
        let mut lifecycle = RecordingLifecycle::default();
        lifecycle.begin(Some("teams".into())).unwrap();
        assert!(lifecycle.owns_call("teams"));
        assert!(!lifecycle.owns_call("zoom"));
    }

    #[test]
    fn invalid_transition_is_rejected_without_changing_state() {
        let mut lifecycle = RecordingLifecycle::default();
        let id = lifecycle.begin(None).unwrap();
        assert_eq!(lifecycle.complete(&id), Err(PennelError::Cancelled));
        assert_eq!(lifecycle.snapshot().phase, RecordingPhase::Starting);
    }
}
