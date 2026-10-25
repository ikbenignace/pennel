//! The floating status pill: transparent, always on top, click-through, never takes focus,
//! bottom-center on the screen that holds the mouse.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use pennel_core::DictationPreviewUpdate;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow};

use crate::status::PillState;

pub const LABEL: &str = "pill";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PillSnapshot {
    #[serde(flatten)]
    pub state: PillState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<DictationPreviewUpdate>,
}

impl Default for PillSnapshot {
    fn default() -> Self {
        Self {
            state: PillState::Hidden,
            preview: None,
        }
    }
}

impl PillSnapshot {
    fn set_state(&mut self, state: PillState) -> bool {
        let changed = self.state != state || (state == PillState::Hidden && self.preview.is_some());
        self.state = state;
        if self.state == PillState::Hidden {
            self.preview = None;
        }
        changed
    }

    fn begin_preview(&mut self, id: String) -> bool {
        if self.state == PillState::Hidden
            || self
                .preview
                .as_ref()
                .is_some_and(|current| current.session_id == id)
        {
            return false;
        }
        self.preview = Some(DictationPreviewUpdate {
            session_id: id,
            revision: 0,
            text: String::new(),
        });
        true
    }

    fn update_preview(&mut self, update: DictationPreviewUpdate) -> bool {
        let Some(current) = self.preview.as_ref() else {
            return false;
        };
        if self.state == PillState::Hidden
            || current.session_id != update.session_id
            || current.revision >= update.revision
        {
            return false;
        }
        self.preview = Some(update);
        true
    }

    fn clear_preview(&mut self, id: Option<&str>) -> bool {
        let Some(current) = self.preview.as_ref() else {
            return false;
        };
        if id.is_some_and(|id| id != current.session_id) {
            return false;
        }
        self.preview = None;
        true
    }
}

pub struct Snapshot(pub Arc<Mutex<PillSnapshot>>);
impl Default for Snapshot {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(PillSnapshot::default())))
    }
}

pub fn snapshot(app: &AppHandle) -> PillState {
    content_snapshot(app).state
}

pub fn content_snapshot(app: &AppHandle) -> PillSnapshot {
    app.try_state::<Snapshot>()
        .map(|state| state.0.lock().unwrap().clone())
        .unwrap_or_default()
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PillMetrics {
    window_width: f64,
    window_height: f64,
}

fn metrics() -> &'static PillMetrics {
    #[derive(Deserialize)]
    struct Tokens {
        pill: PillMetrics,
    }
    static METRICS: OnceLock<PillMetrics> = OnceLock::new();
    METRICS.get_or_init(|| {
        serde_json::from_str::<Tokens>(include_str!("../../../../assets/brand/tokens.json"))
            .expect("checked-in pill metrics must be valid")
            .pill
    })
}

fn fitted_geometry(
    origin: (i32, i32),
    size: (u32, u32),
    scale: f64,
) -> Option<(i32, i32, u32, u32)> {
    if size.0 == 0 || size.1 == 0 {
        return None;
    }
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let width = (metrics().window_width * scale)
        .round()
        .max(1.0)
        .min(f64::from(size.0)) as u32;
    let height = (metrics().window_height * scale)
        .round()
        .max(1.0)
        .min(f64::from(size.1)) as u32;
    Some((
        origin.0 + ((size.0 - width) / 2) as i32,
        origin.1 + (size.1 - height) as i32,
        width,
        height,
    ))
}

const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

#[cfg(test)]
mod tests {
    use super::*;

    fn preview(id: &str, revision: u64, text: &str) -> DictationPreviewUpdate {
        DictationPreviewUpdate {
            session_id: id.into(),
            revision,
            text: text.into(),
        }
    }

    fn listening() -> PillSnapshot {
        let mut content = PillSnapshot::default();
        content.set_state(PillState::Listening);
        content.begin_preview("first".into());
        content
    }

    #[test]
    fn preview_boot_snapshot_retains_status_and_latest_text() {
        let mut content = listening();
        assert!(content.update_preview(preview("first", 2, "Een voorlopige zin")));
        content.set_state(PillState::Working);
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["state"], "working");
        assert_eq!(json["preview"]["session_id"], "first");
        assert_eq!(json["preview"]["revision"], 2);
        assert_eq!(json["preview"]["text"], "Een voorlopige zin");
    }

    #[test]
    fn late_session_and_nonincreasing_revisions_never_replace_current_text() {
        let mut content = listening();
        assert!(content.update_preview(preview("first", 3, "De nieuwste tekst")));
        assert!(!content.update_preview(preview("old", 99, "Een oud resultaat")));
        assert!(!content.update_preview(preview("first", 2, "Een trager resultaat")));
        assert!(!content.update_preview(preview("first", 3, "Een dubbel resultaat")));
        assert_eq!(content.preview.as_ref().unwrap().text, "De nieuwste tekst");
    }

    #[test]
    fn old_completion_cannot_clear_a_new_capture() {
        let mut content = listening();
        content.update_preview(preview("first", 1, "Eerder"));
        assert!(content.begin_preview("second".into()));
        assert!(!content.clear_preview(Some("first")));
        assert!(!content.update_preview(preview("first", 100, "Te laat")));
        let current = content.preview.as_ref().unwrap();
        assert_eq!(current.session_id, "second");
        assert_eq!(current.revision, 0);
        assert!(current.text.is_empty());
    }

    #[test]
    fn repeated_begin_does_not_rewind_an_existing_preview() {
        let mut content = listening();
        content.update_preview(preview("first", 4, "Al herkend"));
        assert!(!content.begin_preview("first".into()));
        assert_eq!(content.preview.as_ref().unwrap().revision, 4);
        assert_eq!(content.preview.as_ref().unwrap().text, "Al herkend");
    }

    #[test]
    fn hiding_clears_transient_text_and_refuses_late_updates() {
        let mut content = listening();
        content.update_preview(preview("first", 1, "Tijdelijke tekst"));
        assert!(content.set_state(PillState::Hidden));
        assert!(content.preview.is_none());
        assert!(!content.update_preview(preview("first", 2, "Later")));
        assert!(!content.begin_preview("second".into()));
        assert!(
            serde_json::to_value(&content)
                .unwrap()
                .get("preview")
                .is_none()
        );
    }

    #[test]
    fn core_caption_is_preserved_for_identical_grapheme_layout_in_both_renderers() {
        let mut content = listening();
        let text = format!("{}e\u{301}👩‍👩‍👧", "é".repeat(180));
        content.update_preview(preview("first", 1, &text));
        let visible = &content.preview.as_ref().unwrap().text;
        assert_eq!(visible, &text);
    }

    #[test]
    fn pill_geometry_stays_inside_small_scaled_and_negative_work_areas() {
        let (x, y, width, height) = fitted_geometry((-1400, 20), (350, 100), 1.25).unwrap();
        assert_eq!((x, y, width, height), (-1400, 20, 350, 100));
        let (x, y, width, height) = fitted_geometry((-1920, 24), (1920, 1056), 1.0).unwrap();
        assert_eq!(width, metrics().window_width as u32);
        assert_eq!(height, metrics().window_height as u32);
        assert!(x >= -1920 && x + width as i32 <= 0);
        assert!(y >= 24 && y + height as i32 <= 1080);
        assert!(fitted_geometry((0, 0), (0, 100), 1.0).is_none());
    }
}

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
        .inner_size(metrics().window_width, metrics().window_height)
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
    content: Arc<Mutex<PillSnapshot>>,
    last_level: Mutex<Instant>,
}

impl Pill {
    pub fn new(app: AppHandle) -> Self {
        let content = app.state::<Snapshot>().0.clone();
        Self {
            app,
            content,
            last_level: Mutex::new(Instant::now()),
        }
    }

    pub fn show(&self, state: PillState) {
        let was_hidden = {
            let mut content = self.content.lock().unwrap();
            let was_hidden = content.state == PillState::Hidden;
            if !content.set_state(state.clone()) {
                return;
            }
            let _ = self.app.emit("dictation-state", &state);
            let _ = self.app.emit_to(LABEL, "pill-state", &*content);
            if state == PillState::Hidden {
                self.emit_preview(&content);
            }
            was_hidden
        };
        self.sync_visibility(was_hidden);
    }

    pub fn begin_preview(&self, id: impl Into<String>) {
        let mut content = self.content.lock().unwrap();
        if content.begin_preview(id.into()) {
            self.emit_preview(&content);
        }
    }

    pub fn update_preview(&self, update: DictationPreviewUpdate) {
        let mut content = self.content.lock().unwrap();
        if content.update_preview(update) {
            self.emit_preview(&content);
        }
    }

    pub fn clear_preview(&self, id: Option<&str>) {
        let mut content = self.content.lock().unwrap();
        if content.clear_preview(id) {
            self.emit_preview(&content);
        }
    }

    fn emit_preview(&self, content: &PillSnapshot) {
        let _ = self.app.emit_to(LABEL, "pill-preview", &content.preview);
    }

    fn sync_visibility(&self, position: bool) {
        let app = self.app.clone();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let current = snapshot(&handle);
            let existing = handle.get_webview_window(LABEL);
            let is_new = existing.is_none();
            let window = existing.or_else(|| {
                (current != PillState::Hidden)
                    .then(|| create(&handle).ok())
                    .flatten()
            });
            let Some(window) = window else {
                return;
            };
            let latest = content_snapshot(&handle);
            let _ = window.emit_to(LABEL, "pill-state", &latest);
            if latest.state == PillState::Hidden {
                platform::hide(&window);
            } else {
                if position || is_new {
                    Self::position(&handle, &window);
                }
                platform::show(&window);
            }
        });
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
        let area = monitor.work_area();
        let Some((x, y, width, height)) = fitted_geometry(
            (area.position.x, area.position.y),
            (area.size.width, area.size.height),
            monitor.scale_factor(),
        ) else {
            return;
        };
        let _ = window.set_size(tauri::PhysicalSize::new(width, height));
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
