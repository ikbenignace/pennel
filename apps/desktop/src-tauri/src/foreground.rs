//! Which app has focus, for per-app style rules and to notice a focus change during a command.
//! Style rules are keyed by macOS bundle ids in the shared core, so known executables map onto them.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundApp {
    pub pid: u32,
    /// Lowercase executable name, e.g. `slack.exe`.
    pub executable: String,
}

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
pub fn style_id(executable: &str) -> String {
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

/// Without a portable way to ask a Wayland compositor, Linux leaves style rules on Neutral.
#[cfg(not(windows))]
pub fn current() -> Option<ForegroundApp> {
    None
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
}
