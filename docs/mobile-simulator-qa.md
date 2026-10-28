# Expo simulator UI verification

Status: scoped Expo Device Hub simulator checks completed; device/Cloud/store acceptance is separate.

## Candidate and scope

- Device: stock iPhone 17 simulator, iOS 26.5, `64C462E2-0904-4E38-98C0-F1F19690D2E4`.
- Bundle: `app.pennel.ios.preview`, version 0.1.0, build 1.
- Initial Release executable SHA-256: `1e7e71fa1b9efc1bb04e6f90cc2aa398b2413e22db8454005fb1198a7150f8c3`.
- Initial unsigned app size: 69,597,308 bytes. This is an installed simulator bundle, not an App Store download measurement.
- The preview app was absent before install. No existing user application data was removed.
- The app launched without a signing repair. Native settings and local meeting APIs were exercised through the real interface.
- No microphone capture, real account, email/login request, external inference, Cloud recognition upload, or voice enrollment was performed.

Initial UI interactions used the Argent app-flow workflow. Subsequent verification uses the explicitly requested Expo Device Hub 0.18.1, its live preview and Accessibility inspector, and the scoped HTTP screenshot helper. No further Argent input is needed for this pass.

## Initial candidate observations

| Scenario | Observed result |
| --- | --- |
| First launch | Introduction rendered with Figtree/Instrument Serif and canonical warm palette |
| Interface language | English → Dutch changed introduction and native-backed settings immediately |
| Skip setup | Reached the real empty Meetings tab without requesting microphone access |
| Four tabs | Meetings, Dictation, Ask and Settings displayed and navigated |
| Appearance | Light → dark applied immediately; language and appearance survived app termination/relaunch |
| Signed-out account | Device-name field and sign-in explanation displayed; sign-in was not invoked |
| Local dictionary | A synthetic `Test` term was saved through the UI and verified in native settings storage |
| Local meeting | A tests-only synthetic fixture was loaded from the isolated preview container |
| Notes | UI edit autosaved to `notes.md`; text remained after tab changes and meeting rename |
| Speakers | IDs 1, 5 and 12 parsed and rendered with names; renaming ID 5 updated metadata and transcript label |
| Naming/privacy | Name dialog explicitly distinguishes labels from remembered voices; no voice-profile directory was created |
| Playback | Transcript click and Stop RPCs completed for a silent AAC archive without errors; audible/device output was not asserted |
| Export | Native share presentation contained the synthetic title/summary; dismissed without choosing a destination |
| Rename | Meeting title/folder updated while stable UID, notes and audio were preserved |
| Missing citation source | A constructed expired-token deep link displayed “This source has changed or is no longer available.” |
| Recognition availability | Signed-out speaker recognition was unavailable with an Account sign-in explanation; no upload was attempted |

The fixture contains only generated text and sixty seconds of digital silence. `tests/e2e/seed-simulator-meeting.py` refuses non-preview containers and existing fixture folders; the app was stopped before seeding. The optional local LLM helper binds only loopback and logs operation counts, never request bodies. Its scripted responses would establish UI/network/citation plumbing, not model quality.

## Findings sent to owners

1. Settings and speaker row accessibility names included a decorative chevron glyph (for example `Providers, `). Rebuilt-candidate Hub inspection confirms clean Settings row names such as `Providers`, `Models` and `Appearance`.
2. Built-in template labels appeared in Dutch inside English meeting UI (`Algemeen`, `1-op-1`, `Klantgesprek`). Rebuilt-candidate Hub preview confirms `General`, `One-to-one`, `Client meeting` and `Standup` in English. Custom names remain literal by the tested ID-based mapping.

## Expo Device Hub pass — 28 October 2026

The intermediate installed Release executable SHA-256 is `02a14c2196f44a30a781fc00f14abf675ffea61fa119ad9939332f80260b8177`; its simulator bundle is 69,722,889 bytes. This pass precedes the native launch-order repair and subsequent QA fixes. Record the final candidate separately rather than treating these screenshots as its receipt.

| Scenario | Observed result |
| --- | --- |
| Preview upgrade | Existing synthetic meeting, renamed title, notes and settings survived the scoped simulator install |
| Native tabs | Four system tab items rendered with Liquid Glass on iOS 26.5; Meetings, Dictation, Ask and Settings all navigated successfully |
| Appearance | App light/dark preferences changed the content and native tab appearance; Hub device appearance remained unchanged |
| Interface language | English → Dutch → English changed content and visible tab labels immediately |
| Tab accessibility | Refreshed native AX retained the initial English tab names after live Dutch selection; reported for repair |
| Row accessibility | Settings controls have explicit clean names without decorative icon glyphs, including translated Dutch names |
| Templates | English labels verified through the real meeting view; existing synthetic notes and summary remained intact |
| Keyboard feasibility copy | Dictation explicitly describes app-started sessions as an interim workflow and cold start as unverified; no session started |
| Unconfigured model choice | Choosing `Not configured` for dictation STT returned an operation error and retained the prior Apple provider; reported for repair, no inference requested |

Saved Hub captures under ignored `build/mobile/qa/device-hub/`: `native-tabs-en-dark.png`, `native-tabs-en-light.png`, `native-tabs-nl-light.png`, `templates-en-fixed.png` and `model-none-rejected-nl.png`.

## Final simulator candidate — 28 October 2026

- Installed Release executable SHA-256: `2679216fe539e88a22a1f62fe71ae07dab91f32cbee844750e55b7518e340bd4`.
- Installed simulator bundle: 69,741,669 bytes; verified after installation from the assigned simulator's app container.
- Scoped install and two launches completed without a signing repair. The preview's synthetic meeting title and language/appearance preferences survived.
- All four native tabs navigated successfully on this candidate in dark appearance. Light and dark Liquid Glass rendering were verified through the actual Hub live preview.
- English → Dutch changed both visible tab labels and refreshed native accessibility labels. The previously stale English accessibility names now become `Dicteren`, `Vraag` and `Instellingen`.
- Selecting `Not configured` for dictation STT now succeeds, hides the model field, displays unconfigured readiness and disables the sample test. This choice survived app termination/relaunch. The original Apple speech choice was restored afterwards; no inference was invoked.
- The candidate was left in English/dark appearance with no capture or session active. No real account, external provider, Cloud recognition, enrollment, microphone permission or email action was invoked.

Final captures saved through the documented `npm run device:hub:screenshot -- --device ... --name ...` JavaScript helper:

- `build/mobile/qa/device-hub/final-native-tabs-en-dark.png`
- `build/mobile/qa/device-hub/final-native-tabs-nl-light.png`
- `build/mobile/qa/device-hub/final-model-none-nl.png`

The final pass resolves both new intermediate findings above. These checks establish simulator interface behavior; they do not establish real model quality or physical iPhone acceptance.

## Saved initial screenshots

All captures are app-owned simulator views of synthetic/empty content, under ignored `build/mobile/qa/`:

- `onboarding-en-light.png`
- `onboarding-nl-light.png`
- `meetings-empty-nl-light.png`
- `appearance-nl-dark.png`
- `appearance-en-dark.png`
- `transcript-en-dark.png`
- `share-sheet-en-dark.png`
- `source-missing-en-dark.png`

## Remaining verification

- Actual sample inference and controlled local summary/ask/citation paths were not exercised in this final simulator pass.
- Real iPhone keyboard cold start, Full Access transitions, memory/latency/battery, locked recording/interruption, Dutch meeting quality, deployed Cloud behavior, signing and App Review remain unverified. No simulator or scripted-provider outcome completes issues #18, #20, #38 or #39.
