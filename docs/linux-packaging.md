# Linux desktop packaging

AppImage builds also require the `file` command (`sudo apt install file` on Debian/Ubuntu). This is a builder prerequisite, not an additional installed-app runtime requirement.

Linux x86-64 is a release target alongside Apple Silicon macOS and Windows x64. CI builds Debian packages and AppImages. Package builds, media integration checks and desktop hardware acceptance are separate evidence.

## Runtime prerequisites

The Debian package declares these dependencies in addition to the WebKitGTK/tray libraries:

```sh
sudo apt install gstreamer1.0-tools gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good gstreamer1.0-libav \
  pulseaudio-utils x11-utils wl-clipboard
```

The native Linux services use fixed GStreamer pipelines for M4A/AAC archive encoding and decoding. Base/Good plug-ins provide parsing, conversion and MP4 handling, and the libav plug-in provides `avenc_aac` and `avdec_aac`. The Bad plug-in set is not required by this configuration. See the upstream [raw-audio parser](https://gstreamer.freedesktop.org/documentation/rawparse/rawaudioparse.html), [AAC parser](https://gstreamer.freedesktop.org/documentation/audioparsers/aacparse.html) and [MP4 muxer](https://gstreamer.freedesktop.org/documentation/isomp4/mp4mux.html) documentation.

`pactl --format=json` observes audio streams through PulseAudio or PipeWire's PulseAudio compatibility server. An installed command is not evidence that a working audio server or microphone is available. X11 application/window context uses `xprop`; Wayland clipboard export uses `wl-copy`.

The AppImage does not currently bundle these external command-line helpers or their codec plug-ins. Install the same runtime packages on the host for the related services. Runtime capability checks must report missing tools, missing codecs and unavailable desktop/audio connections accurately.

## Desktop acceptance

- **X11:** test global press/release gestures, selection reads, guarded paste/clipboard restoration, application styles, automatic call observations, microphone/system tracks, M4A playback/diarization, overlays, mixed-DPI screens and device changes.
- **Wayland:** test the guided compositor-shortcut flow, clipboard-copy fallback, manual meetings, recording/archive playback, call observations and truthful capability explanations. Generic X11 keyboard hooks or synthetic paste must not be advertised as working in native Wayland applications.
- Test each supported environment using the installed Debian package and the AppImage, including missing-helper/codec cases. A container media round trip does not establish desktop accessibility, global-key or compositor behavior.

## Candidate and update artifacts

Linux and Windows x64 distributables append `-DTRANSCRIBE_X86_CONSERVATIVE=ON` to `TRANSCRIBE_CMAKE_ARGS`, retaining existing user arguments. If `CMAKE_ARGS` is also set, the option is appended there too because the dependency applies that variable afterwards. Mac Apple Silicon builds retain their existing configuration.

The CI installer step and signed-candidate helper share this build path. They clean only the selected release target's `transcribe-cpp-sys` artifacts, then require generated CMake caches to show the conservative option enabled and native/x86 SIMD flags disabled. This avoids reusing an earlier `GGML_NATIVE=ON` cache. Explicit user GGML host/SIMD overrides remain preserved but prevent the build from being accepted as a portable candidate. Earlier debug/test results without this check are not portability evidence; retain the new cache verification with each distributable receipt and still test representative CPUs.

The manual signed-candidate workflow includes `x86_64-unknown-linux-gnu` and builds both `.deb` and `.AppImage`. `scripts/desktop-release.py` uses the existing Tauri updater signing key; it never generates keys. The AppImage receives the Tauri updater signature, and the Debian package receives a detached signature using the same key. These Minisign signatures are distinct from Debian archive/repository signing.

The production updater manifest requires the Linux AppImage, Mac updater archive and Windows NSIS installer, their exact accepted hashes, and separate `linux_x11` and `linux_wayland` acceptance results. `scripts/desktop-acceptance.example.json` starts with every gate pending.

Tauri in-app updates on Linux use the AppImage. Debian installations use a manually verified installer or package-manager updates; they must not promise AppImage-style self-updates. Signed candidate production prepares local files only. Publication, the website availability switch and deployment remain separate steps after acceptance.
