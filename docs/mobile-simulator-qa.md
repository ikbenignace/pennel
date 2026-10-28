# Expo simulator UI verification

Status: scoped Expo Device Hub simulator checks passed, including the repaired citation flow. Device/Cloud/store acceptance is separate.

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

## Tab/settings candidate — 28 October 2026, superseded below

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

The tab/settings pass resolves both new intermediate findings above. These checks establish simulator interface behavior; they do not establish real model quality or physical iPhone acceptance.

## Controlled loopback follow-up — 28 October 2026

The same installed executable was exercised through the Device Hub dashboard using a temporary key-free LM Studio provider, `127.0.0.1:61253/v1`, and scripted model `pennel-qa`. The responder bound loopback only, accepted generated test content, and logged operation counters without request bodies. No Cloud service, real provider credential, microphone or enrollment was used.

| Scenario | Observed result |
| --- | --- |
| Provider configuration | Added LM Studio through Providers and saved the loopback URL through its actual field/Save controls |
| Model catalog | Native model retrieval returned the scripted `pennel-qa` model, which was selected through the model row |
| Sample verification | `Test text processing` performed native HTTP inference and displayed `Test passed` and the sample-success explanation |
| Summary regeneration | `Regenerate` saved and rendered the scripted summary; notes and transcript stayed byte-identical |
| Ask | A generated question returned the scripted answer and a numbered citation to the synthetic meeting's transcript |
| Citation navigation | Immediately opening that fresh citation displayed `This source has changed or is no longer available` and remained on Notes despite an unchanged transcript; reported to the product owner for repair |

The responder handled exactly four requests: one model catalog GET, one sample POST, one summary POST and one Ask POST. These checks establish local UI/native/HTTP/JSON plumbing, not inference quality.

Recovery was verified: restored Apple Intelligence/system through Models, removed only the temporary LM Studio provider through its UI confirmation, and confirmed settings bytes match the pre-test backup. With the app stopped, restored the original synthetic `meeting.md` byte-for-byte; notes and transcript also match their pre-test bytes. Restarted the preview and stopped the task-owned responder. Recoverable copies remain under ignored `build/mobile/qa/loopback-backup/`.

Evidence captures under ignored `build/mobile/qa/device-hub/`: `loopback-model-tested.png`, `loopback-summary-regenerated.png`, `loopback-ask-answer.png` and `loopback-citation-opened.png`.

The optional direct `/exec-ws` action route was unsuitable in upstream Device Hub 0.18.1: `actions.describe` succeeded, but `tap` dispatched the Hub CLI, which rejected the positional command. Dashboard pointer and explicit Clipboard Send controls worked. No direct-action helper was retained. Browser Meta+A also left the device shortcut routing active, so the native Select All menu and Hub's explicit synthetic-text field were used instead; the app remained alive.

## Final repaired candidate — 28 October 2026

The initial citation failure above is superseded by the repaired candidate. Executable hashes alone cannot identify a JavaScript-only repair, so the final installed application was checked against every file in the built product:

| Receipt | Installed value |
| --- | --- |
| Executable SHA-256 | `2679216fe539e88a22a1f62fe71ae07dab91f32cbee844750e55b7518e340bd4` |
| `main.jsbundle` SHA-256 | `e7d4bd5e46d958e81ab9c98704036223075914a3f0013b438bb6c85513ee020f` |
| Full file-manifest SHA-256 | `24bf247eba07b00a0afa90d8a806448fba2704c47271109de0f8bcf15cbd6b72` |
| Bundle measurement | 69,742,620 bytes, 214 files; every installed relative filename and file digest matches the built simulator product |

The manifest hashes sorted UTF-8 relative filenames, each followed by NUL and that file's raw 32-byte SHA-256 digest, without newlines. The installed receipt is retained under ignored `build/mobile/qa/final-citation-installed-receipt.json`.

The first repair was checked through actual Ask results for both summary and transcript sources. The final candidate additionally used a controlled transcript with an `Ik` segment containing 596 `x` characters and an `Anderen 12` segment containing `A fresh second track.`, both at 00:00. The Ask result cited the second transcript passage; opening it selected Transcript, showed the fresh second track under 0:00 / Chris (voice 12), and displayed no changed-source warning. This exercises the same-time track ambiguity through the real Rust index and interface, rather than only a constructed DTO test.

The focused rechecks made three further Ask requests to a separate loopback-only responder, bringing the controlled follow-up total to seven requests across both responder sessions. Neither session logged request bodies. Both task-owned responders were stopped afterwards.

Final recovery was verified after relaunch: original providers/model selections and all other settings were restored, while preserving the native bootstrap's valid `meetings_dir` rebasing to the current preview container. Original summary, notes and transcript bytes match their pre-test backups. The temporary provider is absent, and the preview was left at Settings in English/dark appearance with no active capture. No production container or credential was changed.

Final evidence under ignored `build/mobile/qa/device-hub/`:

- `final-citation-native-tabs-en-dark.png`
- `final-accepted-settings-native-tabs-en-dark.png`
- `citation-fixed-summary-opened.png`
- `citation-fixed-transcript-opened.png`
- `equal-time-ask-answer.png`
- `equal-time-citation-opened.png`

## Compatibility rebuild smoke — 28 October 2026

After the native recognition task reference was made explicit for Swift 6.2 compatibility, the rebuilt simulator application was installed and launched on the same assigned iOS 26.5 preview simulator. This is a separate smoke receipt; the full loopback/citation acceptance above remains tied to its original artifact.

| Receipt | Installed value |
| --- | --- |
| Executable SHA-256 | `d93567d23051e619d7516b2307573437994f40f7ada0a6ab924cc8942c108b22` |
| `main.jsbundle` SHA-256 | `e7d4bd5e46d958e81ab9c98704036223075914a3f0013b438bb6c85513ee020f`, byte-identical to the accepted citation candidate |
| Full file-manifest SHA-256 | `134c0136adbd4a2e4d35c64ae4ede2cbfd0265503bafd42aaffbab4f55f4b79a` |
| Bundle measurement | 69,742,619 bytes, 214 files; all installed filenames and digests match the built product |

Launch and native Settings/Liquid Glass tabs rendered successfully in English/dark appearance. Before/after comparison confirms all four original providers, model selections and other settings survived; the native `meetings_dir` path resolves to the current preview container. Synthetic summary, notes, transcript and AAC archive hashes all match their pre-install values. No inference, temporary provider, account or microphone operation was added during this smoke.

The canonical Device Hub screenshot helper saved `build/mobile/qa/device-hub/compatibility-native-tabs-en-dark.png`. The installed receipt is under ignored `build/mobile/qa/compatibility-installed-receipt.json`. Local installation and launch do not by themselves establish a successful CI run with its separate Xcode toolchain.

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

- Real provider/model quality and non-loopback inference were not exercised by the scripted responder.
- Real iPhone keyboard cold start, Full Access transitions, memory/latency/battery, locked recording/interruption, Dutch meeting quality, deployed Cloud behavior, signing and App Review remain unverified. No simulator or scripted-provider outcome completes issues #18, #20, #38 or #39.
