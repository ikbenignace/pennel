<img src="assets/brand/wordmark.svg" height="72" alt="pennel.">

**Say it. Full stop.** Pennel turns dictation into polished text and meetings into transcripts and notes kept on your computer.

The desktop implementation now uses one **Tauri 2 interface and Rust core for macOS, Windows and Linux**. An embedded Swift library retains the Mac's Apple integrations. This checkout contains the shared-app migration; the native SwiftUI app remains a comparison and recovery build until packaged applications pass device acceptance. See [implementation and evidence](docs/shared-app-migration.md).

- **Dictation and commands.** Hold `fn` on Mac or Right Ctrl on Windows/X11 Linux, speak, and release to insert cleaned-up text. Double-tap for hands-free dictation. Shift changes the instruction into a command on the selection where the platform supports selection reading.
- **Meetings.** Microphone and system audio feed separate tracks and local M4A archives. The shared interface includes meeting detection/rules, notes, transcript, templates, speaker recognition, search, cited questions and export. Platform services and permissions determine availability.
- **Private by default.** No account or telemetry. Use local speech models or your chosen provider, including OpenAI-compatible services and local servers. API keys stay in the OS credential store; meetings remain Markdown files in your chosen folder. Logs contain timings, lengths and errors rather than dictated or meeting content.
- **One implementation.** Providers, processing, recording policy, lifecycle, history, search and model management share the Rust core. The same Dutch-first interface, fonts and design assets are used on every desktop platform.

Targets: **Apple Silicon/macOS 14.4+**, **Windows 11 x64**, and **Linux x86-64** with WebKitGTK 4.1. Apple Speech requires macOS 26+. Linux Wayland uses a configured desktop shortcut and clipboard delivery instead of a generic global keyboard hook or automatic paste. Real call, device, clipboard, installer and update acceptance remains required on each target.

## Install

Every push to `main` that changes the app publishes a [GitHub release](https://github.com/pennel-app/pennel/releases) with the Mac `.dmg`, Windows `.exe`/`.msi` and Linux `.deb`/`.AppImage`. On a Mac with Apple Silicon:

```sh
brew install --cask pennel-app/pennel/pennel
brew upgrade --cask pennel   # later updates
```

Mac releases are signed with Pennel's own stable certificate, so macOS asks for microphone, accessibility and input-monitoring permission once (again at the first update with this signature) and keeps it across later updates. They are not yet signed with an Apple Developer ID, so the Homebrew cask removes the download quarantine; Windows installers are unsigned and SmartScreen asks for confirmation.

## Build the shared app

Prerequisites: Rust stable, Node 22+, Python 3 and CMake. Mac builds also need Xcode with the macOS 26 SDK or newer; Windows needs the MSVC build tools. Linux build/runtime packages and desktop differences are documented in [desktop development](docs/desktop.md) and [Linux packaging](docs/linux-packaging.md).

```sh
make desktop-preview  # isolated preview installers; use this for migration QA
make desktop-test     # frontend build and Tauri Rust tests
(cd apps/desktop && npm test)
```

On Mac, `make desktop-preview` also creates `build/Pennel Preview.dmg` without Finder layout automation. The native comparison preview is built with `make app-preview`. It creates `build/Pennel Native Preview.app`, with its own data directory and credential service. `make theme-check` verifies that both interfaces use the generated shared theme tokens. See [redesign acceptance](docs/redesign-acceptance.md) for automated, screenshot and device evidence.

For a development session with hot reload:

```sh
cd apps/desktop
npm ci
../../scripts/with-toolchain.sh npx tauri dev
```

`scripts/with-toolchain.sh` uses an existing system toolchain, or a task-local Rust/CMake installation under the ignored `build/toolchain/` directory when present. It does not install tools or edit shell configuration.

`make desktop` builds canonical packages. Mac canonical builds retain the existing bundle identity, data directory and Keychain service. Preview/debug builds use separate data and credentials and keep meetings inside their own directory; they do not import the native app's library.

## Native Mac reference

Keep the reference app available while comparing features and recovering existing installations:

```sh
make core   # Rust core, generated Swift bindings and XCFramework
make test   # core and native reference tests
make app    # build/Pennel.app (native reference)
swift test --package-path apps/macos-platform
```

`make core` generates `apps/macos/Frameworks/` and `apps/macos/Sources/PennelCore/`. The shared app's Swift platform library has no SwiftUI or second Rust engine. Do not run a canonical shared Mac build alongside the older native app against the same data.

## First run

1. New profiles open the setup wizard: choose speech recognition and language, grant the required access, and try a real first dictation. New profiles default to local recognition in **Snel** mode. Existing profiles retain their configuration and open the workspace directly.
2. Use **Dicteren** for the local editable pad, or the configured shortcut in another app. Automatic external paste from the pad is a separate preference, off by default. Microphone access is required for recording; global shortcuts, external placement, meeting audio and optional speech/calendar features have their own access requirements. Providers, models and appearance remain available in **Instellingen**, with unsupported platform controls visible and explained.
3. On an Apple keyboard, set **System Settings → Keyboard → Press 🌐 key to** *Do Nothing* when using `fn`. Logitech MX keyboards do not send Apple's Fn/Globe keycode; choose **Rechter Option** under **Instellingen → Dicteren → Sneltoets** instead. [Logitech's explanation](https://support.logi.com/hc/en-us/articles/29752695205527-Logitech-keyboard-FN-key-behavior-on-macOS-or-iOS).

The **Dicteren** page explains dictation in other apps; **Open dicteerpad** is optional. The small floating pill shows provisional text during Apple or local recognition. Cloud providers that return one final transcript show text after recording, before cleanup; preview never adds cloud requests or switches providers.

On Wayland, bind a desktop keyboard shortcut to `pennel --toggle-dictation`: press once to start, again to finish, then paste the copied result with Ctrl+V. Bind `pennel --toggle-meeting` for manual meeting recording. For an AppImage, use its executable path in the shortcut command. See [Wayland behavior](docs/desktop.md#linux-x11-and-wayland).

Use **Logboek openen** from the tray/menu bar for troubleshooting. Canonical Mac logs remain under `~/Library/Logs/Pennel/`; other builds use `logs/pennel.log` inside their own data directory.

## Verification and distribution

[The migration record](docs/shared-app-migration.md) separates implemented behavior, automated checks, packaged-app inspection and outstanding device acceptance. Memory, installed-size and download-size figures need measurements of the actual release packages; no generic framework figure is used here.

CI builds the shared app for Mac, Windows and Linux and publishes unsigned releases automatically (see [releasing](docs/releasing.md)). The manual signed-candidate workflow prepares packages using existing credentials and publishes no release. Release helpers preserve the legacy Sparkle upgrade path, and require acceptance of exact Mac, Windows and Linux updater artifacts before preparing the update manifest. Linux in-app updates use AppImage; Debian installations use a verified package or package manager. See [release preparation](docs/linux-packaging.md#candidate-and-update-artifacts) and the [native reference release process](docs/releasing.md).

## Local models

Local speech recognition uses [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp). The bundled [`@handy-computer/models`](https://github.com/handy-computer/models) catalog supplies model sizes, supported languages and benchmark data. Mac uses Metal; Windows/Linux default to CPU with an optional Vulkan build. Update the catalog with `scripts/update-catalog.sh <version>`.

## Layout

- `core/` — shared product logic, runtime state machines and stores; UniFFI also serves the native reference.
- `apps/desktop/` — shared Tauri interface, Rust host and Windows/Linux adapters.
- `apps/macos-platform/` — embedded Swift services for Apple APIs.
- `apps/macos/` — native SwiftUI comparison/recovery app.
- `assets/brand/` — shared design guide, icons and fonts.
- `scripts/` — build, inspection, release preparation and acceptance tools.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[Apache License 2.0](LICENSE). "Pennel" and the Pennel logo are trademarks of the project owner; forks must use a different name and logo.
