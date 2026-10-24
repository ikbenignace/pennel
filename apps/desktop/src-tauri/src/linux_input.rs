//! Wayland never pretends that XTest can paste into native applications.

use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const SHORTCUT_HELP: &str = "Wayland: stel in je desktopinstellingen een sneltoets in voor pennel --toggle-dictation. Pennel kopieert de tekst; plak zelf met Ctrl+V.";

pub fn session_is_wayland(session_type: Option<&str>, display: Option<&str>) -> bool {
    session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
        || display.is_some_and(|value| !value.trim().is_empty())
}

pub fn is_wayland() -> bool {
    session_is_wayland(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

pub fn has_x11_display() -> bool {
    std::env::var("DISPLAY").is_ok_and(|value| !value.trim().is_empty())
}

/// Fixed executable and argument arrays only; callers never pass a shell command.
pub fn bounded_output(program: &str, arguments: &[&str], timeout: Duration) -> Option<Output> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// wl-copy owns the Wayland selection in its daemon after this process finishes.
/// Text is passed through stdin, never command arguments or logs.
pub fn copy_wayland(text: &str) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .args(["--type", "text/plain;charset=utf-8"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Wayland-klembord niet beschikbaar — installeer wl-clipboard".to_owned())?;
    let Some(mut input) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Wayland-klembord kon niet worden geopend".into());
    };
    let owned = text.to_owned();
    let writer = std::thread::spawn(move || input.write_all(owned.as_bytes()));
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("Wayland-klembord reageert niet".to_owned());
            }
        }
    };
    let written = writer
        .join()
        .map_err(|_| "Kopiëren naar het klembord is onderbroken".to_owned())?;
    written.map_err(|_| "Tekst kon niet naar het Wayland-klembord worden geschreven".to_owned())?;
    if status?.success() {
        Ok(())
    } else {
        Err("Wayland-klembord weigerde de tekst".into())
    }
}

pub fn parse_active_window(output: &str) -> Option<String> {
    let value = output.split_whitespace().last()?;
    let hexadecimal = value.strip_prefix("0x")?;
    let id = u64::from_str_radix(hexadecimal, 16).ok()?;
    (id != 0).then(|| format!("0x{id:x}"))
}

pub fn parse_window_pid(output: &str) -> Option<u32> {
    let pid = output.split_once('=')?.1.trim().parse().ok()?;
    (pid != 0).then_some(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_wins_over_an_xwayland_display() {
        assert!(session_is_wayland(Some("wayland"), None));
        assert!(session_is_wayland(Some("x11"), Some("wayland-0")));
        assert!(session_is_wayland(None, Some("wayland-1")));
        assert!(!session_is_wayland(Some("x11"), None));
        assert!(!session_is_wayland(None, Some("")));
    }

    #[test]
    fn active_window_ids_are_numeric_only() {
        assert_eq!(
            parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x12ab"),
            Some("0x12ab".into())
        );
        assert_eq!(
            parse_active_window("_NET_ACTIVE_WINDOW: window id # 0x0"),
            None
        );
        assert_eq!(parse_active_window("0x12;touch /tmp/no"), None);
        assert_eq!(parse_active_window("missing property"), None);
    }

    #[test]
    fn window_pid_must_be_nonzero_and_well_formed() {
        assert_eq!(parse_window_pid("_NET_WM_PID(CARDINAL) = 42"), Some(42));
        assert_eq!(parse_window_pid("_NET_WM_PID(CARDINAL) = 0"), None);
        assert_eq!(parse_window_pid("_NET_WM_PID(CARDINAL) = 42, 43"), None);
        assert_eq!(parse_window_pid("_NET_WM_PID: no such atom"), None);
    }

    #[test]
    fn native_queries_have_a_deadline() {
        let started = Instant::now();
        assert!(bounded_output("/bin/sleep", &["1"], Duration::from_millis(20)).is_none());
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
