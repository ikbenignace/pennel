# Redesign acceptance

This record separates deterministic UI evidence from packaged-app and real-device acceptance.

## Reproduce the UI fixtures

The shared app runs its production development entry with a local, injected platform client. The fixture is provided by the capture harness, not bundled into the production app. All responses, transcript text, permissions and keys are synthetic. Unexpected native calls fail rather than falling through to the real Tauri bridge.

```sh
node --test scripts/fixtures/desktop-platform.test.mjs
node scripts/capture-desktop-reference.mjs
scripts/capture-native-reference.sh
node scripts/fixtures/compare.mjs --require-all
swift scripts/fixtures/compose.swift
```

The desktop capture requires the existing Vite installation and a local Chromium executable. Set `PENNEL_CHROME` or pass `--browser` when its path differs. It creates a temporary browser profile and captures the page through CDP; it never uses the user's browser profile or screen. `--serve` exposes the same fixtures for an existing browser tool. `--case NAME` and `--appearance light|dark` select a smaller matrix.

The native runner compiles the production SwiftUI views in an isolated snapshot module. Its platform replacements are inert, its core data is stored in an owned temporary directory, and a filesystem/network sandbox protects the regular Pennel library, settings, logs and Keychain. Images come from `NSHostingView.cacheDisplay` on owned offscreen windows, not a display capture.

Both sets use the same meeting, notes and transcript, a fixed fixture clock where supported, and matching logical sizes: 1040×740 for the workspace and 850×690 for onboarding. Manifests record the actual dimensions, fixture state and capture method.

## Evidence collected

- Fixture matrix: `scripts/fixtures/visual-cases.json`.
- Desktop evidence: `build/visual-parity/desktop/manifest.json` and appearance subdirectories after capture.
- Native evidence: `build/visual-parity/native/fixture.json` and appearance subdirectories after capture.
- Side-by-side comparisons are generated only after both corresponding renderer captures exist. The HTML gallery is `build/visual-parity/index.html`; pixel compositions are under `build/visual-parity/comparison/` and retain both source captures.

Collected on 2026-10-25:

- `node --test scripts/fixtures/desktop-platform.test.mjs`: 6 tests passed. These check scenario identity, synthetic/secret-free snapshots, owned session events, fail-closed unknown commands, real catalog metadata and manual Pad readiness when a global shortcut permission is denied.
- Native offscreen matrix: 26 cases × light/dark, 52 PNGs; isolated snapshot build and execution completed. The inert meeting-controller replacement includes the production presentation property `willSummarize`; it never records or summarizes.
- Shared development matrix: 26 cases × light/dark, 52 PNGs. All captures completed with no unknown fixture IPC calls, unhandled exceptions, visible application error banners, resource errors or horizontal document overflow.
- `node scripts/fixtures/compare.mjs --require-all`: 52/52 pairs, no missing cases, no logical dimension mismatch and no capture failures.
- `swift scripts/fixtures/compose.swift`: 52 pixel compositions from the two owned capture sets.
- The matrix includes both successful and empty practice sessions, recoverable practice/download errors, denied permissions and an existing/skipped profile with microphone/model readiness but denied global-shortcut access. The successful-practice fixture exposed a repeated-mount/save loop during development; the frontend owner repaired it before this full capture.

The review first covered all 26 scenario states sampled across light and dark appearances. It found material differences in the custom-provider form, Welcome corner label, denied-shortcut warning, permission/practice actions, download-error recovery and empty meetings state. The UI owners repaired those differences. A separate 10-case × light/dark repair matrix was captured and personally inspected before the final full matrix was regenerated from the frozen sources.

One harness inconsistency was also corrected: wizard states with denied Accessibility access now report a matching blocked global-shortcut capability, rather than reporting it available while readiness disallows it. The same fixture test checks this fact and the manual Pad remains usable.

**Covered-surface visual review: pass, with rendering differences.** The repaired onboarding and empty state, ordinary and permission-recovery Pad, meeting transcript, history and settings were inspected again in the fresh final matrix. No material layout, control or recovery-action blocker remains in these covered states. Font rasterization, native popup/checkbox presentation, some row density and minor help wording still differ; this is not a pixel-equality claim.

The 52/52 comparison result checks completeness, logical sizes and shared runtime/layout diagnostics. The visual judgment comes from the paired-image review, not those numeric checks alone. It covers the fixture viewports and states listed in the registry; it does not certify every dialog, responsive size, hardware behavior or packaged webview. Product unit/integration tests, packaged startup and real-device checks remain separate from this renderer harness.

## Local product and package checks

The final frozen source was checked on the local Apple Silicon macOS host on 2026-10-25:

| Check | Result | Evidence |
| --- | --- | --- |
| `make core && make test && make app && make app-preview` | Passed; 303 Rust tests, 5 existing hardware/network tests ignored; 60 native Swift tests | `build/redesign-native-final.log` |
| Locked frontend install, tests and production build | Passed; 55 frontend tests; locally bundled fonts and generated icons | `build/redesign-frontend-checks.log`, `build/redesign-frontend-final.log` |
| Shared backend tests and all-target Clippy | Passed; 86 tests, no Clippy warnings | `build/redesign-rust-final.log` |
| Core formatting and Clippy | Passed | `build/redesign-rust-final.log` |
| macOS platform tests | Passed; 37 tests | `build/redesign-platform-final.log` |
| Theme/icon generator | Reproducibility check and 7 regression tests passed; 20 shared icon shapes | `scripts/generate-theme.py --check` and `--self-test` |
| Packaging configuration regressions | 19 tests passed | `scripts/tests/test_desktop_packaging.py` |
| Shared Mac preview app | Built successfully; bundle identity `dev.pennel.Pennel.preview`, minimum macOS 14.4 | `build/redesign-shared-preview-final.log` |
| Native Mac preview app | Built successfully; separate `dev.pennel.Pennel.native.preview` identity, update feed disabled | `build/redesign-native-final.log` |
| Preview disk image | Created and checksum-verified, mounted read-only, contained the signed preview app and Applications link; bundle signature/identity/minimum checked again inside the image | `build/redesign-preview-image.log`, `build/Pennel Preview.dmg` |
| Native packaged startup and appearance | Real isolated preview opened its headerless wizard, switched between Light/Dark, reached the speech-engine screen and returned to Welcome. AX inspection verified distinct Lokaal/Apple-dicteren/Eigen provider buttons and the selected language label. No model download, microphone capture or permission grant was performed. | Native computer-use startup inspection; `build/redesign-native-startup.log` |

The initial shared DMG cosmetic-layout step timed out in Finder automation. The reproducible Mac preview target now builds the app-only bundle and uses `hdiutil` directly for the disk image, avoiding Finder automation. The shared preview bundle is signed with an existing configured/local identity when available, otherwise explicitly ad-hoc signed with its entitlements; both preview app signatures passed `codesign --verify --deep --strict`.

The real native startup inspection additionally found a SwiftUI group label propagating over its three engine buttons. The group and popup labels were corrected without visual/layout changes, all 60 native tests passed again, and both native bundles were rebuilt. A new process of the rebuilt preview exposed the correct distinct button names and language value. The final paired screenshots remain representative of the unchanged rendering.

Deliverables are `build/Pennel Preview.dmg`, the shared preview app under `apps/desktop/src-tauri/target/release/bundle/macos/`, and `build/Pennel Native Preview.app`. The previous native comparison bundle was preserved under `build/native-reference-before-redesign/Pennel.app`. No installed application was replaced, and the product preview apps were not launched against canonical settings or credentials.

These are local build/package checks, not proof of real microphone capture, OS permission recovery, external insertion or release certification.

## Fn listener recovery follow-up

The user reported that the physical fn key did nothing in the native preview. A live diagnostic stream received no shortcut edges during the attempted hold. Read-only event-tap inspection found that the old process had accepted a partial listener (`0x1000`, flags changes only) although it requested `0x1400` (flags changes and key presses). Its startup log reported an active listener while Accessibility was denied. The initial retry loop stopped at that partial success and did not recreate the listener after permission changed.

Both Mac hotkey monitors now require Accessibility, a valid/enabled port and the complete event mask of their own tap ID. Partial taps are disposed, and the monitor checks and restores its registration every two seconds after startup. Native readiness observes actual listener availability. Manual recording remains independent of global-shortcut availability.

Follow-up verification on 2026-10-25:

- Native/core tests and both native bundles passed again: 303 Rust tests (5 pre-existing ignored) and 60 Swift tests, `build/fn-recovery-native.log`.
- macOS platform suite passed 45 tests, including eight new tap-health/recovery regressions: `build/fn-platform-tests.log`.
- Shared backend 86 tests and all-target Clippy passed; core formatting/Clippy passed using the repository toolchain wrapper: `build/fn-recovery-desktop.log`.
- Locked frontend install, 55 tests and production build passed: `build/fn-recovery-frontend.log`.
- Shared preview app and verified disk image rebuilt: `build/fn-recovery-preview.log`.
- Six native light/dark snapshots covering the ready Pad, denied-shortcut Pad and permissions step were regenerated; each was pixel-identical to its previously reviewed counterpart: `build/fn-recovery-visual/native/`.

The first rebuilt native preview opened with the same isolated settings. macOS revoked its earlier Accessibility and Apple Speech approvals after the ad-hoc signature changed. The user then enabled Accessibility, but the app still correctly reported denied access. System Settings showed the preview switch enabled while TCC's own log reported `Failed to match existing code requirement`: the stored requirement still contained the previous binary's hash. Toggling that stale entry had not replaced its requirement. Bounded diagnostics are in `build/fn-permission-signature.log`; permission databases were not modified directly.

Both preview bundlers now select and retain a valid existing signing certificate rather than needlessly falling back to ad-hoc signing. The shared selector caches only its public certificate SHA in ignored `build/`; it does not create/import certificates, change trust or alter canonical signing. Eleven new selection/CLI regressions passed as part of all 30 packaging tests (`build/fn-recovery-packaging-tests.log`). Two different owned Mach-O fixtures signed with the selected certificate produced different code hashes but identical designated requirements (`build/fn-signing-check/result.txt`). This verifies identity stability, not OS permission approval.

Both previews were signed with that existing Apple Development certificate and passed strict signature verification; the shared disk image was rebuilt and checksum-verified (`build/fn-recovery-signed-native.log`, `build/fn-recovery-signed-desktop.log`). The obsolete Accessibility entry was reset only for `dev.pennel.Pennel.native.preview`, and the final native preview was reopened at the permissions step.

The user granted all three permissions to the stable identity. Without restarting that process, native readiness updated and its log reported `sneltoets beschikbaar=true toegankelijkheid=true`. A separate read-only inspection confirmed that the current process owned one enabled listen-only tap with the full `0x1400` mask (`build/fn-recovery-live-tap.log`). This establishes live permission and listener recovery on this Mac. Physical fn gestures and successful speech transcription remain pending the practice test; unit tests and listener registration alone are not treated as evidence that a dictation completed.

## Global dictation and live-caption follow-up

The user confirmed that Apple-keyboard fn works and the MX fn does not. After selecting **Rechter Option** through the native preview's real settings, the MX shortcut worked. This matches Logitech's documented Fn/Globe limitation. No system keyboard mapping or installed application was changed.

The next request clarified the primary workflow: speak in the currently active external field and see recognition text in the small overlay. The main Dicteren route now leads with that workflow; **Open dicteerpad** is optional. Both apps use the shared core's frozen live session, bounded/cancellable local preview, selected Apple streaming service and session/revision guards. Cloud ASR does not add preview requests. Global delivery freezes the stop target; pad and practice retain their separate ownership rules. Details and source references are in [desktop behavior](desktop.md#dictation-destinations-and-live-text).

Final local checks on 2026-10-25:

| Check | Result | Evidence |
| --- | --- | --- |
| Core, native tests and native bundles | 318 core tests, 5 pre-existing ignored; 76 Swift tests; canonical/reference and isolated preview built | `build/live-final-native.log`, `build/live-final-preview-native.log` |
| Frontend | Locked install, 62 tests and production build passed | `build/live-final-frontend.log`, `build/live-final-preview.log` |
| Shared backend | 99 tests and all-target Clippy passed | `build/live-final-backend.log` |
| macOS platform | 58 tests passed, including Apple preview and stop-target guards | `build/live-final-platform.log` |
| Core formatting/lint | Passed | `build/live-final-rust-lint.log` |
| Shared tokens/packaging/fixtures | 8 generator, 30 packaging and 8 fixture tests passed | `build/live-final-theme.log`, `build/live-final-packaging-tests.log`, fixture test suite |
| Complete renderer matrix | 32 cases × light/dark, 64 captures per app and 64 comparison pairs; no missing images, dimension mismatch, runtime/layout errors or incomplete captures | `build/live-visual-full/index.html`, `build/live-visual-full/comparison.json` |
| Scoped visual review | All 16 affected light/dark pairs inspected; optional-pad hierarchy, plain-text caption, latest two-line tail and working-indicator contrast passed after repair | `build/live-visual/index.html` |
| Preview packages | Both Apple Development signatures passed strict verification; shared DMG rebuilt and checksum-verified | `build/live-final-preview-native.log`, `build/live-final-preview.log` |

Independent source review found and repaired cancellation during owned finalization and command START/STOP target disagreement. Delayed-provider tests now prove that cancelling/closing pad or practice prevents later history/results. Style rules use the same frozen stop-target identity as delivery. The UI review repaired a stretched shared recovery button, native multiline tail selection and native working-indicator contrast.

The final native preview was reopened with its existing Apple Speech, Dark appearance and Rechter Option settings. All permissions survived this certificate-stable update. The user then tested an external text field and explicitly confirmed **live text and paste both work**. Runtime metadata records a successful final transcription and delivery; dictated content was not logged. A preceding attempt returned digital silence and was rejected rather than treated as success. This is a successful native Mac trial with recovery, not a blanket microphone-reliability or cross-platform acceptance claim.

The shared packaged runtime, Windows/Linux devices, full clipboard/focus permutations and remote CI still need separate acceptance. The native reference remains available.

## Packaged and device acceptance

The following require separate evidence and are not established by fixture screenshots:

| Target | Required acceptance | Status |
| --- | --- | --- |
| macOS | Native packaged startup, stable-signature permission recovery, Apple fn/MX Right Option and a successful external live-caption/paste trial checked. Shared packaged runtime, clipboard preservation and broader focus/device permutations remain open. | Partial |
| Windows | Preview startup, microphone, Right Ctrl, foreground/UIPI behavior, clipboard preservation and target delivery | Pending |
| Linux X11 | Preview startup, audio/helpers, shortcut, foreground activation and clipboard preservation | Pending |
| Linux Wayland | Manual/button dictation, configured desktop shortcut, explicit external-paste limitation and manual copy | Pending |
| Upgrade | Existing modern/legacy profiles retain provider/model/key relationships, appearance defaults and meeting data | Pending |
| CI | Core, native and shared app jobs for all configured OS targets | Pending |

The native reference app remains available until the shared packages and real-device acceptance pass. No release publication is implied by local build or screenshot results.

The initial local previews used ad-hoc signing. The fn follow-up above rebuilt both with an existing Apple Development certificate and retained its selection for later preview builds. Production signing/notarization and full permission persistence across upgrades are not established by these local previews.

## Mac menu bar follow-up

The shared Tauri app now launches in the menu bar after completed or skipped setup. Setup still opens on first use, and finishing or skipping it opens Dicteren once. Explicit reopening resumes unfinished setup or opens the workspace. Mac window closing uses an acknowledged save barrier; window-owned sessions are restored on reopening, and quit/update barriers take priority over closing.

Local checks on 2026-10-25 passed: 110 shared-backend tests, 71 frontend tests and the production build, 318 core tests (5 existing ignored), 76 native Swift tests, 58 macOS platform tests, core/shared Clippy and formatting, and 30 packaging tests. The native reference app built successfully. Menu presentation tests cover recording transitions and shared-token icon colors; icon generation is reproducible.

The final shared preview and disk image were rebuilt with the existing stable development identity. Strict bundle signature/identity/permission metadata inspection and disk-image checksum verification passed. Evidence is under `build/menu-bar-*.log` and `build/menu-bar-final-bundle.json`.

A fresh isolated preview profile showed onboarding, and **Later instellen** opened Dicteren. A subsequent cold launch of the final package had accessory activation policy and no visible windows (`build/menu-bar-final-quiet-start.json`). Explicit reopening showed the workspace. Closing the dicteerpad left the same process running without visible windows, and reopening restored a synthetic draft. History/settings navigation and explicit quit were checked through the real packaged interface. A separate process/window metadata check avoided the app-inspection tool's automatic reopening behavior. The previous shared preview profile was restored after testing; canonical data, credentials and the running native preview were preserved.

Four affected fixture surfaces (Dicteren, dicteerpad, recording and onboarding Welcome) were captured in light/dark appearances in both renderers: eight paired compositions, no dimension mismatches or runtime/layout errors. The comparison gallery is `build/menu-bar-visual/index.html`; it is a scoped comparison, not a new full-matrix acceptance claim.

The [desktop CI matrix](https://github.com/ikbenignace/pennel/actions/runs/40208667421) passed all six jobs on revision `8fdd176`: Mac, Windows and Linux app tests/installers, Windows/Linux core checks, and packaging-tool tests. CI exposed existing blockers that were repaired without changing Mac migration behavior: non-Mac dictation-message sizing, explicit development signing, Windows-safe migration refusal assertions, and initial Windows AAC format negotiation. Four portable format regressions passed, and the existing native Mac AAC fixture and Windows roundtrip assertions passed unchanged. The Windows follow-up changes no Mac runtime code.

Live microphone dictation, recording during window closure, interactive status-menu/VoiceOver acceptance and full platform device acceptance remain open. The isolated test profile had no downloaded speech model; no model download or new OS permission grant was performed.

## Main integration follow-up

On 2026-10-25, the menu-bar branch (`bce4309`) was integrated with updated main (`5561f3d`). The merge preserves main's separate library/settings/history/dictation windows, native window chrome, and Cloud/Account behavior. Quiet Mac startup, the recording-aware menu, per-window save/hide barriers and visibility-aware polling remain active. A new backend regression verifies that hiding Settings does not cancel Library or History presentation. CI retains main's signing configuration before packaging instead of signing the application twice.

Local integration checks passed:

- Core/native build and tests: 435 core tests passed, five existing hardware/network tests ignored, 76 native Swift tests passed, and the native reference bundle built (`build/merge-main-native.log`).
- macOS platform suite: 60 tests passed (`build/merge-main-platform.log`).
- Frontend: locked install, 98 tests, TypeScript check and production Vite build passed.
- Shared backend: 131 tests, formatting and all-target Clippy passed (`build/merge-main-desktop-rust.log`).
- Core formatting/Clippy, shared theme reproducibility, 30 packaging tests and nine fixture tests passed (`build/merge-main-quality.log`).
- Shared renderer: 36 cases in light/dark, 72 captures, no runtime/layout failures (`build/merge-main-visual/manifest.json`). The dictation pad, Cloud account and meeting conflict surfaces were visually inspected against the merged main design.
- Shared preview application and checksum-verified disk image built; strict bundle inspection passed with the isolated preview identity and a valid development signature (`build/merge-main-preview.log`, `build/merge-main-bundle.json`). The application was not launched.

These checks cover integration and deterministic rendering. They do not establish new real-device, microphone, Cloud service, installer/update or Windows/Linux acceptance.
