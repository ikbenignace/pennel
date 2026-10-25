//! Which app has focus, for per-app style rules and to notice a focus change during a command.
//! Style rules are keyed by macOS bundle ids in the shared core, so known executables map onto them.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundApp {
    pub pid: u32,
    /// Lowercase executable name, e.g. `slack.exe`.
    pub executable: String,
}

#[cfg(any(not(target_os = "macos"), test))]
const KNOWN: &[(&str, &str)] = &[
    ("slack", "com.tinyspeck.slackmacgap"),
    ("whatsapp", "net.whatsapp.WhatsApp"),
    ("telegram", "ru.keepcoder.Telegram"),
    ("outlook", "com.microsoft.Outlook"),
    ("olk", "com.microsoft.Outlook"),
    ("thunderbird", "com.apple.mail"),
    ("winword", "com.microsoft.Word"),
    ("claude", "com.anthropic.claudefordesktop"),
    ("chatgpt", "com.openai.chat"),
    ("windowsterminal", "com.apple.Terminal"),
    ("cmd", "com.apple.Terminal"),
    ("powershell", "com.apple.Terminal"),
    ("pwsh", "com.apple.Terminal"),
    ("gnome-terminal-server", "com.apple.Terminal"),
    ("konsole", "com.apple.Terminal"),
    ("kitty", "com.apple.Terminal"),
    ("alacritty", "com.apple.Terminal"),
    ("wezterm-gui", "com.apple.Terminal"),
    ("ghostty", "com.mitchellh.ghostty"),
    ("code", "com.microsoft.VSCode"),
    ("zed", "dev.zed.Zed"),
    ("cursor", "com.todesktop.230313mzl4w4u92"),
];

/// The id the core's style rules use for this executable: a known bundle id, else the executable name.
#[cfg(any(not(target_os = "macos"), test))]
pub fn style_id(executable: &str) -> String {
    if executable.contains('.') && !executable.to_lowercase().ends_with(".exe") {
        return executable.to_owned();
    }
    let stem = executable
        .to_lowercase()
        .trim_end_matches(".exe")
        .to_owned();
    KNOWN
        .iter()
        .find(|(name, _)| *name == stem)
        .map_or(stem, |(_, id)| (*id).to_owned())
}

#[cfg(windows)]
pub fn current() -> Option<ForegroundApp> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        named.ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        let executable = path.rsplit(['\\', '/']).next()?.to_lowercase();
        Some(ForegroundApp { pid, executable })
    }
}

#[cfg(target_os = "linux")]
pub fn current() -> Option<ForegroundApp> {
    use crate::linux_input::{bounded_output, parse_active_window, parse_window_pid};
    use std::time::Duration;

    if crate::linux_input::is_wayland() || !crate::linux_input::has_x11_display() {
        return None;
    }
    let active = bounded_output(
        "xprop",
        &["-root", "_NET_ACTIVE_WINDOW"],
        Duration::from_millis(100),
    )?;
    if !active.status.success() {
        return None;
    }
    let window = parse_active_window(std::str::from_utf8(&active.stdout).ok()?)?;
    let properties = bounded_output(
        "xprop",
        &["-id", &window, "_NET_WM_PID"],
        Duration::from_millis(100),
    )?;
    if !properties.status.success() {
        return None;
    }
    let pid = parse_window_pid(std::str::from_utf8(&properties.stdout).ok()?)?;
    let executable = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()?
        .file_name()?
        .to_str()?
        .to_lowercase();
    Some(ForegroundApp { pid, executable })
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn current() -> Option<ForegroundApp> {
    None
}

#[cfg(target_os = "macos")]
pub fn current() -> Option<ForegroundApp> {
    crate::macos::foreground().map(|app| ForegroundApp {
        pid: app.pid,
        executable: app.bundle_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_executables_map_to_style_ids() {
        assert_eq!(style_id("Slack.exe"), "com.tinyspeck.slackmacgap");
        assert_eq!(style_id("WindowsTerminal.exe"), "com.apple.Terminal");
        assert_eq!(style_id("code"), "com.microsoft.VSCode");
    }

    #[test]
    fn unknown_executables_keep_their_name() {
        assert_eq!(style_id("notepad.exe"), "notepad");
    }

    #[test]
    fn mac_bundle_ids_keep_their_case() {
        assert_eq!(style_id("com.microsoft.VSCode"), "com.microsoft.VSCode");
    }
}
