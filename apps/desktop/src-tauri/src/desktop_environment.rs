use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopMaterial {
    LiquidGlass,
    Vibrancy,
    Opaque,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DesktopEnvironment {
    pub material: DesktopMaterial,
    pub appearance: String,
    pub reduce_transparency: bool,
    pub reduce_motion: bool,
    pub increase_contrast: bool,
    pub titlebar_height: f64,
    pub titlebar_controls_width: f64,
    pub focused: bool,
}

impl DesktopEnvironment {
    fn unavailable(window: &WebviewWindow) -> Self {
        Self {
            material: DesktopMaterial::Unavailable,
            appearance: if matches!(window.theme(), Ok(tauri::Theme::Dark)) {
                "dark"
            } else {
                "light"
            }
            .into(),
            reduce_transparency: false,
            reduce_motion: false,
            increase_contrast: false,
            titlebar_height: 0.0,
            titlebar_controls_width: 0.0,
            focused: window.is_focused().unwrap_or(false),
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::DesktopEnvironment;
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::sync::{OnceLock, mpsc};
    use std::time::Duration;
    use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

    const EVENT: &str = "desktop-environment-changed";
    static APPLICATION: OnceLock<AppHandle> = OnceLock::new();
    type EnvironmentSink = unsafe extern "C" fn(*const c_char, *const c_char);

    unsafe extern "C" {
        fn pennel_mac_string_free(value: *mut c_char);
        fn pennel_mac_desktop_material_attach(
            view: *mut c_void,
            label: *const c_char,
            floating: bool,
            sink: EnvironmentSink,
        ) -> *mut c_char;
        fn pennel_mac_desktop_environment(view: *mut c_void) -> *mut c_char;
        fn pennel_mac_desktop_material_detach(view: *mut c_void);
        fn pennel_mac_desktop_material_appearance(view: *mut c_void, appearance: i32);
    }

    unsafe fn take_snapshot(pointer: *mut c_char) -> Option<DesktopEnvironment> {
        if pointer.is_null() {
            return None;
        }
        let json = unsafe { CStr::from_ptr(pointer) }.to_bytes();
        let snapshot = serde_json::from_slice(json).ok();
        unsafe { pennel_mac_string_free(pointer) };
        snapshot
    }

    unsafe extern "C" fn environment_changed(label: *const c_char, json: *const c_char) {
        if label.is_null() || json.is_null() {
            return;
        }
        let Some(app) = APPLICATION.get() else { return };
        let label = unsafe { CStr::from_ptr(label) }.to_string_lossy();
        let Ok(snapshot) = serde_json::from_slice::<DesktopEnvironment>(
            unsafe { CStr::from_ptr(json) }.to_bytes(),
        ) else {
            return;
        };
        // The callback borrows Swift-owned strings only for this call; no Rust context outlives a window.
        let _ = app.emit_to(label.as_ref(), EVENT, snapshot);
    }

    pub fn attach(window: &WebviewWindow, floating: bool) -> Result<(), String> {
        APPLICATION.get_or_init(|| window.app_handle().clone());
        let label = CString::new(window.label()).map_err(|_| "Invalid native window label")?;
        window
            .with_webview(move |webview| {
                let snapshot = unsafe {
                    take_snapshot(pennel_mac_desktop_material_attach(
                        webview.inner(),
                        label.as_ptr(),
                        floating,
                        environment_changed,
                    ))
                };
                let log = crate::diagnostics::Log::new("window");
                if let Some(snapshot) = snapshot {
                    log.info(format!(
                        "Native material ready: window={} material={:?} titlebar_height={:.1}",
                        label.to_string_lossy(),
                        snapshot.material,
                        snapshot.titlebar_height
                    ));
                } else {
                    log.error("Native material attachment returned no environment snapshot");
                }
            })
            .map_err(|error| error.to_string())
    }

    /// Call from a worker: the UI thread completes the native read through with_webview.
    pub fn snapshot(window: &WebviewWindow) -> Result<DesktopEnvironment, String> {
        let fallback = DesktopEnvironment::unavailable(window);
        let (send, receive) = mpsc::sync_channel(1);
        window
            .with_webview(move |webview| {
                let snapshot =
                    unsafe { take_snapshot(pennel_mac_desktop_environment(webview.inner())) };
                let _ = send.send(snapshot);
            })
            .map_err(|error| error.to_string())?;
        receive
            .recv_timeout(Duration::from_secs(5))
            .map(|value| value.unwrap_or(fallback))
            .map_err(|_| "The native window did not respond".into())
    }

    pub fn teardown(window: &WebviewWindow) {
        let _ = window.with_webview(|webview| unsafe {
            pennel_mac_desktop_material_detach(webview.inner());
        });
    }

    pub fn apply_appearance(window: &WebviewWindow, theme: Option<tauri::Theme>) {
        let appearance = match theme {
            Some(tauri::Theme::Light) => 1,
            Some(tauri::Theme::Dark) => 2,
            _ => 0,
        };
        let _ = window.with_webview(move |webview| unsafe {
            pennel_mac_desktop_material_appearance(webview.inner(), appearance);
        });
    }
}

/// Schedules attachment on the UI thread; material readiness is published after native attachment.
pub fn attach(window: &WebviewWindow, floating: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return native::attach(window, floating);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, floating);
        Ok(())
    }
}

pub fn snapshot(window: &WebviewWindow) -> Result<DesktopEnvironment, String> {
    #[cfg(target_os = "macos")]
    return native::snapshot(window);
    #[cfg(not(target_os = "macos"))]
    Ok(DesktopEnvironment::unavailable(window))
}

pub fn teardown(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    native::teardown(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn apply_appearance(window: &WebviewWindow, theme: Option<tauri::Theme>) {
    #[cfg(target_os = "macos")]
    native::apply_appearance(window, theme);
    #[cfg(not(target_os = "macos"))]
    let _ = (window, theme);
}

#[tauri::command]
pub async fn desktop_environment(window: WebviewWindow) -> Result<DesktopEnvironment, String> {
    tauri::async_runtime::spawn_blocking(move || snapshot(&window))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_snapshot_uses_the_frontend_environment_contract() {
        let json = r#"{"material":"liquid_glass","appearance":"dark","reduce_transparency":false,"reduce_motion":true,"increase_contrast":false,"titlebar_height":28.0,"titlebar_controls_width":96.0,"focused":false}"#;
        let snapshot: DesktopEnvironment = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.material, DesktopMaterial::LiquidGlass);
        assert!(snapshot.reduce_motion);
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn unknown_materials_or_missing_accessibility_fields_are_rejected() {
        assert!(serde_json::from_str::<DesktopEnvironment>(r#"{"material":"css_blur","appearance":"light","reduce_transparency":false,"reduce_motion":false,"increase_contrast":false,"titlebar_height":0,"focused":true}"#).is_err());
        assert!(
            serde_json::from_str::<DesktopEnvironment>(
                r#"{"material":"opaque","appearance":"light"}"#
            )
            .is_err()
        );
    }
}
