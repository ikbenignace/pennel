//! X11 key listening through rdev (XRecord). Works on X11 sessions and, on Wayland, only while an
//! XWayland window has focus; see docs/desktop.md.

use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use pennel_core::HotkeyTrigger;

use super::{HotkeyEvent, INJECTING, Key, TriggerTracker, dispatch};

pub struct Hook {
    tracker: Arc<Mutex<TriggerTracker>>,
}

fn key_for(key: rdev::Key) -> Key {
    use rdev::Key as K;
    match key {
        K::ControlLeft => Key::LeftCtrl,
        K::ControlRight => Key::RightCtrl,
        K::Alt => Key::LeftAlt,
        K::AltGr => Key::RightAlt,
        K::MetaLeft => Key::LeftSuper,
        K::MetaRight => Key::RightSuper,
        K::ShiftLeft => Key::LeftShift,
        K::ShiftRight => Key::RightShift,
        K::CapsLock | K::NumLock | K::ScrollLock | K::Function => Key::OtherModifier,
        _ => Key::Other,
    }
}

impl Hook {
    pub fn start(trigger: HotkeyTrigger, sender: Sender<HotkeyEvent>) -> Result<Self, String> {
        if std::env::var_os("DISPLAY").is_none() {
            return Err("geen X11-scherm (DISPLAY) gevonden".into());
        }
        let tracker = Arc::new(Mutex::new(TriggerTracker::new(trigger)));
        let hook_tracker = tracker.clone();
        let (error_tx, error_rx) = std::sync::mpsc::channel::<String>();
        std::thread::Builder::new()
            .name("pennel-hotkey".into())
            .spawn(move || {
                let result = rdev::listen(move |event| {
                    if INJECTING.load(Ordering::SeqCst) {
                        return;
                    }
                    let (key, down) = match event.event_type {
                        rdev::EventType::KeyPress(key) => (key, true),
                        rdev::EventType::KeyRelease(key) => (key, false),
                        _ => return,
                    };
                    dispatch(&hook_tracker, &sender, key_for(key), down);
                });
                if let Err(e) = result {
                    let _ = error_tx.send(format!("X11-toetsenbord niet beschikbaar: {e:?}"));
                }
            })
            .map_err(|e| e.to_string())?;
        match error_rx.recv_timeout(std::time::Duration::from_millis(300)) {
            Ok(message) => Err(message),
            Err(_) => Ok(Self { tracker }),
        }
    }

    pub fn set_trigger(&self, trigger: HotkeyTrigger) {
        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.set_trigger(trigger);
        }
    }
}
