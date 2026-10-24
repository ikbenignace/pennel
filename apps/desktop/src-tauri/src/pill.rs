//! The floating status pill: transparent, always on top, click-through, never takes focus,
//! bottom-center on the screen that holds the mouse.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow};

use crate::status::PillState;

pub const LABEL: &str = "pill";
pub struct Snapshot(pub Mutex<PillState>);
impl Default for Snapshot {
    fn default() -> Self {
        Self(Mutex::new(PillState::Hidden))
    }
}

pub fn snapshot(app: &AppHandle) -> PillState {
    app.try_state::<Snapshot>()
        .map(|state| state.0.lock().unwrap().clone())
        .unwrap_or(PillState::Hidden)
}
const WIDTH: f64 = 420.0;
const HEIGHT: f64 = 72.0;
const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

#[cfg(not(target_os = "macos"))]
pub fn show_without_focus(window: &WebviewWindow) {
    platform::show(window);
}
pub fn hide_without_focus(window: &WebviewWindow) {
    platform::hide(window);
}

pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = tauri::WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("pill.html".into()))
        .title("Pennel")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;
    let _ = window.set_ignore_cursor_events(true);
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = window.ns_window() {
        unsafe {
            crate::macos::configure_panel(pointer, false);
        }
    }
    Ok(window)
}

pub struct Pill {
    app: AppHandle,
    shown: Mutex<PillState>,
    last_level: Mutex<Instant>,
}

impl Pill {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            shown: Mutex::new(PillState::Hidden),
            last_level: Mutex::new(Instant::now()),
        }
    }

    fn window(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window(LABEL)
    }

    pub fn show(&self, state: PillState) {
        let mut shown = self.shown.lock().unwrap();
        if *shown == state {
            return;
        }
        let was_hidden = *shown == PillState::Hidden;
        *shown = state.clone();
        if let Some(snapshot) = self.app.try_state::<Snapshot>() {
            *snapshot.0.lock().unwrap() = state.clone();
        }
        let _ = self.app.emit("dictation-state", &state);
        let Some(window) = self.window() else {
            if state != PillState::Hidden {
                let app = self.app.clone();
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let window = handle
                        .get_webview_window(LABEL)
                        .or_else(|| create(&handle).ok());
                    if let Some(window) = window {
                        let state = snapshot(&handle);
                        let _ = window.emit_to(LABEL, "pill-state", &state);
                        if state != PillState::Hidden {
                            Self::position(&handle, &window);
                            platform::show(&window);
                        }
                    }
                });
            }
            return;
        };
        let _ = window.emit_to(LABEL, "pill-state", &state);
        if state == PillState::Hidden {
            platform::hide(&window);
            return;
        }
        if was_hidden {
            Self::position(&self.app, &window);
            platform::show(&window);
        }
    }

    /// Throttled: the microphone callback fires far more often than the pill can redraw.
    pub fn level(&self, level: f32) {
        let mut last = self.last_level.lock().unwrap();
        if last.elapsed() < LEVEL_INTERVAL {
            return;
        }
        *last = Instant::now();
        let _ = self.app.emit_to(LABEL, "pill-level", level);
    }

    fn position(app: &AppHandle, window: &WebviewWindow) {
        let cursor = app.cursor_position().ok();
        let monitors = app.available_monitors().unwrap_or_default();
        let monitor = cursor
            .and_then(|c| {
                monitors.iter().find(|m| {
                    let (p, s) = (m.position(), m.size());
                    c.x >= f64::from(p.x)
                        && c.x < f64::from(p.x) + f64::from(s.width)
                        && c.y >= f64::from(p.y)
                        && c.y < f64::from(p.y) + f64::from(s.height)
                })
            })
            .cloned()
            .or_else(|| app.primary_monitor().ok().flatten());
        let Some(monitor) = monitor else {
            return;
        };
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let width = (WIDTH * scale) as i32;
        let height = (HEIGHT * scale) as i32;
        let x = area.position.x + (area.size.width as i32 - width) / 2;
        let y = area.position.y + area.size.height as i32 - height;
        let _ = window.set_size(tauri::PhysicalSize::new(width as u32, height as u32));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

#[cfg(windows)]
mod platform {
    use tauri::WebviewWindow;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetWindowPos, ShowWindow,
    };

    fn hwnd(window: &WebviewWindow) -> Option<HWND> {
        window.hwnd().ok().map(|h| HWND(h.0))
    }

    /// Tauri's show() may activate the window; SW_SHOWNOACTIVATE keeps focus in the user's app.
    pub fn show(window: &WebviewWindow) {
        if let Some(hwnd) = hwnd(window) {
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }

    pub fn hide(window: &WebviewWindow) {
        if let Some(hwnd) = hwnd(window) {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use tauri::WebviewWindow;
    pub fn show(window: &WebviewWindow) {
        if let Ok(pointer) = window.ns_window() {
            unsafe {
                crate::macos::show_panel(pointer);
            }
        }
    }
    pub fn hide(window: &WebviewWindow) {
        if let Ok(pointer) = window.ns_window() {
            unsafe {
                crate::macos::hide_panel(pointer);
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use tauri::WebviewWindow;

    pub fn show(window: &WebviewWindow) {
        let _ = window.show();
    }

    pub fn hide(window: &WebviewWindow) {
        let _ = window.hide();
    }
}
