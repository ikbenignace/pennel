#[cfg(windows)]
pub use win::SystemClipboard;

#[cfg(not(windows))]
pub use portable::SystemClipboard;

/// Linux and macOS (development builds): text and images through arboard. X11 has no change
/// counter, so the fingerprint hashes the current text.
#[cfg(not(windows))]
mod portable {
    use std::hash::{Hash, Hasher};

    use arboard::ImageData;

    use crate::clipboard::Clipboard;

    #[derive(Clone)]
    pub enum Snapshot {
        Empty,
        Text(String),
        Image(ImageData<'static>),
    }

    pub struct SystemClipboard {
        inner: Option<arboard::Clipboard>,
    }

    impl SystemClipboard {
        pub fn new() -> Self {
            Self {
                inner: arboard::Clipboard::new().ok(),
            }
        }

        fn inner(&mut self) -> Option<&mut arboard::Clipboard> {
            if self.inner.is_none() {
                self.inner = arboard::Clipboard::new().ok();
            }
            self.inner.as_mut()
        }
    }

    impl Clipboard for SystemClipboard {
        type Snapshot = Snapshot;

        fn fingerprint(&mut self) -> u64 {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.text().hash(&mut hasher);
            hasher.finish()
        }

        fn snapshot(&mut self) -> Snapshot {
            let Some(clipboard) = self.inner() else {
                return Snapshot::Empty;
            };
            if let Ok(text) = clipboard.get_text() {
                return Snapshot::Text(text);
            }
            clipboard
                .get_image()
                .map(Snapshot::Image)
                .unwrap_or(Snapshot::Empty)
        }

        fn restore(&mut self, snapshot: &Snapshot) {
            let Some(clipboard) = self.inner() else {
                return;
            };
            let _ = match snapshot {
                Snapshot::Empty => clipboard.clear(),
                Snapshot::Text(text) => clipboard.set_text(text.clone()),
                Snapshot::Image(image) => clipboard.set_image(image.clone()),
            };
        }

        fn set_transient_text(&mut self, text: &str) {
            #[cfg(target_os = "macos")]
            use arboard::SetExtApple;
            #[cfg(target_os = "linux")]
            use arboard::SetExtLinux;
            if let Some(clipboard) = self.inner() {
                let _ = clipboard.set().exclude_from_history().text(text.to_owned());
            }
        }

        fn text(&mut self) -> Option<String> {
            self.inner()?.get_text().ok()
        }
    }
}

/// Windows: every clipboard format is copied byte for byte so rich text, files and images survive,
/// and `GetClipboardSequenceNumber` tells whether anyone wrote in between.
#[cfg(windows)]
mod win {
    use std::time::Duration;

    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
        GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
    };
    use windows::core::w;

    use crate::clipboard::Clipboard;

    const CF_UNICODETEXT: u32 = 13;
    /// GDI handles rather than memory; Windows synthesizes them again from the formats we keep.
    const HANDLE_FORMATS: [u32; 7] = [2, 3, 9, 14, 0x80, 0x82, 0x83];
    const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

    pub struct SystemClipboard;

    impl SystemClipboard {
        pub fn new() -> Self {
            Self
        }
    }

    /// Another app may hold the clipboard open for a moment.
    struct Open;

    impl Open {
        fn new() -> Option<Self> {
            for _ in 0..20 {
                if unsafe { OpenClipboard(None) }.is_ok() {
                    return Some(Open);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            None
        }
    }

    impl Drop for Open {
        fn drop(&mut self) {
            let _ = unsafe { CloseClipboard() };
        }
    }

    unsafe fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
        let global = HGLOBAL(handle.0);
        let size = unsafe { GlobalSize(global) };
        if size == 0 || size > MAX_SNAPSHOT_BYTES {
            return None;
        }
        let ptr = unsafe { GlobalLock(global) } as *const u8;
        if ptr.is_null() {
            return None;
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
        let _ = unsafe { GlobalUnlock(global) };
        Some(bytes)
    }

    unsafe fn write_global(format: u32, bytes: &[u8]) {
        let Ok(global) = (unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }) else {
            return;
        };
        let ptr = unsafe { GlobalLock(global) } as *mut u8;
        if ptr.is_null() {
            let _ = unsafe { GlobalFree(Some(global)) };
            return;
        }
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
        let _ = unsafe { GlobalUnlock(global) };
        if unsafe { SetClipboardData(format, Some(HANDLE(global.0))) }.is_err() {
            let _ = unsafe { GlobalFree(Some(global)) };
        }
    }

    impl Clipboard for SystemClipboard {
        type Snapshot = Vec<(u32, Vec<u8>)>;

        fn fingerprint(&mut self) -> u64 {
            u64::from(unsafe { GetClipboardSequenceNumber() })
        }

        fn snapshot(&mut self) -> Self::Snapshot {
            let Some(_open) = Open::new() else {
                return Vec::new();
            };
            let mut formats = Vec::new();
            let mut format = 0;
            let mut total = 0;
            loop {
                format = unsafe { EnumClipboardFormats(format) };
                if format == 0 {
                    break;
                }
                if HANDLE_FORMATS.contains(&format) {
                    continue;
                }
                let Ok(handle) = (unsafe { GetClipboardData(format) }) else {
                    continue;
                };
                if let Some(bytes) = unsafe { read_global(handle) } {
                    total += bytes.len();
                    if total > MAX_SNAPSHOT_BYTES {
                        break;
                    }
                    formats.push((format, bytes));
                }
            }
            formats
        }

        fn restore(&mut self, snapshot: &Self::Snapshot) {
            let Some(_open) = Open::new() else {
                return;
            };
            let _ = unsafe { EmptyClipboard() };
            for (format, bytes) in snapshot {
                unsafe { write_global(*format, bytes) };
            }
        }

        fn set_transient_text(&mut self, text: &str) {
            let Some(_open) = Open::new() else {
                return;
            };
            let _ = unsafe { EmptyClipboard() };
            let wide: Vec<u8> = text
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            unsafe {
                write_global(CF_UNICODETEXT, &wide);
                let exclude =
                    RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
                write_global(exclude, &[0]);
                for name in [
                    w!("CanIncludeInClipboardHistory"),
                    w!("CanUploadToCloudClipboard"),
                ] {
                    write_global(RegisterClipboardFormatW(name), &0u32.to_le_bytes());
                }
            }
        }

        fn text(&mut self) -> Option<String> {
            let _open = Open::new()?;
            let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }.ok()?;
            let bytes = unsafe { read_global(handle) }?;
            let wide: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .take_while(|unit| *unit != 0)
                .collect();
            Some(String::from_utf16_lossy(&wide))
        }
    }
}
