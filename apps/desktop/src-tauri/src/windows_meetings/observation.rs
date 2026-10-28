use std::collections::HashMap;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, DEVICE_STATE_ACTIVE, EDataFlow, IAudioSessionControl2,
    IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::{BOOL, Interface};

use super::{AudioUse, ComApartment, Process, app_identity, is_own_process, observations};

/// Calls on arbitrary application threads are isolated from Tauri's STA and from capture COM state.
pub fn snapshot() -> Result<Vec<pennel_core::CallObservation>, String> {
    std::thread::Builder::new()
        .name("pennel-call-observation".into())
        .spawn(snapshot_mta)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "Meeting detection stopped unexpectedly".to_owned())?
}

fn snapshot_mta() -> Result<Vec<pennel_core::CallObservation>, String> {
    let _apartment = ComApartment::new()?;
    let processes = processes()?;
    let own_pid = std::process::id();
    let titles = browser_titles(&processes, own_pid)?;
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|error| error.to_string())?;
        let mut uses = endpoint_sessions(&enumerator, eCapture)?;
        uses.extend(endpoint_sessions(&enumerator, eRender)?);
        Ok(observations(&uses, &processes, &titles, own_pid))
    }
}

fn endpoint_sessions(
    enumerator: &IMMDeviceEnumerator,
    flow: EDataFlow,
) -> Result<Vec<AudioUse>, String> {
    unsafe {
        let endpoints = enumerator
            .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)
            .map_err(|error| error.to_string())?;
        let count = endpoints.GetCount().map_err(|error| error.to_string())?;
        let mut uses = Vec::new();
        for index in 0..count {
            let Ok(endpoint) = endpoints.Item(index) else {
                continue;
            };
            let Ok(manager) = endpoint.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
                continue;
            };
            let Ok(sessions) = manager.GetSessionEnumerator() else {
                continue;
            };
            let Ok(count) = sessions.GetCount() else {
                continue;
            };
            for index in 0..count {
                let Ok(session) = sessions.GetSession(index) else {
                    continue;
                };
                if session.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(session) = session.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(pid) = session.GetProcessId() else {
                    continue;
                };
                if pid != 0 {
                    uses.push(AudioUse {
                        pid,
                        input: flow == eCapture,
                        output: flow == eRender,
                    });
                }
            }
        }
        Ok(uses)
    }
}

struct SnapshotHandle(HANDLE);

impl Drop for SnapshotHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn processes() -> Result<HashMap<u32, Process>, String> {
    unsafe {
        let handle = SnapshotHandle(
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|error| error.to_string())?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        Process32FirstW(handle.0, &mut entry).map_err(|error| error.to_string())?;
        let mut processes = HashMap::new();
        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|character| *character == 0)
                .unwrap_or(entry.szExeFile.len());
            processes.insert(
                entry.th32ProcessID,
                Process {
                    parent: entry.th32ParentProcessID,
                    executable: String::from_utf16_lossy(&entry.szExeFile[..length]),
                },
            );
            if Process32NextW(handle.0, &mut entry).is_err() {
                break;
            }
        }
        Ok(processes)
    }
}

struct TitleContext<'a> {
    processes: &'a HashMap<u32, Process>,
    own_pid: u32,
    titles: HashMap<u32, Vec<String>>,
}

fn browser_titles(
    processes: &HashMap<u32, Process>,
    own_pid: u32,
) -> Result<HashMap<u32, Vec<String>>, String> {
    let mut context = TitleContext {
        processes,
        own_pid,
        titles: HashMap::new(),
    };
    // EnumWindows invokes the callback synchronously, so this stack pointer never escapes.
    unsafe {
        EnumWindows(
            Some(window_title),
            LPARAM(&mut context as *mut TitleContext<'_> as isize),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(context.titles)
}

unsafe extern "system" fn window_title(window: HWND, context: LPARAM) -> BOOL {
    unsafe {
        let context = &mut *(context.0 as *mut TitleContext<'_>);
        if !IsWindowVisible(window).as_bool() {
            return true.into();
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        if is_own_process(pid, context.own_pid, context.processes)
            || !matches!(app_identity(pid, context.processes), Some((_, true)))
        {
            return true.into();
        }
        let length = GetWindowTextLengthW(window);
        if length > 0 {
            let mut buffer = vec![0u16; (length as usize + 1).min(4096)];
            let read = GetWindowTextW(window, &mut buffer);
            if read > 0 {
                context
                    .titles
                    .entry(pid)
                    .or_default()
                    .push(String::from_utf16_lossy(&buffer[..read as usize]));
            }
        }
        true.into()
    }
}
