//! Port of `PennelKit/HotkeyGesture.swift`: hold to talk, double-tap for hands-free.
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEdge {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureAction {
    Start,
    Submit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Holding {
        pressed_at: f64,
    },
    HandsFree {
        started_at: f64,
        awaiting_release: bool,
    },
    AwaitingRelease,
}

#[derive(Debug, Clone)]
pub struct HotkeyGesture {
    tap_max: f64,
    double_tap_window: f64,
    max_duration: f64,
    phase: Phase,
    last_tap_up: Option<f64>,
    hands_free: bool,
}

impl Default for HotkeyGesture {
    fn default() -> Self {
        Self::new(0.25, 0.35, 600.0)
    }
}

impl HotkeyGesture {
    pub fn new(tap_max: f64, double_tap_window: f64, max_duration: f64) -> Self {
        Self {
            tap_max,
            double_tap_window,
            max_duration,
            phase: Phase::Idle,
            last_tap_up: None,
            hands_free: false,
        }
    }

    pub fn is_hands_free(&self) -> bool {
        self.hands_free
    }

    pub fn handle(&mut self, edge: KeyEdge, t: f64) -> Vec<GestureAction> {
        match (edge, self.phase) {
            (KeyEdge::Down, Phase::Idle) => {
                if self
                    .last_tap_up
                    .is_some_and(|up| t - up <= self.double_tap_window)
                {
                    self.last_tap_up = None;
                    self.hands_free = true;
                    self.phase = Phase::HandsFree {
                        started_at: t,
                        awaiting_release: true,
                    };
                } else {
                    self.phase = Phase::Holding { pressed_at: t };
                }
                vec![GestureAction::Start]
            }
            (
                KeyEdge::Down,
                Phase::HandsFree {
                    awaiting_release: false,
                    ..
                },
            ) => {
                self.hands_free = false;
                self.phase = Phase::AwaitingRelease;
                vec![GestureAction::Submit]
            }
            (KeyEdge::Up, Phase::Holding { pressed_at }) => {
                self.phase = Phase::Idle;
                if t - pressed_at >= self.tap_max {
                    return vec![GestureAction::Submit];
                }
                self.last_tap_up = Some(t);
                vec![GestureAction::Cancel]
            }
            (
                KeyEdge::Up,
                Phase::HandsFree {
                    started_at,
                    awaiting_release: true,
                },
            ) => {
                self.phase = Phase::HandsFree {
                    started_at,
                    awaiting_release: false,
                };
                vec![]
            }
            (KeyEdge::Up, Phase::AwaitingRelease) => {
                self.phase = Phase::Idle;
                vec![]
            }
            _ => vec![],
        }
    }

    /// Another key pressed while holding means the hotkey is used as a modifier (Ctrl+C), not to dictate.
    pub fn interrupt(&mut self) -> Vec<GestureAction> {
        if !matches!(self.phase, Phase::Holding { .. }) {
            return vec![];
        }
        self.phase = Phase::AwaitingRelease;
        vec![GestureAction::Cancel]
    }

    pub fn tick(&mut self, t: f64) -> Vec<GestureAction> {
        match self.phase {
            Phase::Holding { pressed_at } if t - pressed_at >= self.max_duration => {
                self.phase = Phase::AwaitingRelease;
            }
            Phase::HandsFree {
                started_at,
                awaiting_release,
            } if t - started_at >= self.max_duration => {
                self.hands_free = false;
                self.phase = if awaiting_release {
                    Phase::AwaitingRelease
                } else {
                    Phase::Idle
                };
            }
            _ => return vec![],
        }
        vec![GestureAction::Submit]
    }
}

#[cfg(test)]
mod tests {
    use super::GestureAction::*;
    use super::KeyEdge::*;
    use super::*;

    #[test]
    fn hold_then_release_submits() {
        let mut g = HotkeyGesture::default();
        assert_eq!(g.handle(Down, 0.0), [Start]);
        assert_eq!(g.handle(Up, 1.0), [Submit]);
    }

    #[test]
    fn short_tap_cancels() {
        let mut g = HotkeyGesture::default();
        assert_eq!(g.handle(Down, 0.0), [Start]);
        assert_eq!(g.handle(Up, 0.1), [Cancel]);
    }

    #[test]
    fn double_tap_enters_hands_free_and_next_press_submits() {
        let mut g = HotkeyGesture::default();
        g.handle(Down, 0.0);
        g.handle(Up, 0.1);
        assert_eq!(g.handle(Down, 0.3), [Start]);
        assert!(g.is_hands_free());
        assert_eq!(g.handle(Up, 0.4), []);
        assert_eq!(g.handle(Down, 5.0), [Submit]);
        assert!(!g.is_hands_free());
        assert_eq!(g.handle(Up, 5.1), []);
        assert_eq!(g.handle(Down, 9.0), [Start]);
        assert!(!g.is_hands_free());
    }

    #[test]
    fn slow_second_tap_is_a_new_hold() {
        let mut g = HotkeyGesture::default();
        g.handle(Down, 0.0);
        g.handle(Up, 0.1);
        g.handle(Down, 1.0);
        assert!(!g.is_hands_free());
        assert_eq!(g.handle(Up, 2.0), [Submit]);
    }

    #[test]
    fn other_key_while_holding_cancels_until_release() {
        let mut g = HotkeyGesture::default();
        g.handle(Down, 0.0);
        assert_eq!(g.interrupt(), [Cancel]);
        assert_eq!(g.handle(Up, 2.0), []);
        assert_eq!(g.handle(Down, 3.0), [Start]);
    }

    #[test]
    fn interrupt_does_not_stop_hands_free() {
        let mut g = HotkeyGesture::default();
        g.handle(Down, 0.0);
        g.handle(Up, 0.1);
        g.handle(Down, 0.2);
        g.handle(Up, 0.3);
        assert_eq!(g.interrupt(), []);
    }

    #[test]
    fn max_duration_submits() {
        let mut g = HotkeyGesture::new(0.25, 0.35, 10.0);
        g.handle(Down, 0.0);
        assert_eq!(g.tick(5.0), []);
        assert_eq!(g.tick(10.0), [Submit]);
        assert_eq!(g.handle(Up, 11.0), []);
    }

    #[test]
    fn hands_free_max_duration_submits_and_resets() {
        let mut g = HotkeyGesture::new(0.25, 0.35, 10.0);
        g.handle(Down, 0.0);
        g.handle(Up, 0.1);
        g.handle(Down, 0.2);
        g.handle(Up, 0.3);
        assert_eq!(g.tick(10.2), [Submit]);
        assert!(!g.is_hands_free());
        assert_eq!(g.handle(Down, 12.0), [Start]);
    }
}
