//! Retains clipboard ownership on X11 after a copy command completes.

/// Like `copy`, but marked so clipboard history and managers skip the text.
pub fn copy_secret(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return crate::macos::copy_secret(text);
    #[cfg(target_os = "linux")]
    {
        if crate::linux_input::is_wayland() {
            return crate::linux_input::copy_wayland_secret(text)
                .map_err(|_| crate::clipboard::COPY_FAILED.to_owned());
        }
        // The persistent clipboard keeps X11 ownership after this call returns.
        with_persistent_clipboard(|clipboard| {
            use arboard::SetExtLinux;
            clipboard
                .set()
                .exclude_from_history()
                .text(text.to_owned())
                .map_err(|error| error.to_string())
        })
        .map_err(|_| crate::clipboard::COPY_FAILED.to_owned())
    }
    #[cfg(windows)]
    {
        let mut clipboard = crate::clipboard_system::SystemClipboard::new();
        crate::clipboard::copy_secret(&mut clipboard, text)
    }
}

#[cfg(not(target_os = "macos"))]
fn with_persistent_clipboard<T>(
    work: impl FnOnce(&mut arboard::Clipboard) -> Result<T, String>,
) -> Result<T, String> {
    use std::sync::{Mutex, OnceLock};
    static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
    let mut clipboard = CLIPBOARD.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if clipboard.is_none() {
        *clipboard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
    }
    work(clipboard.as_mut().expect("initialized clipboard"))
}

pub fn copy(text: &str, html: Option<&str>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return crate::macos::copy_text(text, html);
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "linux")]
        if crate::linux_input::is_wayland() {
            return crate::linux_input::copy_wayland(text);
        }
        with_persistent_clipboard(|clipboard| {
            match html {
                Some(html) => clipboard.set_html(html, Some(text)),
                None => clipboard.set_text(text),
            }
            .map_err(|error| error.to_string())
        })
    }
}
