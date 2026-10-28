# Expo Widgets recording surfaces

Status: source and isolated-runtime prototype verified on 27 October 2026. Signed-device rendering, native action delivery and background behavior are not verified by this document.

This work supports [Expo migration #42](https://github.com/pennel-app/pennel/issues/42), [device acceptance #18](https://github.com/pennel-app/pennel/issues/18) and [signing/store readiness #20](https://github.com/pennel-app/pennel/issues/20). It does not establish the keyboard-only cold-start outcome in [#39](https://github.com/pennel-app/pennel/issues/39).

## Implemented prototype

`apps/mobile/widgets/PennelLiveActivity.tsx` is one pure `'widget'` function shared by the recording and dictation-session factories. It implements the Lock Screen banner, small banner and Dynamic Island compact, minimal and expanded regions. Every appearance value and label reaches the isolated function through serializable props.

`props.ts` derives colors, spacing and typography from the canonical brand tokens, reuses shared English/Dutch status copy, and reproduces the existing `PennelMobileKit.Brand.Wave` silhouette. Payloads contain generic status, timestamps, five bar heights and an app link; they contain no meeting title, transcript, audio, credentials or voice embeddings.

Recording uses the native elapsed timer. A ready session uses a native bounded countdown. Finishing, interrupted, off and stale presentations do not show an active timer. An expired ready snapshot is presented as off; native listening/processing snapshots are not silently replaced by an expiry guess. Stale activities direct the user to verify status in Pennel.

`registerLiveActivityPrototypes()` lazily registers `PennelRecordingPrototype` and `PennelSessionPrototype`. Importing the module or registering factories starts neither recording nor a Live Activity. The prototypes provide **Open Pennel**, not a JavaScript Stop button. `recordingControlAuthority` remains `native-activitykit`.

These are Live Activity layouts, so they do not need new `widgets[]` app-config entries. No new home-screen widget feature is introduced.

## Verified SDK 58 capabilities

The installed packages are `expo-widgets@58.0.23` and `@expo/ui@58.0.23`.

| Capability | Source-level result |
| --- | --- |
| Elapsed/countdown text | Supported. Expo UI `Text` accepts `date`/`dateStyle`, `timerInterval`, `countsDown` and `pauseTime`. Its Swift implementation builds SwiftUI timer text. |
| Widget timer transport | Supported. The widget renderer routes `TextView` nodes to Expo UI's native `TextView`; the isolated-runtime probe verifies dates become native millisecond values. |
| Starting/updating/ending an Expo activity from JavaScript | Supported through the documented factory and instance API. |
| Recovering Expo activities after app restart | The documented factory supplies `getInstances()`. This recovers presentation objects; it does not restore microphone capture. |
| Direct lifecycle calls from Pennel's independent Swift runtime | No public façade was found. `LiveActivityFactory`, `LiveActivity` and `LiveActivityAttributes` are internal Swift types in this package version. |
| Selecting Pennel's recording App Intent from a generic Expo button | Not exposed by the inspected API. The renderer uses the SDK's `LiveActivityUserInteraction` intent. |

Relevant installed source:

- `node_modules/@expo/ui/src/swift-ui/Text/index.tsx` and `ios/TextView.swift` implement timer properties and native rendering.
- `node_modules/expo-widgets/ios/Widgets/ViewRenderer.swift` preserves the native text path and selects the SDK's button implementation.
- `node_modules/expo-widgets/ios/LiveActivityFactory.swift`, `LiveActivity.swift` and `Widgets/WidgetLiveActivity.swift` define internal lifecycle/attribute types.
- `node_modules/expo-widgets/ios/Widgets/Buttons.swift`, `Widgets/AppIntent.swift`, `WidgetsEvents.swift` and `WidgetsModule.swift` implement interaction notifications.

The SDK's Live Activity intent posts a process-local notification named `onExpoWidgetsUserInteraction`. Its event data includes the ActivityKit activity identifier and a control target. A native observer installed at launch is a possible **source-coupled investigation**, because the event sender is public but the notification constant and lifecycle types are internal. It is not a documented stable native control contract, has not been implemented here, and requires exact-version checks and signed-device proof. It also does not provide a public native Start/Update/End API.

Primary references: [Expo Widgets SDK 58](https://docs.expo.dev/versions/v58.0.0/sdk/widgets/), [Expo UI Text SDK 58](https://docs.expo.dev/versions/v58.0.0/sdk/ui/swift-ui/text/), [Apple AudioRecordingIntent](https://developer.apple.com/documentation/appintents/audiorecordingintent).

## Native authority and generated target ownership

The accepted initial integration retains native ActivityKit services for actual recording/session lifecycle and native Stop intents. The Expo layouts are available for controlled prototype rendering without replacing that authority.

The CNG addon must preserve these memberships:

- Compile the existing native `SessionLiveActivity.swift`, `RecordingLiveActivity.swift` and `Controls.swift` into the generated widget target. Exclude the old `@main` widget bundle; register their configurations alongside the generated SDK configuration.
- Compile the shared public activity attributes through the lightweight MobileKit pod used by both the app/runtime and widgets.
- Compile native App Intent declarations directly into both generated app and widget targets. Define `PENNEL_APP` only in the app, and compile shortcut declarations into the app so Xcode extracts their metadata.
- Keep all source outside Expo's generated widget directory, because the SDK generator removes that directory during Prebuild. The addon must add target references and configurations idempotently after generation.
- Include the existing bundled Figtree resources and preserve the configured extension bundle identifier/App Group for each preview or production variant.
- Bootstrap native runtime/action registration before JavaScript. Do not route native Stop through a JavaScript event listener.

After OS termination or force-quit, the microphone has ceased. Reopening must report interruption and reconcile stale activities; no prototype or recovered activity is evidence that recording continued.

## Reproducible probes and remaining acceptance

From `apps/mobile`:

```sh
node --test widgets/tests/live-activities.test.cjs
node_modules/.bin/tsc --ignoreConfig --noEmit --strict --target ES2022 --module preserve --moduleResolution bundler --jsx react-jsx --resolveJsonModule --esModuleInterop --skipLibCheck widgets/*.ts widgets/*.tsx
```

Seven tests passed. They use the installed SDK's Babel widget serializer, widget JavaScript normalizer/React stubs and Expo UI wrappers. They verify self-contained serialization, native timer payloads, valid countdown ranges, expiry/stale/inactive states, canonical props, app-only links and side-effect-free registration. They do not run SwiftUI or emulate ActivityKit scheduling.

| Check | Evidence |
| --- | --- |
| Pure widget serialization and SDK JavaScript rendering | Passed by the seven probes. |
| Widget TypeScript API compatibility | Passed by the isolated typecheck above. |
| Repeated clean/non-clean Prebuild retains native sources and widget bundle registration | Passed by the CNG integration test and actual pod/source ownership checks. |
| Native timer advances with app JavaScript suspended, including session expiry | Pending signed-device acceptance. |
| Native Start/Update/End and Stop while foreground, locked or suspended | Generated app/widget sources compile in Release; interaction while locked/suspended remains pending signed-device acceptance. |
| OS termination/force-quit reports interrupted capture and clears stale presentation | Pending device acceptance. |
| VoiceOver, enlarged text, Always-On, font loading and compact-region fit | Pending owned renders/device acceptance. |
| Extension package size, peak memory, CPU and battery | Pending measurements of the exact signed candidate. |

The prototype work launched no simulator or physical-device capture and introduced no production or Cloud action. Neither Expo control parity nor the device/store issues should be closed from these source/JavaScript checks.
