//! System-wide listening for the dictation key. Platform hooks translate raw key events into [`Key`]s;
//! [`TriggerTracker`] turns those into edges for the gesture, interrupts and Shift changes.
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
#![cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use pennel_core::HotkeyTrigger;

use crate::gesture::KeyEdge;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftSuper,
    RightSuper,
    LeftShift,
    RightShift,
    /// Caps Lock, Fn and other keys that only modify; they never cancel a dictation.
    OtherModifier,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HotkeyEvent {
    Edge(KeyEdge, f64),
    Interrupt,
    Shift(bool),
}

#[derive(Debug, Default)]
pub struct TriggerTracker {
    trigger: Option<HotkeyTrigger>,
    ctrl: [bool; 2],
    alt: [bool; 2],
    super_: [bool; 2],
    shift: [bool; 2],
    active: bool,
}

impl TriggerTracker {
    pub fn new(trigger: HotkeyTrigger) -> Self {
        Self {
            trigger: Some(trigger),
            ..Self::default()
        }
    }

    pub fn set_trigger(&mut self, trigger: HotkeyTrigger) {
        *self = Self::new(trigger);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_shift_down(&self) -> bool {
        self.shift.iter().any(|d| *d)
    }

    fn trigger_engaged(&self) -> bool {
        match self.trigger.unwrap_or(HotkeyTrigger::RightControl) {
            HotkeyTrigger::RightControl | HotkeyTrigger::Fn => self.ctrl[1],
            HotkeyTrigger::RightOption => self.alt[1],
            HotkeyTrigger::RightCommand => self.super_[1],
            HotkeyTrigger::ControlSuper => {
                self.ctrl.iter().any(|d| *d) && self.super_.iter().any(|d| *d)
            }
        }
    }

    pub fn key(&mut self, key: Key, down: bool, time: f64) -> Vec<HotkeyEvent> {
        let slot = match key {
            Key::LeftCtrl => Some(&mut self.ctrl[0]),
            Key::RightCtrl => Some(&mut self.ctrl[1]),
            Key::LeftAlt => Some(&mut self.alt[0]),
            Key::RightAlt => Some(&mut self.alt[1]),
            Key::LeftSuper => Some(&mut self.super_[0]),
            Key::RightSuper => Some(&mut self.super_[1]),
            Key::LeftShift => Some(&mut self.shift[0]),
            Key::RightShift => Some(&mut self.shift[1]),
            Key::OtherModifier | Key::Other => None,
        };
        let changed = slot.is_some_and(|held| std::mem::replace(held, down) != down);
        let mut events = Vec::new();
        if matches!(key, Key::LeftShift | Key::RightShift) {
            if changed {
                events.push(HotkeyEvent::Shift(self.is_shift_down()));
            }
            return events;
        }
        if key == Key::Other && down && self.active {
            events.push(HotkeyEvent::Interrupt);
        }
        let engaged = self.trigger_engaged();
        if engaged != self.active {
            self.active = engaged;
            let edge = if engaged { KeyEdge::Down } else { KeyEdge::Up };
            events.push(HotkeyEvent::Edge(edge, time));
        }
        events
    }
}

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

pub fn now() -> f64 {
    epoch().elapsed().as_secs_f64()
}

static HELD: AtomicBool = AtomicBool::new(false);
static SHIFT: AtomicBool = AtomicBool::new(false);

pub fn trigger_held() -> bool {
    HELD.load(Ordering::SeqCst)
}

pub fn shift_held() -> bool {
    SHIFT.load(Ordering::SeqCst)
}

fn publish(tracker: &TriggerTracker) {
    HELD.store(tracker.is_active(), Ordering::SeqCst);
    SHIFT.store(tracker.is_shift_down(), Ordering::SeqCst);
}

/// Synthetic Ctrl+V/Ctrl+C must not reach the gesture as user input.
pub static INJECTING: AtomicBool = AtomicBool::new(false);

pub struct Listener {
    #[cfg(any(windows, target_os = "linux"))]
    inner: PlatformListener,
}

#[cfg(windows)]
type PlatformListener = windows::Hook;
#[cfg(target_os = "linux")]
type PlatformListener = linux::Hook;

impl Listener {
    /// Starts the platform hook on its own thread; events arrive on `sender`.
    pub fn start(trigger: HotkeyTrigger, sender: Sender<HotkeyEvent>) -> Result<Self, String> {
        epoch();
        #[cfg(any(windows, target_os = "linux"))]
        {
            PlatformListener::start(trigger, sender).map(|inner| Self { inner })
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = (trigger, sender);
            Err("sneltoets wordt op dit platform niet ondersteund".into())
        }
    }

    pub fn set_trigger(&self, trigger: HotkeyTrigger) {
        #[cfg(any(windows, target_os = "linux"))]
        self.inner.set_trigger(trigger);
        #[cfg(not(any(windows, target_os = "linux")))]
        let _ = trigger;
    }
}

/// Shared by the platform hooks: feeds one key event through the tracker and forwards the results.
fn dispatch(
    tracker: &std::sync::Mutex<TriggerTracker>,
    sender: &Sender<HotkeyEvent>,
    key: Key,
    down: bool,
) -> Vec<HotkeyEvent> {
    let mut tracker = tracker.lock().unwrap_or_else(|p| p.into_inner());
    let events = tracker.key(key, down, now());
    publish(&tracker);
    for event in &events {
        let _ = sender.send(*event);
    }
    events
}

#[cfg(test)]
mod tests {
    use super::HotkeyEvent::*;
    use super::*;

    fn press(t: &mut TriggerTracker, key: Key) -> Vec<HotkeyEvent> {
        t.key(key, true, 1.0)
    }

    fn release(t: &mut TriggerTracker, key: Key) -> Vec<HotkeyEvent> {
        t.key(key, false, 2.0)
    }

    #[test]
    fn right_ctrl_produces_edges_and_ignores_auto_repeat() {
        let mut t = TriggerTracker::new(HotkeyTrigger::RightControl);
        assert_eq!(press(&mut t, Key::RightCtrl), [Edge(KeyEdge::Down, 1.0)]);
        assert_eq!(press(&mut t, Key::RightCtrl), []);
        assert_eq!(release(&mut t, Key::RightCtrl), [Edge(KeyEdge::Up, 2.0)]);
    }

    #[test]
    fn left_ctrl_does_not_trigger_right_ctrl() {
        let mut t = TriggerTracker::new(HotkeyTrigger::RightControl);
        assert_eq!(press(&mut t, Key::LeftCtrl), []);
        assert_eq!(release(&mut t, Key::LeftCtrl), []);
    }

    #[test]
    fn other_key_while_held_interrupts() {
        let mut t = TriggerTracker::new(HotkeyTrigger::RightOption);
        press(&mut t, Key::RightAlt);
        assert_eq!(press(&mut t, Key::Other), [Interrupt]);
        assert_eq!(press(&mut t, Key::LeftCtrl), []);
        assert_eq!(press(&mut t, Key::OtherModifier), []);
    }

    #[test]
    fn other_key_without_trigger_is_ignored() {
        let mut t = TriggerTracker::new(HotkeyTrigger::RightControl);
        assert_eq!(press(&mut t, Key::Other), []);
    }

    #[test]
    fn shift_is_reported_separately() {
        let mut t = TriggerTracker::new(HotkeyTrigger::RightControl);
        press(&mut t, Key::RightCtrl);
        assert_eq!(press(&mut t, Key::LeftShift), [Shift(true)]);
        assert_eq!(press(&mut t, Key::RightShift), [Shift(true)]);
        assert_eq!(release(&mut t, Key::LeftShift), [Shift(true)]);
        assert_eq!(release(&mut t, Key::RightShift), [Shift(false)]);
        assert!(t.is_active());
    }

    #[test]
    fn chord_needs_ctrl_and_super_in_any_order() {
        let mut t = TriggerTracker::new(HotkeyTrigger::ControlSuper);
        assert_eq!(press(&mut t, Key::LeftSuper), []);
        assert_eq!(press(&mut t, Key::LeftCtrl), [Edge(KeyEdge::Down, 1.0)]);
        assert_eq!(release(&mut t, Key::LeftSuper), [Edge(KeyEdge::Up, 2.0)]);
        assert_eq!(release(&mut t, Key::LeftCtrl), []);
        assert_eq!(press(&mut t, Key::RightCtrl), []);
        assert_eq!(press(&mut t, Key::RightSuper), [Edge(KeyEdge::Down, 1.0)]);
    }

    #[test]
    fn changing_trigger_resets_state() {
        let mut t = TriggerTracker::new(HotkeyTrigger::RightControl);
        press(&mut t, Key::RightCtrl);
        t.set_trigger(HotkeyTrigger::RightOption);
        assert!(!t.is_active());
        assert_eq!(release(&mut t, Key::RightCtrl), []);
    }
}
