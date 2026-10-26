# Pennel for iPhone

The iPhone app is native SwiftUI on the shared Rust core (`pennel-core`), linked as an XCFramework.

## Build

```sh
make ios-core   # XCFramework (ios-arm64 + ios-arm64-simulator) and Swift bindings
make ios        # ios-core, then an unsigned Release device build; prints the .app size
make ios-test   # ios-core, swift test for PennelMobileKit, then the PennelTests on the simulator (IOS_DESTINATION, default iPhone 17)
```

Requirements: Xcode, the Rust targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`
(`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`).

`scripts/build-core-ios.sh` builds the core with `--no-default-features` (no local models) and
`IPHONEOS_DEPLOYMENT_TARGET=26.0`, writes the XCFramework to `apps/ios/Frameworks/` and the generated
bindings to `apps/ios/PennelCore/` (both git-ignored). It fails when the generated `pennel_core.swift`
differs from the macOS build in `build/bindings/`, so the uniffi surface stays identical; run `make core`
first so that check runs.

## Project

`apps/ios/Pennel.xcodeproj` is a hand-written project (no generator). Its source folders are synchronized
folders, so new Swift files in `Pennel/Sources`, `PennelKeyboard/Sources`, `PennelWidgets/Sources` or
`PennelTests` join their target without editing the project. Files in `Shared/` are compiled into both the app and
the widgets extension (Live Activity attributes and the intents its buttons run); the app target defines the Swift
condition `PENNEL_APP` so shared intents only perform their action in the app.

| Target | Bundle id | Contents |
| --- | --- | --- |
| `Pennel` | `app.pennel.ios` | The app: one `PennelEngine` (`AppModel.shared`), the bindings from `PennelCore/`, `PennelCoreFFI.xcframework`, `PennelMobileKit` |
| `PennelKeyboard` | `app.pennel.ios.keyboard` | Dictation keyboard. Links `PennelMobileKit` only, never the core; no network, no logging, no Keychain access group; Figtree Medium/SemiBold. It writes only `command.json` and reads `session.json` / `result.json` |
| `PennelKeyboardTests` | | Logic tests of `KeyboardModel`; `PennelKeyboard/Model/` (no UIKit) is compiled into both the extension and this hostless bundle, because an extension cannot host tests |
| `PennelWidgets` | `app.pennel.ios.widgets` | Session Live Activity; links `PennelMobileKit` and Figtree Medium/SemiBold |
| `PennelTests` | | Swift Testing, hosted by the app on the simulator |

- Data: the core runs only in the app, with its data in `<app container>/Library/Application Support/Pennel`.
  Meetings go to `Documents/Meetings` (visible in Files under "Op mijn iPhone › Pennel"); the app re-points
  `meetings_dir` there on every launch because the container path changes when iOS reinstalls the app. Nothing the core
  locks lives in the App Group `group.app.pennel`.
- API keys: Keychain service `app.pennel.ios`, `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
- Fonts: the targets copy Figtree and Instrument Serif straight from `assets/brand/fonts/` in their resources phase
  and register them through `UIAppFonts`.
- Simulator builds are signed ad hoc (needed for the Keychain and App Group in tests); device builds are unsigned.

## Dictation session (keyboard hand-off)

`pennel://sessie/start` opens the app and starts `DictationSession`: the audio session goes to `.playAndRecord` with
`.mixWithOthers` and `.allowBluetoothHFP`, the microphone runs, and the Live Activity shows `Pennel staat klaar · m:ss`
with **Stop**. While `ready` every sample is dropped; only samples between a keyboard `start` and its `stop`/`cancel`
are kept. Files live in `<App Group>/handoff/` (`HandoffStore.groupDirectory`): the app writes `session.json` once per
second (10 times per second while listening, with the level) and `result.json` with the id of the keyboard's `stop`
command. The session ends, and the audio session is deactivated, after the chosen duration without a dictation
(default 5 minutes), on **Stop**, on a phone call or other interruption (`Sessie gestopt door een oproep.`), on a
media-services reset and when the app terminates; if the app is killed, the keyboard reads `off` after 5 s without
a heartbeat.

## Keyboard

Dictation pad (pill with five bars) and an ABC letter pad (QWERTY by default; AZERTY from Instellingen > Dicteren > Toetsenbord > Indeling,
stored as the name `qwerty`/`azerty` in the App Group defaults, read without Full Access as the default). Hold the pill to talk until release; tap to talk
hands-free until the next tap; the cross cancels. Taps are ignored while the app is processing (the app drops a `start` then). Without Full Access the pad shows the
explanation and **Open Instellingen**; letters always work. Opening `pennel://sessie/start` and Settings lives only in
`AppOpener.swift`; setting `isEnabled` to false (App Review, spec R1) shows `Open Pennel en tik op Start sessie.` instead.
A result is inserted once, only for the id of the keyboard's own stop command; **Laatste dictaat invoegen** appears only when a fresh keyboard finds an uninserted result.
Gap: the Full Access flag for onboarding is Task 12; the opener returns true when the responder chain accepted the call, so a silent refusal by iOS still needs the device check.

Keyboard memory (Instruments Allocations, 10 dictations plus letter typing, target under 30 MB): pending device acceptance.
Simulator Release build of `PennelKeyboard.appex`: 782850 bytes in total (binary 651 KB, two fonts).

## Not available on iPhone

Shown disabled with the reason, never hidden (also listed in `docs/desktop.md`): CLI agents, local models,
echo cancellation, call detection / calendar / per-app and per-meeting recording rules, and style per app.

## Sizes

Measured with `scripts/build-core-ios.sh` (unstripped archives are about 93 MB): each release `libpennel_core.a` is copied and stripped with
`strip -S -x`, then its byte size is read.

| Slice | Stripped `libpennel_core.a` |
| --- | --- |
| ios-arm64 | 23006680 bytes |
| ios-arm64-simulator | 22998400 bytes |

Unsigned Release build, not App Store thinned size (`make ios`, Xcode 27, sum of all files in `Pennel.app`):
10403967 bytes (about 10.4 MB; the app binary is 9.0 MB, keyboard extension 0.5 MB, widgets extension 0.6 MB
including two Figtree fonts).

App Store size: pending device acceptance.

## Device acceptance

Pending device acceptance (no physical device available yet): memory profile in Instruments, 60-minute
meeting, dictation through the keyboard on a device, archive and App Store size, and the session check (start a
session, lock the phone for 2 minutes, unlock, dictate from the keyboard; the orange dot disappears at expiry).
