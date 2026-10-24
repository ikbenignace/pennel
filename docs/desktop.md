# Pennel for Windows and Linux

`apps/desktop/` is a Tauri v2 app: a Rust backend that links `pennel-core` directly and a small
Vite + TypeScript webview (no framework, ~20 kB of JS). The Mac app stays the native Swift app; the
desktop app reuses the same core for transcription, cleanup, commands, history, local models and
meetings, and ports only the platform glue.

## Architecture

```
apps/desktop/
  index.html, pill.html, src/        webview: settings, history, meetings, the pill
  src-tauri/src/
    main.rs          setup, tray, windows, data folder
    dictation.rs     controller (port of DictationController.swift) + paste worker
    gesture.rs       hold-to-talk / double-tap hands-free (port of HotkeyGesture.swift)
    status.rs        what the pill shows, Shift command intent (DictationStatus, CommandIntent)
    paste_queue.rs   pastes in recording order (PasteQueue)
    clipboard.rs     guarded paste + selection read (PasteboardGuard, SelectionReader)
    clipboard_system.rs  Windows native clipboard / arboard on Linux
    hotkey/          TriggerTracker + Windows hook + Linux X11 listener
    keys.rs          synthetic Ctrl+V / Ctrl+C
    audio.rs, resample.rs  cpal capture to 16 kHz mono f32
    meeting.rs       mic + system audio into MeetingSession
    secrets.rs       API keys via the keyring crate
    diagnostics.rs   pennel.log
    commands.rs      the webview API
```

Threads: the hotkey hook posts events to the dictation controller thread; each dictation runs the
core on its own thread; one paste worker owns the clipboard so pastes and selection reads never
overlap. cpal streams live on their own threads because they are not `Send` everywhere.

Data lives in the local (non-roaming) data folder: `%LOCALAPPDATA%\Pennel` on Windows,
`~/.local/share/Pennel` on Linux (settings, history, models, `logs/pennel.log`). Meetings go to
`~/Pennel` (Windows: `%USERPROFILE%\Pennel`) unless configured otherwise. A development build on a
Mac uses `~/Library/Application Support/Pennel Desktop` so it never touches the Mac app's data.

## Platform specifics

### Hotkey

Same gestures as the Mac: hold to talk, double-tap for hands-free (tap again to stop), another key
while holding cancels (the key was used as a modifier), Shift while holding turns the dictation
into a command on the selected text. Choices: **Right Ctrl** (default), **Right Alt**,
**Ctrl + Windows/Super**. Settings store these as new `HotkeyTrigger` variants (`right_control`,
`control_super`; Right Alt reuses `right_option`).

- **Windows:** a `WH_KEYBOARD_LL` hook on its own message-loop thread. It is listen-only, needs no
  admin rights, and ignores injected events (our own Ctrl+V). Keys typed into an elevated
  (administrator) window are not delivered to a non-elevated hook, and pasting into such windows is
  blocked by UIPI. For the Ctrl+Win chord Pennel taps the unassigned key `0xE8` while Win is down so
  releasing Win does not open the Start menu. On layouts with AltGr, Right Alt also sends a fake
  Left Ctrl; that is treated as a modifier and does not cancel.
- **Linux (X11):** rdev listens through XRecord. Works on X11 sessions.
- **Linux (Wayland):** there is no portable way for an app to see global key events. Through
  XWayland rdev only sees keys while an X11 app has focus, so the hotkey effectively does not work
  in native Wayland apps (GNOME, KDE). Workarounds: log in with an X11 session, or (future work) read
  `/dev/input` via evdev for users in the `input` group, or the GlobalShortcuts portal (which only
  reports press/release of a registered shortcut, enough for hold-to-talk but not for "other key
  cancels"). Super-based chords may also open the GNOME overview on release.

### Paste and command mode

Port of `PasteboardGuard`: snapshot the clipboard, write the text, send Ctrl+V, wait 700 ms, and
restore the snapshot unless something else was copied meanwhile. Before sending keys Pennel waits
(max 1.5 s) until the hotkey and Shift are released, since a held Right Alt (AltGr) or Shift would
turn Ctrl+V into another shortcut.

- **Windows:** every clipboard format is copied byte for byte (text, RTF/HTML, files, DIB images;
  GDI handle formats are re-synthesized by Windows). `GetClipboardSequenceNumber` detects a newer
  copy. Dictated text is marked `ExcludeClipboardContentFromMonitorProcessing`,
  `CanIncludeInClipboardHistory=0` and `CanUploadToCloudClipboard=0`, so it stays out of Win+V
  history and cloud clipboard. Keys are sent with `SendInput`.
- **Linux:** arboard (X11 or XWayland clipboard); only text or one image is restored. Change
  detection compares the text. Dictated text carries the KDE password-manager hint so clipboard
  managers skip it. Keys go through enigo (XTest). On Wayland, synthetic keys only reach XWayland
  apps.

Command mode reads the selection by putting a unique sentinel on the clipboard, sending Ctrl+C and
waiting up to 300 ms for the clipboard to change; the user's clipboard is restored afterwards, and
again if a slow app answers within 2 s. On Windows Pennel also checks that the same process still
has focus before pasting the result, like the Mac app.

Style per app: on Windows the foreground executable is mapped onto the Mac bundle ids used by the
core's style rules (`slack.exe` → Slack's rule, `WindowsTerminal.exe` → Terminal, ...); unknown
apps can be added by executable name. Linux does not detect the active app yet (everything is
Neutral).

### Audio

cpal opens the default input; any rate and channel count is downmixed and resampled to 16 kHz mono
(box-filtered decimation, linear interpolation when upsampling). Meetings:

- **Windows:** WASAPI loopback of the default output device (cpal builds an input stream on the
  output device).
- **Linux:** cpal's PulseAudio host (pure Rust, also works with PipeWire's pulse server) records the
  `<default sink>.monitor` source.
- If system audio cannot be opened the meeting records the microphone only and says so.
- No echo cancellation on the desktop yet: with speakers instead of a headset the microphone track
  also picks up the other side. Dictation during a meeting silences the "Ik" track and marks the
  period, as on the Mac.

### The pill

A second webview window: transparent, undecorated, always on top, not in the taskbar,
`focusable(false)` and click-through. On Windows it is shown with `SW_SHOWNOACTIVATE` and
`HWND_TOPMOST` so it never takes focus from the app you dictate into. It sits bottom-center on the
work area of the monitor that holds the mouse. Wayland compositors decide window placement
themselves, so there it may appear elsewhere; transparency needs a compositor on X11.

### Keys and privacy

API keys live in Windows Credential Manager or the Secret Service (GNOME Keyring, KWallet) under
service `app.pennel.desktop`, read once per launch. The log records timings, lengths and errors,
never dictated or meeting text. No telemetry.

## Build and run

Prerequisites everywhere: Rust (stable), Node 22+, CMake.

- **Windows:** Visual Studio Build Tools (MSVC, "Desktop development with C++"); WebView2 ships with
  Windows 10/11.
- **Linux (Debian/Ubuntu):**
  `sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libx11-dev libxtst-dev libxi-dev libdbus-1-dev patchelf cmake build-essential`

```sh
cd apps/desktop
npm ci
npx tauri dev                    # run with hot reload
npx tauri build                  # installers in src-tauri/target/release/bundle/
cargo test --manifest-path src-tauri/Cargo.toml
```

Local speech models run on the CPU. For a Vulkan GPU build: `npx tauri build -- --features vulkan`
(needs the Vulkan SDK).

`npx tauri dev` also works on a Mac for UI work: the window, pill, settings, history and meetings
run, but there is no global hotkey there (use the native Mac app).

CI (`.github/workflows/desktop.yml`) builds and tests `pennel-core` and the app on
`windows-latest` and `ubuntu-22.04` and uploads unsigned NSIS/MSI and deb/AppImage installers as
workflow artifacts.

## Verified vs. not verified

Verified:
- `pennel-core` builds and its tests pass on Windows, Linux and macOS (CI); the Mac app still builds
  and its tests pass.
- The desktop crate compiles, passes clippy and its unit tests (gesture, trigger tracking, status,
  paste queue, clipboard guard, resampler, notices, log rotation) on Windows and Linux in CI, and
  produces installers there.
- On macOS (development): the app starts, creates its tray icon and windows, and the settings UI
  works.

Not verified on real hardware yet (needs a manual pass on a Windows 11 machine and an X11 + a Wayland
Linux desktop):
- the keyboard hook and gestures, the Ctrl+Win Start-menu suppression;
- the pill never stealing focus, its position on multi-monitor / mixed-DPI setups;
- paste and clipboard restore in Office, browsers, Electron apps, terminals;
- command mode selection reads;
- microphone capture on various devices, WASAPI loopback and the PulseAudio monitor;
- Credential Manager / Secret Service storage;
- installers (unsigned: SmartScreen will warn).

## Known gaps

- Wayland: hotkey and paste only work with XWayland apps (see above).
- Linux: no active-app detection for per-app styles; clipboard restore keeps text or one image only.
- No echo cancellation, call detection or calendar integration for meetings; no diarization UI,
  meeting chat or export on the desktop yet (the core APIs exist: `diarize_meeting`, `ask_meetings`,
  `export_meeting`).
- Meeting audio is not saved as files (the Mac app keeps `ik.m4a`/`anderen.m4a`), so diarization
  cannot run afterwards.
- No onboarding wizard (the app opens on Providers when dictation is not set up), no auto-start at
  login, no auto-update, unsigned installers.
