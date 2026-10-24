//! Retains clipboard ownership on X11 after a copy command completes.

pub fn copy(text: &str, html: Option<&str>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return crate::macos::copy_text(text, html);
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "linux")]
        if crate::linux_input::is_wayland() {
            return crate::linux_input::copy_wayland(text);
        }
        use std::sync::{Mutex, OnceLock};
        static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
        let mut clipboard = CLIPBOARD.get_or_init(|| Mutex::new(None)).lock().unwrap();
        if clipboard.is_none() {
            *clipboard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        let clipboard = clipboard.as_mut().expect("initialized clipboard");
        match html {
            Some(html) => clipboard.set_html(html, Some(text)),
            None => clipboard.set_text(text),
        }
        .map_err(|error| error.to_string())
    }
}
