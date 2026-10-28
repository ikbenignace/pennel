use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub struct ExternalTarget {
    pub name: String,
    pub app_id: String,
    valid: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    token: String,
    #[cfg(not(target_os = "macos"))]
    target: desktop::Target,
    #[cfg(not(target_os = "macos"))]
    current_only: bool,
}

impl ExternalTarget {
    pub fn cancel(&self) {
        self.valid.store(false, Ordering::SeqCst);
        #[cfg(target_os = "macos")]
        crate::macos::cancel_external_target(&self.token);
    }

    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::SeqCst)
    }
}

pub fn availability() -> (bool, Option<String>) {
    #[cfg(target_os = "linux")]
    if crate::linux_input::is_wayland() {
        return (false, Some("Wayland does not allow automatic external pasting. Copy the text and paste it with Ctrl+V yourself.".into()));
    }
    #[cfg(target_os = "linux")]
    if !crate::linux_input::has_x11_display() {
        return (
            false,
            Some("No X11 session is available to control an external destination app.".into()),
        );
    }
    #[cfg(target_os = "macos")]
    if crate::macos::permissions()
        .get("accessibility")
        .and_then(|value| value.as_str())
        != Some("granted")
    {
        return (
            false,
            Some("Give Pennel Accessibility access to paste into other apps.".into()),
        );
    }
    (true, None)
}

pub fn start_tracking() {
    #[cfg(target_os = "macos")]
    crate::macos::start_external_target_tracking();
    #[cfg(not(target_os = "macos"))]
    desktop::start();
}

pub fn capture() -> Option<ExternalTarget> {
    if !availability().0 {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        let target = crate::macos::external_target()?;
        Some(ExternalTarget {
            name: target.name,
            app_id: target.bundle_id,
            token: target.token,
            valid: Arc::new(AtomicBool::new(true)),
        })
    }
    #[cfg(not(target_os = "macos"))]
    desktop::capture()
}

pub fn capture_current(_app: &tauri::AppHandle) -> Result<ExternalTarget, String> {
    let (available, reason) = availability();
    if !available {
        return Err(reason.unwrap_or_else(|| "External pasting is unavailable.".into()));
    }
    #[cfg(target_os = "macos")]
    let target = crate::macos::current_external_target().map(|target| ExternalTarget {
        name: target.name,
        app_id: target.bundle_id,
        token: target.token,
        valid: Arc::new(AtomicBool::new(true)),
    });
    #[cfg(not(target_os = "macos"))]
    let target = desktop::capture_current();
    target.ok_or_else(|| "No destination app is available. Your dictation is in History.".into())
}

pub fn hint() -> Option<String> {
    let target = capture()?;
    let name = target.name.clone();
    target.cancel();
    Some(name)
}

pub fn is_current(target: &ExternalTarget) -> bool {
    if !target.is_valid() {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::external_target_is_current(&target.token)
    }
    #[cfg(not(target_os = "macos"))]
    {
        desktop::still_selected(target)
    }
}

#[cfg(target_os = "macos")]
pub fn paste(text: &str, target: &ExternalTarget) -> Result<(), String> {
    if !target.is_valid() {
        return Err("External paste was cancelled. Your text is in Pennel.".into());
    }
    crate::macos::paste_external_target(text, &target.token)
}

#[cfg(not(target_os = "macos"))]
pub fn prepare(target: &ExternalTarget) -> Result<(), String> {
    desktop::prepare(target)
}

#[cfg(not(target_os = "macos"))]
pub fn still_selected(target: &ExternalTarget) -> bool {
    desktop::still_selected(target)
}

#[cfg(not(target_os = "macos"))]
mod desktop {
    use super::*;
    use std::sync::{Mutex, OnceLock, Weak};
    use std::time::{Duration, Instant};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Target {
        pub pid: u32,
        pub window: usize,
        pub identity: String,
        pub name: String,
    }

    #[derive(Default)]
    struct Tracker {
        latest: Option<Target>,
        captures: Vec<(Target, bool, Weak<AtomicBool>)>,
    }
    static TRACKER: OnceLock<Arc<Mutex<Tracker>>> = OnceLock::new();

    fn observe(tracker: &mut Tracker, current: Option<Target>) {
        let Some(current) = current else {
            return;
        };
        for (target, current_only, valid) in &tracker.captures {
            if *target != current
                && (*current_only || current.pid != std::process::id())
                && let Some(valid) = valid.upgrade()
            {
                valid.store(false, Ordering::SeqCst);
            }
        }
        tracker
            .captures
            .retain(|(_, _, valid)| valid.strong_count() > 0);
        if current.pid != std::process::id() {
            tracker.latest = Some(current);
        }
    }

    pub fn start() {
        TRACKER.get_or_init(|| {
            let tracker = Arc::new(Mutex::new(Tracker::default()));
            observe(&mut tracker.lock().unwrap(), current());
            let retained = tracker.clone();
            std::thread::spawn(move || {
                loop {
                    observe(&mut retained.lock().unwrap(), current());
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
            tracker
        });
    }

    pub fn capture() -> Option<ExternalTarget> {
        capture_mode(false)
    }

    pub fn capture_current() -> Option<ExternalTarget> {
        capture_mode(true)
    }

    fn capture_mode(current_only: bool) -> Option<ExternalTarget> {
        start();
        let mut tracker = TRACKER.get()?.lock().unwrap();
        let current = current();
        observe(&mut tracker, current.clone());
        let target = if current_only {
            current.filter(|target| target.pid != std::process::id())?
        } else {
            tracker.latest.clone()?
        };
        if process_identity(target.pid)? != target.identity {
            return None;
        }
        let valid = Arc::new(AtomicBool::new(true));
        tracker
            .captures
            .push((target.clone(), current_only, Arc::downgrade(&valid)));
        Some(ExternalTarget {
            name: target.name.clone(),
            app_id: crate::foreground::style_id(&target.name),
            target,
            valid,
            current_only,
        })
    }

    pub fn prepare(captured: &ExternalTarget) -> Result<(), String> {
        let target = &captured.target;
        if !captured.is_valid() || process_identity(target.pid).as_ref() != Some(&target.identity) {
            return Err(
                "The destination app is no longer available. Your text is in Pennel.".into(),
            );
        }
        let foreground = current();
        if foreground.as_ref().is_some_and(|app| app == target) {
            return Ok(());
        }
        if captured.current_only {
            captured.cancel();
            return Err("Another app is active — not pasted. Your dictation is in History.".into());
        }
        if foreground
            .as_ref()
            .is_none_or(|app| app.pid != std::process::id())
        {
            captured.cancel();
            return Err("Another app is active. Your text is in Pennel.".into());
        }
        activate(target)?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while !still_selected(captured) {
            if !captured.is_valid() || Instant::now() >= deadline {
                captured.cancel();
                return Err(
                    "The destination app could not be activated. Your text is in Pennel.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    pub fn still_selected(captured: &ExternalTarget) -> bool {
        captured.is_valid()
            && current().as_ref() == Some(&captured.target)
            && process_identity(captured.target.pid).as_ref() == Some(&captured.target.identity)
    }

    #[cfg(windows)]
    fn process_identity(pid: u32) -> Option<String> {
        use windows::Win32::Foundation::{CloseHandle, FILETIME};
        use windows::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let (mut created, mut exited, mut kernel, mut user) = (
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
            );
            let result =
                GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user);
            let _ = CloseHandle(process);
            result.ok()?;
            Some(format!(
                "{}:{}",
                created.dwHighDateTime, created.dwLowDateTime
            ))
        }
    }

    #[cfg(windows)]
    fn current() -> Option<Target> {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let app = crate::foreground::current()?;
        let window = unsafe { GetForegroundWindow() };
        if window.is_invalid() {
            return None;
        }
        Some(Target {
            pid: app.pid,
            window: window.0 as usize,
            identity: process_identity(app.pid)?,
            name: app.executable,
        })
    }

    #[cfg(windows)]
    fn activate(target: &Target) -> Result<(), String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        if unsafe { SetForegroundWindow(HWND(target.window as *mut std::ffi::c_void)) }.as_bool() {
            Ok(())
        } else {
            Err("Windows could not activate the destination app. Your text is in Pennel.".into())
        }
    }

    #[cfg(target_os = "linux")]
    fn process_identity(pid: u32) -> Option<String> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.rsplit_once(')')?
            .1
            .split_whitespace()
            .nth(19)
            .map(str::to_owned)
    }

    #[cfg(target_os = "linux")]
    fn current() -> Option<Target> {
        if crate::linux_input::is_wayland() {
            return None;
        }
        let (window, pid) = x11::active()?;
        let name = std::fs::read_link(format!("/proc/{pid}/exe"))
            .ok()?
            .file_name()?
            .to_string_lossy()
            .into_owned();
        Some(Target {
            pid,
            window,
            identity: process_identity(pid)?,
            name,
        })
    }

    #[cfg(target_os = "linux")]
    fn activate(target: &Target) -> Result<(), String> {
        x11::activate(target.window).then_some(()).ok_or_else(|| {
            "The destination app could not be activated. Your text is in Pennel.".into()
        })
    }

    #[cfg(target_os = "linux")]
    mod x11 {
        use std::ffi::{c_char, c_int, c_long, c_uchar, c_ulong, c_void};
        use std::ptr;
        #[repr(C)]
        struct ClientMessage {
            kind: c_int,
            serial: c_ulong,
            send: c_int,
            display: *mut c_void,
            window: c_ulong,
            message: c_ulong,
            format: c_int,
            data: [c_long; 5],
        }
        #[repr(C)]
        union Event {
            message: std::mem::ManuallyDrop<ClientMessage>,
            pad: [c_long; 24],
        }
        #[link(name = "X11")]
        unsafe extern "C" {
            fn XOpenDisplay(name: *const c_char) -> *mut c_void;
            fn XCloseDisplay(display: *mut c_void) -> c_int;
            fn XDefaultRootWindow(display: *mut c_void) -> c_ulong;
            fn XInternAtom(
                display: *mut c_void,
                name: *const c_char,
                only_if_exists: c_int,
            ) -> c_ulong;
            fn XGetWindowProperty(
                display: *mut c_void,
                window: c_ulong,
                property: c_ulong,
                offset: c_long,
                length: c_long,
                delete: c_int,
                requested: c_ulong,
                actual: *mut c_ulong,
                format: *mut c_int,
                count: *mut c_ulong,
                remaining: *mut c_ulong,
                data: *mut *mut c_uchar,
            ) -> c_int;
            fn XFree(data: *mut c_void) -> c_int;
            fn XSendEvent(
                display: *mut c_void,
                window: c_ulong,
                propagate: c_int,
                mask: c_long,
                event: *mut Event,
            ) -> c_int;
            fn XFlush(display: *mut c_void) -> c_int;
        }
        unsafe fn property(
            display: *mut c_void,
            window: c_ulong,
            name: &std::ffi::CStr,
        ) -> Option<c_ulong> {
            let atom = unsafe { XInternAtom(display, name.as_ptr(), 0) };
            let (mut actual, mut format, mut count, mut remaining, mut data) =
                (0, 0, 0, 0, ptr::null_mut());
            let result = unsafe {
                XGetWindowProperty(
                    display,
                    window,
                    atom,
                    0,
                    1,
                    0,
                    0,
                    &mut actual,
                    &mut format,
                    &mut count,
                    &mut remaining,
                    &mut data,
                )
            };
            if result != 0 || data.is_null() {
                return None;
            }
            let value = if format == 32 && count > 0 {
                Some(unsafe { *(data.cast::<c_ulong>()) })
            } else {
                None
            };
            unsafe { XFree(data.cast()) };
            value
        }
        pub fn active() -> Option<(usize, u32)> {
            unsafe {
                let display = XOpenDisplay(ptr::null());
                if display.is_null() {
                    return None;
                }
                let result = (|| {
                    let window =
                        property(display, XDefaultRootWindow(display), c"_NET_ACTIVE_WINDOW")?;
                    let pid = property(display, window, c"_NET_WM_PID")?;
                    Some((window as usize, pid as u32))
                })();
                XCloseDisplay(display);
                result
            }
        }
        pub fn activate(window: usize) -> bool {
            unsafe {
                let display = XOpenDisplay(ptr::null());
                if display.is_null() {
                    return false;
                }
                let atom = XInternAtom(display, c"_NET_ACTIVE_WINDOW".as_ptr(), 0);
                let mut event = Event {
                    message: std::mem::ManuallyDrop::new(ClientMessage {
                        kind: 33,
                        serial: 0,
                        send: 1,
                        display,
                        window: window as c_ulong,
                        message: atom,
                        format: 32,
                        data: [1, 0, 0, 0, 0],
                    }),
                };
                let result = XSendEvent(
                    display,
                    XDefaultRootWindow(display),
                    0,
                    (1 << 20) | (1 << 19),
                    &mut event,
                ) != 0;
                XFlush(display);
                XCloseDisplay(display);
                result
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn a_third_app_permanently_invalidates_a_captured_target() {
            let target = Target {
                pid: 42,
                window: 7,
                identity: "first-launch".into(),
                name: "Editor".into(),
            };
            let valid = Arc::new(AtomicBool::new(true));
            let mut tracker = Tracker {
                latest: Some(target.clone()),
                captures: vec![(target.clone(), false, Arc::downgrade(&valid))],
            };
            observe(
                &mut tracker,
                Some(Target {
                    pid: 43,
                    window: 8,
                    identity: "other".into(),
                    name: "Other".into(),
                }),
            );
            observe(&mut tracker, Some(target));
            assert!(!valid.load(Ordering::SeqCst));
        }

        #[test]
        fn global_delivery_is_cancelled_when_pennel_takes_focus_but_pad_delivery_is_retained() {
            let target = Target {
                pid: 42,
                window: 7,
                identity: "editor".into(),
                name: "Editor".into(),
            };
            let global = Arc::new(AtomicBool::new(true));
            let pad = Arc::new(AtomicBool::new(true));
            let mut tracker = Tracker {
                latest: Some(target.clone()),
                captures: vec![
                    (target.clone(), true, Arc::downgrade(&global)),
                    (target.clone(), false, Arc::downgrade(&pad)),
                ],
            };
            observe(
                &mut tracker,
                Some(Target {
                    pid: std::process::id(),
                    window: 9,
                    identity: "pennel".into(),
                    name: "Pennel".into(),
                }),
            );
            assert_eq!(tracker.latest.as_ref(), Some(&target));
            observe(&mut tracker, Some(target));
            assert!(!global.load(Ordering::SeqCst));
            assert!(pad.load(Ordering::SeqCst));
        }
    }
}
