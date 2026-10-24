//! Synthetic Ctrl+V / Ctrl+C for pasting and reading the selection.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::hotkey;

const RELEASE_WAIT: Duration = Duration::from_millis(1500);

/// Waits until the user lets go of the dictation key and Shift: a held Right Alt (AltGr) or Shift
/// would otherwise turn our Ctrl+V into another shortcut.
pub fn wait_for_release() {
    let deadline = Instant::now() + RELEASE_WAIT;
    while (hotkey::trigger_held() || hotkey::shift_held()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[derive(Clone, Copy)]
pub enum Shortcut {
    Paste,
    Copy,
}

pub fn send(shortcut: Shortcut) -> Result<(), String> {
    hotkey::INJECTING.store(true, Ordering::SeqCst);
    let result = platform::send(shortcut);
    // X11 delivers our own events to the listener asynchronously.
    std::thread::sleep(Duration::from_millis(30));
    hotkey::INJECTING.store(false, Ordering::SeqCst);
    result
}

#[cfg(windows)]
mod platform {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_CONTROL,
    };

    use super::Shortcut;

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn send(shortcut: Shortcut) -> Result<(), String> {
        let letter = VIRTUAL_KEY(match shortcut {
            Shortcut::Paste => b'V',
            Shortcut::Copy => b'C',
        } as u16);
        let inputs = [
            key(VK_CONTROL, false),
            key(letter, false),
            key(letter, true),
            key(VK_CONTROL, true),
        ];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            Ok(())
        } else {
            Err("toetsaanslag geblokkeerd (UIPI: venster draait als administrator?)".into())
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    use super::Shortcut;

    #[cfg(target_os = "macos")]
    const MODIFIER: Key = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    const MODIFIER: Key = Key::Control;

    pub fn send(shortcut: Shortcut) -> Result<(), String> {
        let letter = match shortcut {
            Shortcut::Paste => 'v',
            Shortcut::Copy => 'c',
        };
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        enigo
            .key(MODIFIER, Direction::Press)
            .and_then(|()| enigo.key(Key::Unicode(letter), Direction::Click))
            .map_err(|e| e.to_string())?;
        enigo
            .key(MODIFIER, Direction::Release)
            .map_err(|e| e.to_string())
    }
}
