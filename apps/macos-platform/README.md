# Pennel Mac platform library

This static Swift library embeds the existing Apple integrations in the shared Tauri application.
It has no SwiftUI, generated PennelCore bindings, product engine or independent persistence layer.
The original native application remains available as the migration reference.

`apps/desktop/src-tauri/build.rs` compiles the library for macOS 14.4+ and links it into Rust. Xcode
with the macOS 26 SDK or newer is required to compile Apple Speech and Foundation Models; execution
of those providers is guarded by macOS 26 availability. Foundation Models is weak-linked to retain
startup support on macOS 14.4. The shared app currently targets Apple Silicon.

```sh
swift test --package-path apps/macos-platform
swift build --package-path apps/macos-platform --configuration release
```

The tests inspect permission status without requesting access and use synthetic audio for M4A
round-trip and two-track timeline tests. They do not start microphone or system audio recording.

## Bridge lifetime and threading

The owned Rust wrappers live in `apps/desktop/src-tauri/src/macos.rs`.

- Strings returned directly by native functions are owned and released with `pennel_mac_string_free`.
- Text and PCM delivered to a callback are borrowed only for that call. Rust copies retained data.
- Capture/hotkey handles are retained Swift objects. Native stop closes callback admission and drains
  pending track writers before returning; only then may Rust free the callback context.
- AppKit, hotkeys, capture lifetime, clipboard and calendar operations run on the main queue. Call
  blocking wrappers from a worker while the application main loop is active. Do not hold shared
  application locks while waiting for native/main-queue work.
- `other_instances()` reads application identities and PIDs only. Canonical-data startup uses it
  to reject concurrent older Pennel/Musli applications that do not yet honor the new core locks.
- Selection, paste, Apple Speech and Apple Intelligence complete through one terminal callback. The Rust wrappers reject
  these blocking operations on the main thread. Permission requests return after initiating an
  explicit user request; inspect permissions again to obtain the eventual outcome.
- Apple Intelligence generation copies the owned message array before returning to Rust, uses only
  the on-device system model and returns sanitized errors without input/output text. It checks
  availability on every request, uses a fresh session without tools and cancels after the supplied
  timeout. The native exact-token check supplements the core's conservative prompt budget.
- Audio source/track `0` is microphone / “Ik”; `1` is system output / “Anderen”. PCM is 16 kHz mono
  Float32. Each callback includes the starting offset in milliseconds on the recording timeline.

## Audio and platform behavior

New Mac meetings receive core-resolved `audio/microphone.m4a` and `audio/system.m4a` AAC paths. The compatibility ABI retains the legacy names. Both writers share one monotonic origin;
device interruption gaps are written as silence, and acquisition timestamps use the same archive
timeline. The effective tap stream format is resolved after aggregate creation, with debounced
format/rate/route recovery and bounded content-free capture diagnostics. Capture preserves the existing Core Audio process-tap and AVAudioEngine voice-processing
workarounds, device-change recovery and silent-capture fallbacks. Archive failures are returned on
stop, allowing Rust to finalize the transcript while reporting the storage error.

Core Audio system-capture authorization has no public preflight used by this library. Its initial
permission state is `unknown`; capture availability and observed system audio are reported by the
meeting handle. This is distinct from denied microphone, Accessibility, speech or calendar access.

Window helpers operate only on a live NSWindow owned by the calling Tauri process. They retain a
real nonactivating NSPanel and move the existing webview presentation container into that panel.
They preserve the Tauri window's class, synchronize its frame on show, and never activate a key/main
window. Both show and hide must use the native helpers; Tauri's original window stays hidden.
Hidden-window tests cover class/lifetime preservation, nonactivating flags and click-through versus
interactive content. Packaged full-screen, multi-monitor and focus behavior still requires device
acceptance testing.
