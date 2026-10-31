# iPhone keyboard: implementation and feasibility evidence

Evidence date: 27 October 2026; reproduction paths updated for shell retirement on 30 October 2026. Tracking: [keyboard improvement #39](https://github.com/pennel-app/pennel/issues/39), [Expo migration #42](https://github.com/pennel-app/pennel/issues/42), [device acceptance #18](https://github.com/pennel-app/pennel/issues/18), [signing/review #20](https://github.com/pennel-app/pennel/issues/20).

## Current conclusion

The requested complete keyboard-only flow, including a cold or expired containing-app session, is **not verified**. An active microphone session can still receive the keyboard's start/stop/cancel commands. The implementation displays the inactive-session requirement honestly and never launches the containing app through the responder chain. This interim capability does not complete #39, and no decision to release with an alternative has been made.

Source inspection and compiler checks are not signed-device or App Review evidence. The available paired iPhone was disconnected during the environment inspection; no physical microphone, force-quit, keyboard memory, or App Review acceptance was performed for this change.

## Implemented behavior

- Regular typing is the default surface, with QWERTY/AZERTY, one-shot shift and double-tap caps lock, numbers/symbols, hold-to-repeat delete, return-key traits, email/URL punctuation, and double-space period for suitable text contexts.
- A visible Pennel microphone control supports tap and hold gestures for an already-ready session. Cancellation covers listening and processing; the native runtime owns stopping and cancelling the actual recording/recognition.
- Typing and the system keyboard switch remain available without Full Access. The controller reads its current `hasFullAccess`; the app's historical Full Access flag is not treated as current permission evidence.
- Suggestions default off. Explicit opt-in enables a bounded local prefix index using `UILexicon` and, when Full Access is available, the dictionary published by Pennel. No keyboard input, prefix, suggestion, or lexicon is sent to a provider or logged. The containing app writes only dictionary preferences, not typed text.
- The keyboard releases its hosting view and transient typing/lexicon state when dismissed. Letter layout and interface-language preferences may be mirrored locally so disabling Full Access does not prevent regular typing.
- A dictation may insert automatically only in the same live document, keyboard lifetime, and focus/selection generation that started it, and only for its own stop-command ID. A field/cursor change or keyboard reappearance leaves the result for an explicit recovery action. There is no durable document identity assumption across process launches.
- Insertion is consumed before delivery. A processing cancellation suppresses a late result rather than offering it for insertion. The native runtime must also prevent cancelled processing from publishing or committing a late result.
- Preview/production App Groups and Darwin signals resolve `PennelAppGroupIdentifier` and `PennelHandoffNamespace` from each bundle's Info.plist. Missing keys preserve the old native production identity; malformed explicit values fail away from production rather than falling back into it.

The UIKit-free `KeyboardSessionModel`, typing state, preferences, and delivery guard live in MobileKit. The native extension uses those tested helpers. Its existing keyboard test bundle contains integration checks for the shared model.

## Public API findings

[Apple's custom-keyboard guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html) documents the keyboard microphone restriction, host-controlled text selection, and supplementary lexicon. Full Access permits shared-container access; it does not establish an exception for microphone capture. [Review guideline 4.4.1](https://developer.apple.com/app-store/review/guidelines/#extensions) restricts app launching and requires ordinary input and keyboard switching without Full Access.

`UITextDocumentProxy.documentIdentifier` is a public UUID used to distinguish documents. This implementation combines it with a process-local lifetime and context generation; it does not infer the host application's identity or persist an insertion destination. [Apple API](https://developer.apple.com/documentation/uikit/uitextdocumentproxy/documentidentifier).

`requestSupplementaryLexicon` is public and supplies terms for local suggestions, including common words, text shortcuts and selected names. It is a supplement to Pennel's own bounded dictionary, not a claim to Apple's complete autocorrection implementation. [Apple API](https://developer.apple.com/documentation/uikit/uiinputviewcontroller/requestsupplementarylexicon(completion:)).

### App Intents candidate

The guarded compiler/process-routing probe is `apps/mobile/native/ios/Keyboard/Feasibility/KeyboardRecordingIntentProbe.swift`. It declares an `AudioRecordingIntent` with background supported modes and a SwiftUI `Button(intent:)`. With `PENNEL_KEYBOARD_FEASIBILITY` defined, both declarations typecheck under the iOS application-extension restriction. The probe is outside the production keyboard source directories and is disabled without that compilation flag.

The probe never requests microphone permission, starts capture, or reports recording success. If deliberately installed in a signed test harness, it writes only a bundle/process receipt with `recordingStarted: false` to the configured App Group. That receipt can establish where `perform()` ran; it cannot establish containing-app wakeup or microphone eligibility.

Next signed-device investigation:

1. Compile the intent declarations into the test app and keyboard targets, preserving native App Intents metadata.
2. Invoke the dedicated probe button and inspect the receipt to establish execution process, whether the host app appears, and whether behavior differs for resident/suspended/cold/force-quit states.
3. Only if the system provides a documented, review-compatible host execution path, test the real native recording action and required Live Activity on a controlled test recording. Record actual microphone indicators and background limits.
4. If no allowed path is established, document the exact barrier and alternatives for the user's product decision. Do not substitute direct `perform()` calls, a forced app launch, a private API, an unrelated background mode, or an indefinitely active microphone.

Adopting `AudioRecordingIntent` does not itself prove permission to begin audio capture from a keyboard or background process. [Apple recording-intent documentation](https://developer.apple.com/documentation/appintents/audiorecordingintent).

## Alternatives for the release decision

If the signed cold-start investigation cannot establish the requested interaction, these are reviewable product choices:

| Alternative | Actual interaction and remaining gap |
| --- | --- |
| Explicit bounded session | Start a visible, time-limited session in Pennel, then dictate repeatedly from its keyboard. Requires preparation outside the keyboard. |
| User-invoked system action | Start the session through an accepted Shortcut, Action Button, Back Tap or Control Center action, subject to the OS foreground/permission behavior verified on device. Requires a separate system action. |
| In-app dictation and ordinary keyboard | Ship local typing plus in-app dictation/copy while retaining the full keyboard-only interaction as follow-up. Dictation does not begin from another app's keyboard. |

None of these alternatives has been accepted as fulfillment of #39. Keep the issue open and make the release decision after the device evidence; the implementation does not silently select an alternative.

## Evidence matrix

| State or scenario | Automated/source evidence | Required device evidence |
| --- | --- | --- |
| Ready app session, tap/hold and stop | Command sequence and same-context once-only delivery unit tests pass | Real insertion in representative host apps |
| App session off or expired | No launch/no start command; truthful inactive state and heartbeat expiry tests | Cold-start candidate and user-visible interaction |
| App suspended | Native handoff design retained; keyboard compile passes | Recording/processing continue within actual OS limits |
| App terminated or force-quit | No attempt to resurrect capture; stale heartbeat reads off | Exact OS behavior, microphone release and recovery |
| Field/selection change while processing | Result is deferred; document and generation tests pass | Host callbacks and cursor movement in actual apps |
| Keyboard dismissed/recreated | No automatic result insertion across keyboard lifetimes | Cross-app switching and keyboard process termination |
| Cancel during processing, late result | Late result is not inserted or offered in the keyboard tests | Native inference cancellation and history behavior |
| Full Access disabled | No handoff command; local typing/switching remain separate | Permission revocation/re-enabling and Settings behavior |
| QWERTY/AZERTY, caps, symbols, spacing | Shared typing-state and preference tests pass | Layout, gestures, long press, repeat timing and feedback |
| Local suggestions | Opt-in, bounded entries, case handling and dictionary removal tested | UILexicon behavior, consent, input latency and memory |
| VoiceOver/Dynamic Type | Accessible control labels and bounded key growth compile | Real focus order, pronunciation, hit targets and large text |
| Peak memory/battery/latency | No whole document is retained; lexicon and dictionary are bounded | Instruments peak under 30 MB in worst tested state; record CPU, battery, timing and package size |

Test on supported iOS 26 and iOS 27 devices, recording exact patch versions. Include secure fields, phone pads, apps that reject third-party keyboards, long host documents, AZERTY + symbols + dictation, lock/unlock, connectivity changes and phone-call interruptions. Automated/simulator observations must remain separate from these signed-device results.

## Reproducing the compiler checks

From the repository root, with Xcode selected:

```sh
swift test --package-path apps/mobile/native/ios/MobileKit
probe_sdk_path="$(xcrun --sdk iphoneos --show-sdk-path)"
probe_build_dir="$(mktemp -d -t pennel-keyboard-probe)"
xcrun swiftc -parse-as-library -swift-version 6 -target arm64-apple-ios26.0 \
  -sdk "$probe_sdk_path" -application-extension -emit-module -enable-testing \
  -module-name PennelMobileKit -emit-module-path "$probe_build_dir/PennelMobileKit.swiftmodule" \
  apps/mobile/native/ios/MobileKit/Sources/PennelMobileKit/*.swift
xcrun swiftc -typecheck -parse-as-library -swift-version 6 -target arm64-apple-ios26.0 \
  -sdk "$probe_sdk_path" -application-extension -I "$probe_build_dir" -module-name PennelKeyboard \
  apps/mobile/native/ios/Keyboard/Model/*.swift apps/mobile/native/ios/Keyboard/Sources/*.swift
xcrun swiftc -typecheck -parse-as-library -swift-version 6 -target arm64-apple-ios26.0 \
  -sdk "$probe_sdk_path" -application-extension -I "$probe_build_dir" \
  -DPENNEL_KEYBOARD_FEASIBILITY -module-name PennelKeyboardIntentProbe \
  apps/mobile/native/ios/Keyboard/Feasibility/KeyboardRecordingIntentProbe.swift
```

Observed toolchain: Xcode 27.0 (27A266a), iPhoneOS SDK 27.0, Swift 6.4. Deployment target for these checks: iOS 26.0. These commands typecheck source; they do not produce signed-device or runtime acceptance.

## Local build receipt

The working-tree keyboard change was checked on 27 October 2026:

- MobileKit: **56 tests passed**, including typing/preferences and the relocated UIKit-free keyboard-session tests.
- Keyboard source and the guarded App Intents probe: iOS 26.0 deployment-target application-extension typechecks passed against SDK 27.0.
- Existing native `PennelKeyboard` target: unsigned Debug simulator build and unsigned Release iPhoneOS build passed. Neither command booted a simulator or exercised a physical microphone.
- Unsigned Release extension file total: **1,245,658 bytes**. Binary SHA-256: `249c36c9edda52d3645c2ea95dc0b121769e096b13fcb42a48ee7546f0613226`.

This is an isolated extension build from the integration working tree, not the signed Expo app, an App Store download-size measurement, or a keyboard-memory result. The final integrated/signed candidate must receive its own artifact receipt. App Intents metadata extraction was skipped for this production keyboard target because it does not link the guarded probe or AppIntents; app/widget intent metadata remains a separate acceptance check.
