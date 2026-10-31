# Mobile workflows, sync setup and feedback

Tracking: [app #77](https://github.com/pennel-app/pennel/issues/77), with the existing [keyboard acceptance #39](https://github.com/pennel-app/pennel/issues/39), [device acceptance #18](https://github.com/pennel-app/pennel/issues/18) and [store acceptance #20](https://github.com/pennel-app/pennel/issues/20). Private Cloud implementation and operational evidence remain in the Cloud repository.

## Approved product contract

- Preserve Pennel's canonical tokens, Figtree/Instrument Serif, native navigation, English/Dutch parity and account-free local use. Meetings and dictation remain peer tasks.
- First use chooses a real dictation or meeting task and preserves its goal/progress through relevant setup. Setup deferral remains available; success requires a real result.
- Finished meetings lead with their summary, then personal notes with an explicit editing action. Reading, sources and draft/conflict preservation take precedence over configuration.
- Recording, interruption, finishing and save failure have distinct truthful presentations. Stop stays reachable and dismissing the recording screen explains that capture continues.
- Sync has a guided setup path, explicit first-device/existing-library branches and actual first-sync completion. A paused device validates and reuses its retained key. Usage and destructive account administration are separate from setup.
- A connected device may approve a new device through QR or manual connection codes. Approval signs the new device in and grants access to the existing encrypted library. The recovery code remains the no-other-device fallback. Connecting a device never resets the library or rotates its recovery code.
- Pairing is short-lived, versioned and one-use. Product interfaces receive only transient presentation codes and nonsecret status; the shared Rust core owns cryptographic state, credentials and vault keys. Server capability, protocol review, deployed two-device and signed-device evidence remain explicit acceptance gates.
- Keyboard typing stays reachable during unavailable-session guidance. State-specific actions, retained-result previews and recovery retain document/lifetime/focus guards and once-only insertion. Keyboard-only cold start remains an unresolved release requirement until supported-device evidence satisfies #39.
- App haptics default on; keyboard typing haptics are independently optional and default off. Feedback follows real acknowledged actions/results, never polling or background sync. Native controls keep their existing feedback. Held delete does not cause continuous buzzing.

## Implementation and verification

Work is coordinated on `codex/mobile-sync-keyboard-ux` and the Cloud companion branch. Core owns behavior, native code owns platform services/extensions, Expo owns mobile UI and Tauri owns desktop UI. Generated native projects are not source ownership locations.

Required validation includes core and desktop regressions, mobile type/controller/helper tests, clean and repeated non-clean Prebuild, native runtime/keyboard tests, simulator and device Release builds, and app-owned screenshots. Network tests use isolated fixtures and fake mail/providers. No production account, content, credentials or deployment is needed for implementation verification.

Physical acceptance covers QR and manual linking in both phone/computer directions, Full Access transitions, large text/VoiceOver, recording and microphone/haptic interaction, interruption/recovery, and the exact signed candidate. A passing simulator, scripted service or compiler check does not establish those outcomes.

Exact completed commands, results and unresolved gates will be recorded with the final implementation receipt in `docs/mobile.md`.

## Review findings addressed

Two independent engineering reviews checked the implementation. Provisioning now keeps credentials private until server activation, journals recoverable state in the OS credential store, verifies writes, serializes cancellation and rechecks identity while holding mutation locks. Recovery acknowledgment binds the exact displayed code to its account, library generation and wrapping revision. Practice uses a fresh native session/result identity, so unrelated capture cannot be adopted or stopped by onboarding.

The Cloud admission flag stops new connections while existing admitted requests can complete or cancel. This prevents a rollout change from stranding an acknowledged or partially provisioned device. These checks are engineering evidence, not an independent cryptographic audit.

The native QR renderer includes an opaque four-module quiet zone. A hosted test decodes the actual generated PNG and verifies its border in dark appearance. Camera scanning, physical typing/gestures, perceived haptic strength and VoiceOver comprehension still require separate device acceptance.
