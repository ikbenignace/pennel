# Historical native iPhone implementation

The SwiftUI shell and hand-written Xcode project described here were retired under [cleanup #75](https://github.com/pennel-app/pennel/issues/75) on 30 October 2026. This page preserves the old implementation, build receipts and draft review/privacy notes as historical context. Its old paths, targets, labels and measurements are not current build or submission instructions. [Pre-retirement source](https://github.com/pennel-app/pennel/tree/e6f02b2cd178b541dcbfcdb46125336c762dbf07/apps/ios) remains in Git.

Use [Expo mobile development and acceptance](mobile.md) for the active app, native ownership and current commands. Native services, MobileKit, keyboard and widget helpers now live under `apps/mobile/native/ios/`. Device, signed-upgrade, keyboard cold-start, deployed Cloud and store acceptance remain open. Revalidate the draft App Review and privacy notes below against the actual submitted Expo candidate.

## Historical build (retired)

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
| `PennelKeyboard` | `app.pennel.ios.keyboard` | Dictation keyboard. Links `PennelMobileKit` only, never the core; no network, no logging, no Keychain access group; Figtree Medium/SemiBold. It writes only `command.json` (plus the boolean `keyboardFullAccessSeen` flag in the App Group defaults, so the app can tell that Full Access is on) and reads `session.json` / `result.json` |
| `PennelKeyboardTests` | | Hostless logic tests of the shared MobileKit `KeyboardSessionModel`; the extension retains its `KeyboardModel` alias |
| `PennelWidgets` | `app.pennel.ios.widgets` | Session and recording Live Activities; links `PennelMobileKit` and Figtree Medium/SemiBold |
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
a heartbeat. The **Dicteersessie** card at the top of the Dicteren tab shows `Uit` with **Start sessie**, or
`Klaar · nog m:ss` with **Stop sessie** and the explanation, `Luistert…` and `Pennel verwerkt je dictaat.` (spec §4.2).

## Keyboard

The shared native keyboard provides QWERTY/AZERTY, shift/caps lock, numbers and symbols, repeat delete,
contextual return/email/URL keys, keyboard switching and opt-in local suggestions. `UILexicon` is the
system supplement; Pennel's dictionary is shared only when Full Access permits the App Group. Typing
and switching remain available without Full Access. Interface copy is English or Dutch.

Hold the pill to talk until release; tap for hands-free dictation; the cross cancels. Dictation requires
an already-ready containing-app microphone session. When it is off, the keyboard explains how to start
one in Pennel. The responder-chain app launcher has been removed; `AppOpener` only opens Settings.
This interim flow does not complete [keyboard-only cold start #39](https://github.com/pennel-app/pennel/issues/39).

Automatic insertion requires an unconsumed result for the keyboard's own stop command and the same
live keyboard/document/focus context. Context changes or keyboard restart require explicit recovery;
document identifiers are not assumed durable across launches. See the [feasibility and verification
receipt](mobile-keyboard-feasibility.md) for the public API probe, state matrix and pending device work.
The first time the keyboard runs with Full Access it sets one boolean in the App Group defaults
(`keyboardFullAccessSeen`, `KeyboardFullAccess`), never any text; the first launch and Instellingen › Dicteren ›
Toetsenbord read it to show whether the keyboard is ready. Without Full Access the keyboard cannot reach the group, so the
flag is not cleared when Full Access is switched off later.
The historical Full Access flag records a prior observation; it is not proof of the current permission.
The keyboard uses its current access/session state before claiming readiness.

Keyboard memory (Instruments Allocations, 10 dictations plus letter typing, target under 30 MB): pending device acceptance.
The current unsigned extension size and build receipt are in the feasibility document. Package size
and compiler tests do not establish peak device memory or keyboard-only cold-start behavior.

## Meetings

Under **Nieuwe meeting**, an optional title and **Start opname** (the desktop's labels) record a physical meeting from the microphone as one track, the existing **Anderen** track, so the
desktop reads it unchanged. `MeetingRecorder` places the 16 kHz samples on the meeting timeline and sends them to
`MeetingSession::push_audio_at(Speaker::Others, …)` in whole seconds that start on a millisecond. It also writes `anderen.m4a`
(AAC, 16 kHz mono, 32 kbit/s), with silence in the gaps, so **Afspelen vanaf dit punt** lands on the right moment. Recording
continues with the screen locked (background mode `audio`). The Live Activity shows the time and **Stop opname**
(`StopMeetingIntent` in `Shared/`).

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

## First launch

Shown once, while the core's `onboarding.status` is `pending` (set to `completed` or `skipped` afterwards; the iPhone's
steps differ from the desktop's, so `resume_step` is not used). Instellingen › Weergave › **Introductie opnieuw openen**
shows it again, like the desktop. Steps (spec §4.7): **Welkom bij Pennel** with `Drie dingen en je kan dicteren in elke
app.`; **Microfoon toestaan**; **Hoe wil je dicteren?** with **Pennel Cloud**, **Apple-dicteren** or **Eigen provider**;
then the Account screen (login) only after Pennel Cloud, or the Providers screen only after Eigen provider; **Toetsenbord
toevoegen en Volledige toegang aanzetten** with the Full Access explanation and the keyboard's status; and the optional
**Action Button instellen**. The choice points dictation and meeting speech at Apple-dicteren at once, at Pennel Cloud only
after a login, and at an own provider only once it has a key. **Later instellen** closes it at any step. Shared labels
(**Aan de slag**, **Verder**, **Terug**, **Klaar**, **Later instellen**, **Geef toegang**, **In orde**) come from
`apps/desktop/src/views/onboarding.ts`.

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

Unsigned Release build, not App Store thinned size (`make ios`, Xcode 27, sum of all files in `Pennel.app`, measured
2026-10-26 with all iPhone features): 14074619 bytes (about 14.1 MB; the app binary is 12072104 bytes, the keyboard
extension 874690 bytes, the widgets extension 876296 bytes, each extension including two Figtree fonts).

App Store size: pending device acceptance.

## Device acceptance

Pending device acceptance (no physical device available yet): memory profile in Instruments, 60-minute
meeting with the screen locked and a phone call in the middle, dictation through the keyboard on a device, archive and App Store size, and the session check (start a
session, lock the phone for 2 minutes, unlock, dictate from the keyboard; the orange dot disappears at expiry).

## Shortcuts, Action Button, Back Tap and Control Center

- Intents live in `apps/ios/Shared/` (compiled into the app and the widgets extension; the action runs in the app only under `PENNEL_APP`): **Dicteer naar klembord**, **Start dicteersessie**, **Start opname** (`supportedModes = .foreground`, iOS 26 API) and **Stop dicteersessie**, **Stop opname** (`AudioRecordingIntent` + `LiveActivityIntent`, `.background`, bound to the Live Activity buttons). `IntentEnvironment` makes their dependencies injectable for tests.
- `PennelShortcuts` offers Dutch phrases, each with the app name; the test reads them from the built `Metadata.appintents`.
- Control Center: **Dicteer**, **Dicteersessie**, **Opname** (`ControlWidgetButton`, `PennelWidgets/Sources/Controls.swift`). There is no stop button in Control Center yet; Stop sessie is in the app and the Live Activity.
- Settings › Dicteren › Snel starten: session duration 5 min (default), 15 min, 1 uur (`SessionClock.Duration`, stored in `UserDefaults.standard`, read at every session start) and the Action Button / Tik achterop / bedieningspaneel explanation.

Pending device acceptance (no device available):
- Spike: start a meeting from the Action Button with the phone locked and unlocked, with `StartMeetingIntent.supportedModes = .background` as an `AudioRecordingIntent` that starts its Live Activity before `AVAudioSession.setActive(true)`. Record the result per iOS version here. Until it works on iOS 26 and 27 the foreground hop stays.
- Manual: each intent from the Action Button, Back Tap (via Opdrachten), Siri and Control Center; the Stop buttons on the Lock Screen Live Activities with the phone locked.

## App Review notes

Text for the App Review notes field in App Store Connect (English, for the reviewer):

> Pennel is a dictation and meeting-notes app. Everything works without an account.
>
> **Testing without an account.** On first launch choose "Apple-dicteren" (Apple's on-device speech recognition, no
> account or key). Then: Dicteren tab → "Dicteer naar klembord" records, transcribes on the device and puts the text on
> the clipboard. Meetings tab → "Start opname" records a meeting from the microphone; "Stop opname" finishes it and shows
> the transcript.
>
> **Keyboard session setup.** Keyboard extensions cannot directly use the microphone. In this candidate the user starts
> a bounded session in Pennel or through its system actions, then returns to the receiving app. The keyboard does not
> launch Pennel or another app; its Settings action is limited to system settings. Full Access enables the small-file
> handoff with Pennel and opt-in local dictionary preferences. Typed text is not uploaded or logged. Letter keys and
> keyboard switching work without Full Access. A completely keyboard-only cold start remains under investigation.
>
> **Why background audio is used.** After the user starts a dictation session, the app keeps the microphone open in the
> background (background mode `audio`) for a limited time (5 minutes by default, chosen by the user: 5 min, 15 min or
> 1 hour) so the keyboard can dictate in other apps. A Live Activity ("Pennel staat klaar") is shown the whole time with a
> Stop button, and the session ends on Stop, on a phone call, or when the time runs out. Audio is only kept between a
> dictation start and stop on the keyboard; while the session waits, every sample is discarded. Meeting recordings started
> by the user also continue with the screen locked, with their own Live Activity and Stop button.
>
> **Pennel Cloud (optional).** Signing in shows a short code in the app and opens the Pennel Cloud verification page
> in the browser, where the user confirms that code; the app checks every few seconds until the sign-in is confirmed. Review account: an e-mail address with a mailbox
> shared with App Review, given in the App Review "Sign-in information" fields of App Store Connect.

The review Cloud account itself (a mailbox to share, or a fixed test code on the server in `pennel-cloud`, spec R9) does
not exist yet: create it before the first TestFlight review and fill in the sign-in fields. A screen recording of the
keyboard session (spec R3) is also still to be made on a device.

**Export compliance.** `Info.plist` declares `ITSAppUsesNonExemptEncryption = false`, while the Cloud vault uses
end-to-end encryption. Whether that encryption qualifies for the exemption must be confirmed by the owner before the
first TestFlight upload; change the key (and file the documentation App Store Connect asks for) if it does not.

## Privacy label answers

For App Store Connect, App Privacy (to be confirmed against `pennel-cloud` before submitting):

- **Tracking:** no. No third-party SDKs, no advertising, no analytics, no crash reporting.
- **Data collected:** Contact Info → Email Address, linked to the user, purpose App Functionality, only when the user signs
  in to Pennel Cloud. Nothing is collected without an account.
- **Meetings synced through Pennel Cloud** are end-to-end encrypted with a key Pennel cannot read, so they are not
  collected data in Apple's sense.
- **Audio and text sent for transcription or cleanup** go to the provider the user chose (Pennel Cloud or their own
  provider) for that request only. If the Pennel Cloud server keeps any of it beyond the request, add Audio Data / Other User
  Content (App Functionality, linked) to the label.
- Apple-dicteren and Apple Intelligence run on the device; nothing leaves the iPhone.

