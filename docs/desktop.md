# Shared Pennel desktop app

`apps/desktop/` hosts one Tauri 2 application for Mac, Windows and Linux. Its TypeScript/Vite interface links through Rust commands to one `PennelEngine`. The native Mac presentation remains a comparison/recovery build during migration; it is not the shared application's host.

As in the original Pennel, the main window **Pennel** is the meeting library: its sidebar holds search, **Vraag het je meetings**, the live recording and meetings grouped by **Vandaag, Deze week, Eerder**. **Instellingen** (centred tabs), **Geschiedenis** and **Dicteren** (direct dictation and the pad) open in their own titled windows from the tray menu, `Cmd/Ctrl+,` and links inside the app. Library/search/chat, live and completed meetings, notes/transcript/questions, speaker naming/recognition, templates, exports, history, the recording prompt and the dictation pill remain available. Settings retain this order and Dutch copy: **Providers, Modellen, Dicteren, Meetings, Opnemen, Woordenlijst, Stijlen**, followed by **Weergave**. Platform-specific features remain visible with capability/permission explanations.

## Appearance and first use

**Systeem, Licht and Donker** use the shared semantic tokens in `assets/brand/tokens.json`. `make theme` generates the CSS and native Swift values; `make theme-check` checks that the committed outputs match. The interface uses bundled Figtree. Instrument Serif remains the wordmark face. The floating pill has separate colors so changing the window appearance does not invert it.

New profiles use local speech recognition in **Snel** mode and open a separate headerless wizard: welcome, language/speech recognition, access, a real first dictation and an optional **Leer je stem** step. A successful practice resumes at that step; finishing or skipping it completes the wizard. The model download and provider credentials use the existing core and OS credential services. A saved wizard step is not proof that a model or permission remains available. Existing profiles keep their providers, model choices and dictation mode and can resume setup when necessary. Windows and Linux open the workspace at launch; Mac uses the menu bar lifecycle below.

The in-app dictation pad retains an editable local draft. **Dicteerpad ook extern plakken** is off by default and applies only to recordings made in that pad. Enabling it captures the previous external target when recording starts; it does not change normal shortcut dictation in other apps. If the captured target cannot be used, the text remains in the pad. Onboarding practice is a separate owned session: it transcribes with a temporary Fast setting and never uses external paste or clipboard delivery.

Window closing and navigation invalidate pending owned external delivery. Meeting recording and the normal background runtime retain their existing lifetime. Real device and platform acceptance are recorded separately in [redesign acceptance](redesign-acceptance.md).

### Mac menu bar lifecycle

The shared Tauri app lives in the Mac menu bar, without a Dock icon or Cmd-Tab entry, including while its windows are open. First launch or unfinished setup opens onboarding. Completing or skipping setup opens **Dicteren** once; later cold launches stay in the menu bar.

Click the Pennel icon for the native action menu. **Start opname**, **Stop opname** and **Opname afronden…** reflect the recording runtime; the icon distinguishes an active recording and the dictation hint reports actual shortcut availability. **Open Pennel** opens the meeting library, or resumes unfinished onboarding. Opening the running app through Finder or Spotlight does the same. Settings and history actions open their respective windows.

Closing a product window saves its drafts before hiding it, without changing the other windows. If saving fails or times out, Pennel keeps that window open with an error. Reopening restores its owned session and refreshes the view; presentation polling pauses while hidden. Global dictation, the pill and meeting recording continue independently of window visibility. **Stop Pennel** uses the existing shutdown path to save drafts and finalize recordings.

## Dictation destinations and live text

### Saving and recovery

The shared app keeps completed dictation text available even when History cannot write to SQLite. **Nog niet bewaard** entries appear first in History, with **Kopiëren**, **Opnieuw bewaren** and explicit discard. Saving again never repeats recognition or pastes into another application. A failed save cancels automatic placement in the original destination; recovery notices refer to the exact dictation rather than whichever result finished last. Clipboard-write failures also stop placement instead of sending a paste command with stale clipboard contents.

Temporary recovery entries live only in the running process. Pennel tries to save them before quitting or updating and stays open when saving still fails. Explicitly discarding an entry works even when SQLite is unavailable. New dictation is blocked at twenty retained/in-flight results or the five-MiB recovery budget; an unexpectedly oversized completed result is kept intact and blocks further capture until resolved. Closing a window does not discard temporary results. A process or computer crash cannot preserve memory-only text.

Meeting audio chunks are durably written before recognition. Recognition queues file references rather than unbounded PCM; the capture ingress is bounded. Transcript replacement is atomic and recovery audio is removed only after its matching transcript is committed. Repeated storage failures stop capture with a persistent warning and retain the remaining material for **Opnieuw bewaren**, which does not rerun successful recognition. Pending WAVs are discovered after a restart. New recovery filenames and optional millisecond transcript timestamps prevent short chunks in the same second from overwriting one another; existing whole-second files remain readable, and ordinary timestamp display/export is unchanged.

Two consecutive failed recognition chunks pause further recognition requests for that recording while audio continues to be saved. The recording screen explains the pause; failed chunks can be transcribed explicitly after recording. Fatal capture/storage errors stop recording, retain notes and title drafts, and prevent a new recording from replacing unsaved material.

Speech provider, model, language, dictionary and credentials are frozen when a meeting starts. The live screen reports the frozen, nonsecret recognition configuration. Later settings edits apply to the next recording; summaries and questions use the model chosen when requested. Cloud account changes are blocked during an active meeting, while normal token refresh for the same account remains available.

**Dicteren** opens the global workflow. Click a text field in another app, hold the configured shortcut and release to paste there. The main Pennel window does not need to be open. **Open dicteerpad** is an explicit secondary action; its existing local-draft and optional external-paste behavior remain separate from global dictation.

Global dictation freezes an opaque external target when recording stops, before asynchronous transcription and cleanup. Provider, language, mode and dictionary are frozen at recording start; app-specific style context comes from the same stop target as delivery. A changed or unavailable target is never replaced with another recipient. The completed result remains in History with a recovery message. Mac additionally checks the captured Accessibility text-field identity; Windows/X11 currently guard the app, window and process-launch identity rather than exact field identity. Wayland retains its explicit copy-only behavior.

`LiveDictationSession` owns preview IDs, monotonic revisions and cancellation in the shared core. Apple uses the existing selected system provider through `SpeechAnalyzer` with volatile results. Local models use one cancellable decode at a time over a bounded six-second audio tail. Cloud providers remain one final ASR request; their raw result can appear before cleanup, without extra preview requests. Preview cannot write history, run cleanup/commands, read selections or paste. The authoritative final pass uses the same frozen settings. Closing or cancelling an owned pad/practice session also cancels pending finalization, preventing later result/history commits.

The non-activating pill shows up to two recent lines and keeps the last caption while processing. Session, revision and owner guards reject stale text and audio-level updates. Its dimensions and colors come from shared tokens. Apple preview stop closes and drains callbacks, then waits for analyzer cleanup off the UI thread before definitive recognition.

Logitech MX keyboards do not transmit Apple's Fn/Globe keycode. They can use **Rechter Option** in **Instellingen → Dicteren → Sneltoets**; `fn` remains available for Apple keyboards. [Logitech's documentation](https://support.logi.com/hc/en-us/articles/29752695205527-Logitech-keyboard-FN-key-behavior-on-macOS-or-iOS).

The delivery and caption behavior was compared with [FluidVoice's stop-target policy](https://github.com/altic-dev/FluidVoice/blob/0fccdeb014462e6f82889af38c7e9de92d87a130/Sources/Fluid/Services/DictationTargetPolicy.swift) and [live recognition loop](https://github.com/altic-dev/FluidVoice/blob/0fccdeb014462e6f82889af38c7e9de92d87a130/Sources/Fluid/Services/ASRService.swift#L6982-L7278). Provider limitations differ; Pennel does not claim that a batch cloud endpoint streams during recording.

## Architecture and lifetime

| Layer | Responsibility |
| --- | --- |
| `core/` | Providers, processing, gestures/paste ordering, call debounce/rules, recording lifecycle, timestamped ingestion, stores, models, search/chat/templates/diarization/export |
| Tauri Rust host | One engine, application/background lifetime, command/event contracts, recording and dictation workers, native adapters, window/tray management and updates |
| Shared TypeScript views | Render snapshots and send explicit actions; no microphone capture or credential persistence |
| `apps/macos-platform/` | Swift/Core Audio/AVFoundation/Accessibility/EventKit/Apple Speech/Foundation Models behind an owned C ABI |
| Windows adapters | Win32 keyboard/clipboard/window services, WASAPI/audio sessions, Media Foundation AAC and endpoint-dependent echo processing |
| Linux adapters | X11 hooks/context, PulseAudio/PipeWire metadata and monitor capture, fixed GStreamer AAC pipelines, Wayland clipboard delivery |

The runtime survives window closing/reopening. The main window handles the library and live-recording routes, and settings, history and dictation each have their own window; onboarding, the recording prompt and pill retain their own presentation. Audio passes directly from native capture into Rust as 16 kHz mono Float32, with one recording timeline and interruption gaps. JavaScript receives state, levels, progress and targeted owned-session results. The Swift library has no SwiftUI or embedded Rust core; [its bridge documentation](../apps/macos-platform/README.md) describes ownership and main-queue requirements.

Recording uses `idle → starting → recording → stopping → finalizing → idle/error`. The runtime rejects concurrent starts and meeting-folder changes while active, and session identifiers protect newer meetings from older completions. Automatic call detection samples activity every two seconds, requires a sustained five-second candidate, and ends after thirty seconds without microphone activity. It stops only recordings associated with that detected call. The recording prompt expires after sixty seconds.

## Platform behavior

| Capability | macOS | Windows | Linux X11 | Linux Wayland |
| --- | --- | --- | --- | --- |
| Dictation trigger | Fn/right modifiers, native event tap | Right Ctrl/Alt or Ctrl+Super hook | Same gestures through XRecord | Configured compositor shortcut invokes manual toggle |
| Record shortcut | ⌘⇧R native tap | Ctrl+Shift+R registration | Ctrl+Shift+R registration | Configured shortcut invokes meeting toggle |
| Selection/insertion | Accessibility, guarded copy fallback, ⌘V | Guarded copy, foreground PID checks, SendInput | Guarded copy/paste, X11 foreground context | Copy result and paste manually; selection reading unavailable |
| Call observations | Core Audio process activity + browser titles | Audio sessions/process trees + browser titles | PulseAudio/PipeWire metadata + X11 browser titles | Native call-app metadata; browser classification unavailable |
| Meeting audio | AVAudioEngine + Core Audio process tap | Microphone + WASAPI loopback | Microphone + PulseAudio/PipeWire monitor | Same audio-server capture, subject to device access |
| Echo processing | Apple voice processing and existing fallbacks | Available endpoint effects; runtime probe | Unavailable; use headphones | Unavailable; use headphones |
| Calendar / Apple Speech | EventKit / macOS 26+ speech | Visible but unavailable | Visible but unavailable | Visible but unavailable |
| Apple Intelligence text model | On-device Foundation Models on eligible macOS 26+ Macs; live availability check | Visible but unavailable | Visible but unavailable | Visible but unavailable |
| M4A archive/playback/diarization | AVFoundation | Media Foundation decode/encode, native output | GStreamer AAC plus native output | Same codec support, subject to installed helpers |
| Local speech models (Whisper, Parakeet, …) | Metal | CPU, x86-64-v3 (AVX2); older processors see the models disabled with a reason | Same as Windows | Same as Windows |
| Pennel Cloud (login, meeting sync, hosted models) | Shared app: Account tab | Shared app: Account tab | Shared app: Account tab | Shared app: Account tab |

This table describes implemented paths, not completed hardware certification. Call classification is heuristic. Safari/WebKit attribution, app helper variants, device routes and real meetings remain acceptance cases.

### Built-in Apple models

In **Providers → Provider toevoegen**, add **Apple-dicteren** for dictation/meeting transcription and **Apple Intelligence** for cleanup, commands, summaries and meeting questions. Neither account has an API key or server URL. The provider actions assign the relevant model slots; the **Modellen** tab shows macOS-managed models without an editable model name. Speech permission is requested explicitly from the Apple-dicteren card. macOS manages speech-language assets and may install them on first use.

Apple Intelligence uses `FoundationModels.SystemLanguageModel.default` on the device, with a fresh session and no tools. Device eligibility, Apple Intelligence being disabled and models still preparing are reported in Dutch; **Controleer beschikbaarheid** rechecks that status. Pennel does not switch to a cloud provider when the model is unavailable. Windows and Linux keep the Apple choices visible but disabled with their macOS requirement. Foundation Models is weak-linked so the app retains its macOS 14.4 minimum; the model itself requires macOS 26 or newer.

The shared core applies a smaller prompt budget for Apple Intelligence. Long transcripts use repeated map/reduce, while meeting questions select relevant sources and preserve the numbering of the sources actually supplied. Oversized notes, templates or dictation receive an explicit length error instead of losing text. Native generation checks the exact token count on macOS 26.4 and newer, reserves response space and applies the request timeout. Existing cleanup recovery retains the raw dictation when processing fails.

### Model setup and verification

Models and recording surfaces distinguish the destination for audio from optional text processing. **Ingesteld** means the selected configuration is present; it does not claim that a provider request succeeded. **Test spraakherkenning** and **Test tekstverwerking** are explicit actions using bundled synthetic audio or fixed sample text, never a user's dictation, meeting, dictionary or pad. The destination is shown before the request. A result is marked **Test geslaagd** only for the configuration tested, and is invalidated after model, endpoint, language or credential changes. Catalog connection testing remains separate and failed catalog requests can be retried.

Model downloads have one owner per model. Onboarding and Settings can attach to the same worker and replay its latest progress. Attaching is atomic and never starts a new download when the old snapshot is stale; only an explicit download action can do that. Cancelling keeps partial bytes, and changing the selected speech engine does not implicitly cancel another window's download.

Meeting answers can open the exact transcript, summary or personal-note passage and return to the original conversation position. Changed or ambiguous passages are reported instead of selecting unrelated text. Dialogs, detail tabs and context menus retain names, keyboard operation and focus restoration. These shared-renderer behaviors do not establish screen-reader or platform-device acceptance; use the [pilot checklist](improvement-pilot.md).

### Pennel Cloud

The shared app has an **Account** tab (last in Settings) with login, usage, meeting sync, the recovery code, devices and account deletion, plus a **Pennel Cloud** provider card. The native Mac reference app shows only a disabled **Account** tab and a disabled Pennel Cloud provider card, both reading "Beschikbaar in de nieuwe Pennel-app"; this is a recorded parity gap, not a hidden feature.

The shared Account tab has controls beyond the original plan: **Nu synchroniseren**, **Herstelcode maken**, **Verwijderingen bevestigen** (the mass-delete brake: a sync that would remove many files waits for confirmation) and a notice while a shown recovery code has not been confirmed.

**Claude-toegang** (Account tab, below the recovery code): a switch, off by default on every computer, that lets Claude search and read this computer's meetings through Pennel Cloud's MCP server. While signed in and switched on, `ClaudeAccessRunner` (`src-tauri/src/claude_access.rs`) keeps a long poll open and answers each relayed tool call with the core's read-only `claude_access_answer`; signing out or deleting the account switches it off. The section shows the connection status and the connector URL and `claude mcp add` command to copy. Same on every platform; the legacy Mac app has no equivalent (recorded gap, like the rest of the Account tab). Protocol and privacy rules: [claude-access.md](claude-access.md).

What is synced: text files in the meeting folders (Markdown notes, transcripts) and `.templates`. Audio is not synced. Content is end-to-end encrypted ("end-to-end versleuteld") on the device before upload, with a recovery code to restore the vault on another device. The server can see the number of objects, their sizes, times and which device uploads.

The recovery code is shown, never written to disk or logs. Copying it excludes it from clipboard history on Windows and on X11 (ownership is kept so pasting keeps working). On macOS it is marked transient/concealed so clipboard managers that honour these types skip it; macOS's own clipboard history behaviour is unverified. On Wayland it uses `wl-copy` with the history-exclusion option; older wl-clipboard versions fall back to plain `wl-copy`, which clipboard managers may record.

### Hotkeys and clipboard

Hold to talk, double-tap for hands-free, press again to submit, and use Shift for commands. Modifier use interrupts hold-to-talk. A command's foreground PID is checked before selection and before delivery; the paste queue preserves recording order.

Mac snapshot/restore preserves pasteboard representations. Windows preserves native clipboard formats, excludes dictated text from clipboard history/cloud clipboard, and detects concurrent user copies. Elevated Windows applications can block synthetic input through UIPI. Linux X11 restores text or one image; rich export does not imply preservation of every prior clipboard format. Paste delivery errors are reported rather than silently treated as success.

The pill and recording prompt use platform no-activation behavior; the pill is click-through. Mac uses an NSPanel without replacing the Tauri window's class. Full-screen, Spaces, mixed-DPI/multi-monitor placement and real foreground focus require packaged desktop tests.

### Linux X11 and Wayland

X11 uses XRecord for dictation gestures, XTest for synthetic keys and `xprop` for active-window/application context. It requires an accessible X11 session. Wayland deliberately disables those paths even when XWayland is also present.

Configure desktop shortcuts for these commands:

```sh
pennel --toggle-dictation
pennel --toggle-meeting
```

For dictation, press the shortcut once to start and again to finish. Pennel uses `wl-copy` to place the result on the Wayland clipboard; paste with Ctrl+V. Selection reading, synthetic paste and foreground-app styles are unavailable there. The meeting command starts/stops a manual recording. When using AppImage, use the AppImage's full executable path in the desktop shortcut.

Native call applications can still be observed through PulseAudio/PipeWire on Wayland. Browser classification requires X11 window titles, so use manual browser-meeting recording on Wayland. Missing `pactl`, an unreachable audio server or missing codecs produce a reason in capability state. Codec availability is cached for the launch; restart after installing missing packages.

Linux M4A services use fixed GStreamer pipelines and bounded subprocesses; audio is passed through file descriptors rather than command-line text. [Linux packaging](linux-packaging.md) lists build/runtime dependencies and explains external AppImage helpers. The supported configuration uses Base/Good/libav and does not require the Bad plug-in set.

### Audio, errors and privacy

Meetings retain `ik.m4a` and `anderen.m4a` and local Markdown. System-capture failure can leave microphone-only recording with a visible reason. Mac preserves voice-processing/device-change recovery and silent-capture fallbacks. A microphone/device failure, archive overflow/write failure or missing codec must not be interpreted as a successful complete recording.

Dictation during a meeting suppresses the microphone's transcript input and marks the interval. This is **not raw-audio redaction**: archives can still contain the dictated audio. Diarization decodes are bounded to the existing three-hour limit.

### Speaker recognition

**Sprekers herkennen** runs after a meeting (opt-in) or from the meeting's transcript. It uses the two speaker models (34 MB, one download in Settings → Meetings or the onboarding) or, with **Via Pennel Cloud**, the signed-in account, which processes audio in memory and does not store it. The desktop decodes `anderen.m4a` first and `ik.m4a` only when the core will split the microphone track (no usable system track and a trained voice), so an hour-long call never holds both tracks in memory. An older Sortformer download still splits voices, but cannot recognise them across meetings.

**Leer je stem** records the microphone for at most 60 seconds through the same capture path as dictation (Swift capture on Mac, CPAL elsewhere). Samples stay in memory and only the voiceprint is stored; it cannot start while a meeting records or a dictation runs. Every platform offers the same controls. A build without local recognition shows **Op dit toestel** disabled with its reason and offers Pennel Cloud.

Keys remain in Keychain, Windows Credential Manager or Secret Service. No API key is returned in an application snapshot. Logs contain timings, lengths and errors rather than dictated text, meeting content or browser titles. Chosen remote providers receive the audio/text needed for the selected task; local mode does not add a cloud account or telemetry.

## Data, preview and upgrades

| Build | Data directory | Credentials |
| --- | --- | --- |
| Canonical Mac | `~/Library/Application Support/Pennel` | `dev.pennel.Pennel` Keychain service |
| Canonical Windows | `%LOCALAPPDATA%\Pennel` | Existing `app.pennel.desktop` service |
| Canonical Linux | `$XDG_DATA_HOME/Pennel`, normally `~/.local/share/Pennel` | Existing `app.pennel.desktop` Secret Service entries |
| Preview/debug | Corresponding `Pennel Preview` directory | Separate preview service |

Canonical meetings use `~/Pennel` or the configured library. Previews pin their library to `<preview data>/Meetings`, reject external paths/symlink escapes, and do not migrate native credentials. `tauri.preview.conf.json` changes the bundle identity to `dev.pennel.Pennel.preview`; debug builds are also isolated.

For permission-bearing Mac previews, both bundlers reuse an existing certificate through `scripts/preview-signing-identity.py`. Explicit `APPLE_SIGNING_IDENTITY` or `PENNEL_SIGN_IDENTITY` takes precedence; otherwise the selector retains a still-valid certificate SHA in ignored `build/preview-signing-identity.sha1`, then tries the known local certificates or an existing Apple Development identity. With no suitable certificate it reports the ad-hoc fallback. It never creates/imports a certificate or changes certificate trust. Canonical native signing selection is unchanged.

Updating an allowed preview with ad-hoc signing changes its designated requirement and can invalidate microphone, speech and Accessibility approvals. macOS can show an old Accessibility switch as enabled while rejecting the new signature; the obsolete entry then needs to be replaced before granting the current app access. Preserve the preview's signing identity on subsequent builds. Debug/release bundles that share the preview identifier must not both remain registered with Launch Services when selecting the app in privacy settings.

Mac shortcut readiness requires a healthy event tap, not just a non-null tap handle or saved permission. Both Mac monitors verify their own tap identity, full requested event mask, port validity and enabled state, then recover registration after permission changes or listener failure. A partially authorized tap is discarded. Native local previews accept `PENNEL_SIGN_IDENTITY` to reuse a stable existing signing certificate.

Core startup obtains an exclusive data lease, validates the migration journal and takes a consistent settings/SQLite/failed-audio snapshot before normalization or retention. Existing data snapshots live under `migration-backup-v1`; `migration.json` records startup migration. Library leases and active-session guards prevent conflicting writers and folder moves. These files support recovery and are not a substitute for user backups.

Canonical Mac startup preserves the native bundle identity and Keychain service, rejects a concurrently running older Pennel/Musli app, and handles legacy Musli paths/keys without deleting source data. Failed credential migration is reported for recovery. Permission preservation still depends on actual signing identity and OS behavior; test upgrades using signed packages.

Canonical Mac logs remain `~/Library/Logs/Pennel/pennel.log`; other builds keep `logs/pennel.log` inside their data directory. **Logboek openen** resolves the current build's file.

## Build and test

Use Rust stable, Node 22+, Python 3 and CMake. Mac requires Xcode with the macOS 26 SDK or newer. Windows uses the MSVC toolchain and Windows SDK. Linux needs WebKitGTK/tray, X11/audio development libraries and the runtime helpers listed in [Linux packaging](linux-packaging.md).

```sh
make desktop-preview
make desktop-test
(cd apps/desktop && npm test)
```

`make desktop-preview` installs locked frontend dependencies and builds isolated preview packages. On Mac it builds the app-only Tauri package and creates `build/Pennel Preview.dmg` with `hdiutil`, so preview packaging also works without an interactive Finder session. `make desktop-test` builds the frontend and runs backend tests; run `npm test` as well for frontend regression coverage. `make desktop` builds canonical packages. Installer output is under `apps/desktop/src-tauri/target[/<target>]/release/bundle/` unless a debug build was requested.

For hot reload:

```sh
cd apps/desktop
npm ci
../../scripts/with-toolchain.sh npx tauri dev
```

The toolchain wrapper uses system tools or an existing ignored `build/toolchain/` installation; it installs nothing. Mac local models use Metal. Windows/Linux use CPU by default; an explicitly selected Vulkan build uses `npx tauri build -- --features vulkan` and requires the Vulkan SDK.

Distributable Windows/Linux x64 builds in CI and the release helper compile the local speech engine (transcribe.cpp/ggml) for a fixed **x86-64-v3** baseline: SSE4.2, AVX, AVX2, FMA, F16C and BMI2 on; host tuning (`GGML_NATIVE`), AVX-VNNI, AVX-512 and AMX off. `TRANSCRIBE_X86_CONSERVATIVE=ON` keeps every other ggml SIMD flag off. The flags are appended after existing CMake arguments, only the native dependency's release-target cache is cleared, and the generated CMake caches must show exactly this baseline. Earlier releases only set the conservative option, which turned every SIMD tier off, so local Whisper and Parakeet ran scalar code. A one-off CI benchmark on AVX2 runners, with 10 s of audio and the median of three runs, measured these times. Windows: Whisper small 45.7 s → 4.0 s and Parakeet 0.6B 17.9 s → 1.6 s. Linux: 24.9 s → 4.1 s and 10.5 s → 1.6 s. See [portable candidate configuration](linux-packaging.md#candidate-and-update-artifacts). Direct development builds tune for the build machine and are not certified portable across CPUs.

Every Intel Core processor since Haswell (2013) and every AMD processor since Zen (2017) meets the baseline, which covers the Core and Ryzen processors that Windows 11 supports. Some Pentium Gold, Celeron and Atom processors on that list lack AVX. Before a local speech model or the older Sortformer speaker model loads, the core checks the processor (`core/src/cpu_support.rs`). On a processor without these features, the **Lokaal** provider and local model downloads are shown disabled with "Deze processor is te oud voor lokale modellen. Kies een cloudprovider.", and a load attempt returns that reason instead of crashing on an illegal instruction. Speaker recognition uses the prebuilt ONNX Runtime, which selects its instruction set at run time, so it stays available. Runtime CPU dispatch for ggml (`dynamic-backends` in transcribe-cpp-sys) would also cover those processors, but it needs a shared library plus per-ISA modules in every installer. It is not used yet.

Keep the native comparison build working:

```sh
make core && make test && make app
swift test --package-path apps/macos-platform
make packaging-test
```

## Verification and release state

Automated checks and local package inspection are recorded in [shared-app-migration.md](shared-app-migration.md). Linux's isolated codec round-trip and synthetic PulseAudio observations verify code paths; they are not real calls, installed-package tests or Wayland compositor acceptance. Cross-target Windows type checking does not establish Windows execution. CI configuration does not imply a workflow has run for this change.

The standard CI matrix is configured to build Mac, Windows and Linux development artifacts without requiring release keys. The manual candidate workflow uses existing signing keys/certificates and performs no publication. The Tauri updater is enabled only when a public key is configured; previews without release configuration do not auto-update. Linux AppImages support in-app updates; Debian installations use a verified package or package manager.

Pennel Cloud is built in the shared app and awaits device acceptance (see open-items T11-T13); the Linux clipboard paths for the recovery code have not been exercised on a Linux device.

Before replacing the native Mac app or switching website downloads, pass real Mac/Windows/X11/Wayland desktop, migration, signed-update and performance acceptance. The prepared legacy Sparkle archive/appcast provides the native-to-shared transition; the shared app uses the Tauri updater. Public release availability and distribution claims must follow those results.

The look follows the original Swift app at `91775c1`: Figtree throughout, Instrument Serif titles, a paper surface with a barely darker sidebar, soft unbordered panels and real app icons. Choices use segmented pills, switches and pill menu buttons in the brand colours; there is no system accent blue. `scripts/capture-desktop-windows.sh` captures the preview build's own windows (light and dark) with synthetic data for review; `scripts/capture-desktop-reference.mjs` renders the same views headless.
