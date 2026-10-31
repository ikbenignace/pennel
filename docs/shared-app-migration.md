# Shared Pennel application

Pennel's shared desktop implementation targets Apple Silicon/macOS 14.4+, Windows 11 x64 and Linux x86-64. One TypeScript/Tauri interface and Rust product runtime use native services where required. The SwiftUI application shell was retired by the user-approved [cleanup #75](https://github.com/pennel-app/pennel/issues/75). The Apple platform service library, existing-installation migration and Sparkle transition remain part of the shared app; packaged-device and signed-upgrade acceptance remain open.

This document distinguishes implementation, scoped automated checks and release acceptance. Dated native-reference results below are historical; their removed build targets are not current instructions. The [pre-retirement Mac shell](https://github.com/pennel-app/pennel/tree/e6f02b2cd178b541dcbfcdb46125336c762dbf07/apps/macos) remains available in Git. Code existence, a configured CI job or a browser screenshot is not production evidence.

## Apple models and preview verification — 2026-10-25

The shared preview now includes Apple-dicteren for dictation/meeting transcription and Apple Intelligence for cleanup, commands, summaries and cited questions. Apple Intelligence uses the on-device Foundation Models adapter, with availability reasons, bounded prompts, repeated summary reduction, exact native token checks where supported and no cloud fallback. Windows and Linux retain visible unavailable Apple controls.

- The preview was signed with an existing Apple Development identity. Signature verification passed; this is still a development build, not a notarized Developer ID release. Microphone, speech and Accessibility approvals survived a subsequent rebuild using the same identity.
- The real packaged app's three model slots were set to Apple-dicteren / Apple Intelligence / Apple-dicteren and displayed **In orde**. Its local **Test model** action passed. A separate on-device synthetic Dutch generation test also passed without logging generated content. Speech-authorization status now refreshes after approval without requiring another click.
- Core checks passed: **289 tests**, with **five live tests ignored**, plus format and strict clippy. Mac host checks passed **62 tests**; Linux x64 container checks passed **76 desktop tests** and strict clippy. Swift platform checks passed **31 tests**; the native reference passed **45 tests** and a release build. Frontend checks passed **26 unit tests**, typecheck/build and **five browser-fixture scenarios**; release tooling passed **19 tests**.
- New app-surface comparison boards use owned offscreen native views and a Chromium synthetic-IPC harness. They establish presentation evidence, not native Windows/Linux device behavior. Live microphone dictation, real meetings, native Windows execution and signed production updates remain acceptance work.

The local addendum is recorded in ignored `build/apple-models-implementation-receipt.json`, `build/apple-models-shared/verification.json`, `build/apple-linux-check.log`, `build/apple-preview-final-build.log` and `build/apple-models-comparison/`. The earlier migration checks below remain dated baseline evidence.

## Migration verification — 2026-10-24

The shared implementation is available as an isolated migration preview. The native reference remains available; no release was published, and no signing keys, production data or existing credentials were replaced during verification.

- Optimized Mac preview built and passed signature/bundle inspection with its preview identifier. The signature is **ad-hoc**, not Developer ID; notarization was not performed.
- The actual packaged Mac application passed onboarding → library → seven-tab Settings → keyboard dictionary editing → persisted preview setting → normal Command-Q exit. This used the real Rust/Swift backend, not mocked IPC. A navigation-broadcast feedback loop found by this smoke test was fixed with scoped events and a frontend regression guard; the built app was retested. Only the test's synthetic dictionary entry was removed afterwards.
- Full Linux x64 checks passed in isolated Debian 12: **272 core tests** (five live tests ignored), **74 desktop tests**, **26 frontend tests**, format checks and strict clippy. Both unsigned `.deb` and AppImage packages were built, extracted and inspected; their final frontend asset and portable native CPU cache were verified. This does not establish installation or physical X11/Wayland acceptance.
- Mac checks passed: **60 desktop tests**, **19 Swift platform tests**, **45 native reference Swift tests**, strict Rust lint/format checks, and native reference bindings/bundling. **19 release-tooling tests** and **179 website tests** also passed.
- New provider HTTP errors retain safe status-based messages; provider error bodies cannot enter new stored HTTP errors. Diagnostics use categories/status codes. Historical error rows/backups were intentionally not rewritten.

Exact artifacts and scope are recorded in ignored `build/shared-app-implementation-receipt.json`, `build/mac-native-smoke.json`, `build/linux-artifacts/receipt.json`, `build/linux-final-logs/source-manifest.json` and `build/shared-source-manifest.json`. The seven inspected comparison boards are in `build/visual-parity/index.html`; those boards explicitly use native offscreen views versus Chromium with mocked IPC and are presentation evidence only.

Windows native module APIs were cross-checked against the Windows target, but a full Windows MSVC app/installer run is still required. Real call/device/permission/clipboard tests, sixty-minute recording and comparable performance measurements, production signing/notarization and the real signed-update transition remain release gates. CI is configured for all three platforms; no remote workflow was represented as having run for this working tree.

## Implemented migration

| Area | Implementation | Remaining acceptance |
| --- | --- | --- |
| Product runtime | Shared gestures, paste ordering, call debounce/policy, explicit recording lifecycle, timestamped ingestion and operation/session guards | Concurrent dictation/meeting workflows, long calls and interruption recovery on real devices |
| Mac services | Embedded Swift/C/Rust bridge for Fn/modifiers, clipboard/selection, Core Audio, voice processing, EventKit, Apple Speech, M4A and native panels | Permissions, browser/helper attribution, speaker/headphone routes, AirPods, focus/Spaces and signed upgrades |
| Windows services | Existing Win32 hook/clipboard plus audio-session observation, Media Foundation archives and endpoint-dependent echo processing | Native Windows execution, app-specific paste, drivers, real meetings and signed installers |
| Linux services | X11 context/gestures, compositor-invoked Wayland toggles, clipboard delivery, PulseAudio/PipeWire observations and GStreamer M4A | Installed Debian/AppImage behavior on actual X11 and Wayland desktops, audio servers/devices and compositor focus |
| Shared interface | Library/search/chat, live/detail views, notes/transcript/questions, speakers, templates, export/history, setup and seven settings tabs | Visual/accessibility comparison and full interface actions through packaged native hosts |
| Data preservation | Exclusive data/library leases, startup snapshots before normalization/pruning, legacy Mac path/key handling and isolated previews | Signed upgrade/recovery with actual existing installations and credential permissions |
| Distribution | Three-platform CI, public-key-configured updater, signed candidate preparation, legacy Sparkle transition and exact-artifact acceptance manifest | Required signing credentials, install/update tests, measured packages and separate publication |

See [desktop.md](desktop.md) for actual platform behavior and [linux-packaging.md](linux-packaging.md) for Linux dependencies. The Mac platform library has [separate lifetime/threading documentation](../apps/macos-platform/README.md).

## Reproduction commands and evidence locations

Run from the app repository root. Generated evidence stays under ignored `build/`; retain the exact revision and command results with each integration/release receipt.

| Check | Reproduce or inspect | Evidence scope / current location |
| --- | --- | --- |
| Core Rust | `scripts/with-toolchain.sh cargo test --manifest-path core/Cargo.toml --lib`; fmt and clippy as required by project guidance | Core tests cover lifecycle/detection/storage/timestamp cases; Linux x64 logs under `build/linux-full-logs/core-tests.log` and `core-clippy.log` |
| Tauri Rust | `scripts/with-toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`; corresponding fmt/clippy checks | Mac host logs: `build/desktop-tests-final.log`, `build/desktop-clippy-final.log`; these do not establish Windows/Linux desktop behavior |
| Swift platform | `swift test --package-path apps/macos-platform` | Synthetic archives, timeline gaps, clipboard/lifetime/window helpers and status inspection; no microphone/system recording during tests |
| Historical native reference | Inspect the [pre-retirement source](https://github.com/pennel-app/pennel/tree/e6f02b2cd178b541dcbfcdb46125336c762dbf07/apps/macos) and retained receipt | `build/native-reference-final-check.log` records the original build; the shell and its build commands are retired |
| Frontend | `cd apps/desktop && npm ci && npm test && npm run build` | Async/save/draft/navigation regression tests and production frontend build; browser harness uses IPC fixtures |
| Mac preview package | Build with `tauri.preview.conf.json`; inspect with `scripts/verify-desktop-bundle.py` | `build/mac-preview-inspection-final.json`: valid arm64 ad-hoc preview signature; separate actual UI run in `build/mac-native-smoke.json`. No Developer ID/notarization or call/device acceptance |
| Visual comparison | Inspect application-window captures and shared-view harness | `build/visual-parity/`; platform-labelled web fixtures do not imply native Windows/Linux execution |
| Linux codec / observations | Reproduction commands and scope in `build/linux-native-evidence.json` | Seven actual Rust codec tests in isolated Debian 12/aarch64; Mac AAC fixture decode, gap-filled M4A round trip, and a synthetic PulseAudio server/streams |
| Windows native modules | Inspect `build/windows-check/logs/windows-module-clippy.log` | Actual Windows module code cross-checked against MSVC target with limited core/logging stubs; not a complete Windows app build or execution |
| Linux x64 full integration | Inspect `build/linux-final-logs/`, its `source-manifest.json` and `build/linux-artifacts/receipt.json` | Final native x64 checks and unsigned Deb/AppImage inspection passed. Installed-package, GUI, permission and call/device acceptance remain separate |
| Release tooling | `make packaging-test` | Metadata, signing configuration, exact artifact hashes and separate Linux X11/Wayland acceptance gates; no signing keys generated or publication performed |

The Linux codec receipt records the exact captured test result and synthetic probe output as a summary with reproduction sources; it is not an invented full stdout log. The synthetic PulseAudio probe uses no physical microphone or user audio.

At the original migration baseline, the website checkout separately prepared a typed Mac/Windows/Linux artifact map and qualified-download helper. Its `latestRelease` was null and Windows/Linux availability copy stayed unchanged. The website's release handoff documentation describes the separate publication step.

## Acceptance before native replacement or publication

Shell retirement is a source-ownership decision, not completion of the acceptance gates below. Run them against the actual candidate packages, preserving the previously installed application and data for recovery:

- Mac and Windows: Teams/Zoom/browser Meet, both audio tracks, permission denial/recovery, speakers/headphones and device switches; repeat dictation immediately after a meeting.
- X11 Linux: global gestures, selection and guarded paste, foreground styles, browser/native call observations, monitor audio, installed codec/helper dependencies and overlays.
- Wayland Linux: compositor shortcut toggles, copied dictation plus manual paste, manual browser recording, native call observations, missing-tool reasons and compositor placement. Acceptance must not claim generic global hooks, selection reading or synthetic paste.
- All targets: notes/draft saves, search/citations/templates/speakers/export, closing/reopening windows during capture, shutdown/update while work is pending, archive errors, failed transcription and recovery.
- Existing installations: settings/providers/models/history/meetings, custom library paths, Mac legacy data/keys, concurrent-process rejection, interrupted migration and rollback using the retained snapshot.
- Packaged interface: app-window-only comparisons, keyboard/screen-reader access, multi-monitor/mixed-DPI placement and no-focus pill/prompt behavior.
- Signed distribution: install and update from prior versions, legacy Sparkle-to-Tauri transition, correct credential/bundle identities, Linux AppImage self-updates and Debian package instructions.
- Performance: equivalent model-unloaded/loaded release configurations, all application processes, startup/hotkey response, idle CPU and a sixty-minute recording. Do not reuse the historical unmeasured memory figure.

The production updater manifest requires accepted Mac, Windows and Linux AppImage artifact hashes and separate `linux_x11` / `linux_wayland` gates. Debian packages retain their own signed installer/checksum handoff. Test instrumentation and synthetic providers stay out of release builds.

## Baseline issues and release boundaries

[open-items.md](open-items.md) preserves the historical native Mac issue list. Shared timestamping, guards and permission explanations have targeted tests; that does not close real-device questions about clock/device behavior, browser attribution, echo processing or third-party paste.

Missing codecs/helpers, unsupported OS features and denied permissions remain visible with reasons. Apple Speech and EventKit are Mac-specific; Linux has no echo processing and native Wayland input restrictions remain explicit. Existing media/diarization limits and three-hour decode bound remain documented.

The initial desktop migration added no accounts, billing, telemetry, cloud synchronization or mobile app. Later mobile and distribution work has separate evidence in the [mobile guide](mobile.md) and [release guide](releasing.md); historical native/Sparkle receipts do not establish current release acceptance.
