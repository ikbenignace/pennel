//! Window captures for visual review of preview builds (scripts/capture-desktop-windows.sh).
//! Only active in a preview build started with `PENNEL_WINDOW_CAPTURE=1`; release builds never enable it.
#![cfg(target_os = "macos")]

use std::ffi::{c_char, c_void};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

const VARIABLE: &str = "PENNEL_WINDOW_CAPTURE";

#[link(name = "objc")]
unsafe extern "C" {
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
}

fn active() -> &'static bool {
    static ACTIVE: OnceLock<bool> = OnceLock::new();
    ACTIVE.get_or_init(|| std::env::var(VARIABLE).is_ok_and(|value| value == "1"))
}

pub fn enabled(app: &AppHandle) -> bool {
    *active() && crate::is_preview(app)
}

/// WebKit stops painting a web view whose window is occluded (for example while the screen is locked),
/// which leaves captures blank. Capture mode keeps the views painting.
pub fn prepare(app: &AppHandle, window: &tauri::WebviewWindow) {
    if !enabled(app) {
        return;
    }
    let _ = window.with_webview(|webview| unsafe {
        let view = webview.inner();
        if view.is_null() {
            return;
        }
        let responds: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        let set: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) =
            std::mem::transmute(objc_msgSend as *const ());
        let selector = sel_registerName(c"_setWindowOcclusionDetectionEnabled:".as_ptr());
        if responds(
            view,
            sel_registerName(c"respondsToSelector:".as_ptr()),
            selector,
        ) {
            set(view, selector, false);
        }
    });
}

/// `--capture-show <window> [tab]` opens a window on a route; `--capture-eval <label> <script>` drives
/// a fixture state that has no route (a meeting tab, a dialog).
pub fn dispatch(app: &AppHandle, arguments: &[String]) -> bool {
    if !enabled(app) {
        return false;
    }
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--capture-show")
    {
        let kind = arguments
            .get(index + 1)
            .map(String::as_str)
            .unwrap_or("workspace");
        let tab = arguments.get(index + 2).map(String::as_str);
        crate::show_window(app, kind, tab);
        return true;
    }
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--capture-eval")
    {
        if let (Some(label), Some(script)) = (arguments.get(index + 1), arguments.get(index + 2))
            && let Some(window) = app.get_webview_window(label)
        {
            let _ = window.eval(script);
        }
        return true;
    }
    false
}
