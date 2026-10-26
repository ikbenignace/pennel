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

## Meetings

Under **Nieuwe meeting**, an optional title and **Start opname** (the desktop's labels) record a physical meeting from the microphone as one track, the existing **Anderen** track, so the
desktop reads it unchanged. `MeetingRecorder` places the 16 kHz samples on the meeting timeline and sends them to
`MeetingSession::push_audio_at(Speaker::Others, …)` in whole seconds that start on a millisecond. It also writes `anderen.m4a`
(AAC, 16 kHz mono, 32 kbit/s), with silence in the gaps, so **Afspelen vanaf dit punt** lands on the right moment. Recording
continues with the screen locked (background mode `audio`). The Live Activity shows the time and **Stop opname**
(`StopRecordingIntent` in `Shared/`).

- **One microphone:** `SharedMicrophone` runs one `AVAudioEngine` for the meeting, the keyboard session and in-app
  dictation. `SharedAudioSession` reference-counts the `AVAudioSession`, so whichever stops first never cuts the others.
  While a dictation runs during a meeting, the meeting receives silence for that time. It also gets `set_dictating`
  and `mark_dictation`, like the desktop; the core itself only silences the "Ik" track.
- **Interruptions:** a call pauses the recording. When the call ends, Pennel reactivates the audio session and continues;
  the timeline follows the clock, so the transcript has a gap instead of shifted text. If the microphone does not come
  back, the meeting is finished with what was recorded and is marked as stopped early. A meeting with a gap or an
  early stop is never shown as a complete recording: the meeting page lists the interruption (kept per meeting in the
  app's preferences, because the meeting files have no field for it), and no automatic summary runs after an early stop.
  When samples stop without an interruption (the engine died on a route change), `SharedMicrophone` restarts the
  engine; after 2 s without samples the meeting restarts its capture and shows a warning, and the silent stretch is
  recorded as a gap. A recording whose microphone never came back stops early.
- **Finishing:** after **Stop opname** the core transcribes the last chunks and saves. The meeting id is exposed as
  `MeetingRuntime.activeMeetingId` until then, so sync never sends a meeting that is still recording. The automatic
  summary then runs when a language model is ready. Speaker recognition does not run on iPhone.
  `BGContinuedProcessingTask` `app.pennel.ios.finish` is requested at every stop, while Pennel is still in front (iOS only
  grants it for a user action), and ends when finishing is done; leaving the app keeps finishing going with progress.
- **Library:** **Zoeken in meetings**, grouped like the desktop library by **Vandaag**, **Deze week**, **Eerder**. Each
  meeting has **Notities**, **Transcript** and **Vragen**, plus **Samenvatten** / **Opnieuw genereren** with
  **Sjabloon**. **Delen** offers **Kopieer opgemaakt** (HTML and plain text on the pasteboard),
  **Kopieer als Markdown**, **Kopieer met transcript** and **Deel…** (the iOS share sheet). The **Vraag** tab asks
  across all meetings.

Device acceptance pending: a 60-minute meeting with the screen locked on the oldest supported iPhone, with one phone call
in the middle; also whether iOS accepts the finish task identifier without a wildcard suffix.

## Pennel Cloud

The **Account** screen in Settings follows the desktop's Account tab (`apps/desktop/src/views/account.ts`) section for
section: sign-in, usage, **Meetings synchroniseren**, **Herstelcode**, **Toestellen**, **Account verwijderen**. The device name
defaults to `iPhone` (iOS no longer gives the real name) and can be changed; the platform sent is `ios`. The verification
page opens in the default browser and the app polls like the desktop (interval, +5 s on `slow_down`, stop after 15 minutes).

- **Secrets:** `KeychainSecretStore` is the core's `SecretStore` (accounts `pennel-cloud-refresh`, `pennel-cloud-vault-key`),
  service `app.pennel.ios`, `AfterFirstUnlockThisDeviceOnly`, so sync works with the screen locked during a recording.
- **Recovery code:** shown once from memory, never stored. **Kopiëren** puts it on the pasteboard as local-only with a
  2-minute expiry; **Afdrukken** uses the system print sheet.
- **Sync runner:** `CloudSyncRunner` runs `cloud_sync_run` off the main actor, one pass at a time with at most one queued
  rerun. Triggers: app foreground, a finished meeting, writes (notes, summary, rename, delete, retranscribe, conflict
  removal), every 180 s while in the foreground, and `BGAppRefreshTask` `app.pennel.ios.sync` (iOS decides when it runs).
  Every pass receives `MeetingRuntime.activeMeetingId`, so the meeting being recorded is never pushed. The core only
  excludes the ids it is given per pass, so `MeetingRuntime.start` first holds the runner: new passes are deferred and the
  pass in flight is awaited (off the main thread, at most 15 s, then **Pennel is nog aan het synchroniseren…**), the folder
  is created and `activeMeetingId` set without suspending, and then the hold is released.
- **Badges and conflicts:** each meeting row shows **Gesynchroniseerd** / **Wacht op sync** / **Conflictversie**; the meeting
  page shows the desktop's conflict banner with **Bekijken** (the file's text in a sheet) and **Verwijderen**.
- **Provider card:** Pennel Cloud shows `Ingelogd als …` or **Log in bij Pennel Cloud**, which opens Account.
- Not on iPhone: the Account screen has no Claude access section.

Pending device acceptance: record on the iPhone and see the meeting on the Mac within a minute of finishing; edit notes on
the Mac and see them on the iPhone at the next foreground; whether `BGAppRefreshTask` ever runs for a given user.

## Not available on iPhone

Shown disabled with the reason, never hidden (also listed in `docs/desktop.md`): CLI agents, local models,
echo cancellation, call detection / calendar / per-app and per-meeting recording rules, style per app, and speaker
recognition and naming in meetings.

## Sizes

Measured with `scripts/build-core-ios.sh` (unstripped archives are about 93 MB): each release `libpennel_core.a` is copied and stripped with
`strip -S -x`, then its byte size is read.

| Slice | Stripped `libpennel_core.a` |
| --- | --- |
| ios-arm64 | 23553568 bytes |
| ios-arm64-simulator | 23544600 bytes |

Unsigned Release build, not App Store thinned size (`make ios`, Xcode 27, sum of all files in `Pennel.app`):
12774231 bytes (about 12.8 MB; the app binary is 10.3 MB, keyboard extension 0.8 MB, widgets extension 0.8 MB
including two Figtree fonts).

App Store size: pending device acceptance.

## Device acceptance

Pending device acceptance (no physical device available yet): memory profile in Instruments, 60-minute
meeting with the screen locked and a phone call in the middle, dictation through the keyboard on a device, archive and App Store size, and the session check (start a
session, lock the phone for 2 minutes, unlock, dictate from the keyboard; the orange dot disappears at expiry).
