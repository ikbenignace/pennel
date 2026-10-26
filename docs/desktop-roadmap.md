# Desktop roadmap

Current work is focused on validating Pennel on real desktops and improving installation and updates. The shared application is implemented; the checks below establish how it behaves with actual devices, receiving applications and release packages.

Use the [Pennel roadmap](https://github.com/orgs/pennel-app/projects/1) to track work across the desktop app, iPhone, Cloud and website. The ten desktop acceptance tasks below retain their completion criteria and source context. New tasks start in **Backlog**, with no assignee or due date. An organisation owner still needs to [make the board public](https://github.com/pennel-app/pennel/issues/25); access may require membership until then.

Historical native Mac limitations in [open-items.md](open-items.md) are regression context. Each task below is acceptance or distribution work, rather than a claim that an old bug still exists. Cloud, Teams and iPhone work belongs on the same board, with implementation issues in their owning repository.

| Desktop task | GitHub issue |
| --- | --- |
| 1. Mac dictation, commands and clipboard delivery | [#4](https://github.com/pennel-app/pennel/issues/4) |
| 2. Mac meeting audio and device switching | [#5](https://github.com/pennel-app/pennel/issues/5) |
| 3. Meeting review, speakers, citations and export | [#6](https://github.com/pennel-app/pennel/issues/6) |
| 4. Windows 11 installers and device workflows | [#7](https://github.com/pennel-app/pennel/issues/7) |
| 5. Linux X11 packages and desktop integration | [#8](https://github.com/pennel-app/pennel/issues/8) |
| 6. Linux Wayland shortcuts and recording | [#9](https://github.com/pennel-app/pennel/issues/9) |
| 7. Production signing and installer trust | [#10](https://github.com/pennel-app/pennel/issues/10) |
| 8. Upgrades, recovery and signed updates | [#11](https://github.com/pennel-app/pennel/issues/11) |
| 9. Sixty-minute recording and performance | [#12](https://github.com/pennel-app/pennel/issues/12) |
| 10. Mac user pilot and accessibility | [#13](https://github.com/pennel-app/pennel/issues/13) |

## Evidence for acceptance tasks

Record the candidate commit, package SHA-256, OS version, relevant hardware and selected models/providers. Keep automated checks, owned UI fixtures and real-device observations distinct. Record pass/fail and useful diagnostics without meeting content, dictated text, API keys or recovery codes. Open a separate bug with a minimal reproduction when a check fails.

## 1. Validate Mac dictation, commands, permissions and clipboard delivery

Confirm first use and everyday dictation in real receiving applications, using an isolated preview profile.

Done when:

- [ ] Local-model onboarding completes a real practice dictation; denied and revoked permissions have usable recovery actions.
- [ ] Hold-to-talk and hands-free dictation work with an Apple keyboard and the configured external-keyboard shortcut.
- [ ] Mail, a browser, a chat application and Terminal receive text exactly once; clipboard contents are preserved where supported.
- [ ] Commands with and without a selection behave correctly in each receiving application.
- [ ] Changing the destination during processing or closing the target retains the completed dictation in History without inserting it elsewhere.
- [ ] Cancelling an owned pad/practice session during finalization produces no late result, History entry or external insertion.

Sources: [pilot scenarios](improvement-pilot.md#required-mac-scenarios), [dictation destinations](desktop.md#dictation-destinations-and-live-text), [historical T5](open-items.md).

## 2. Validate Mac meeting audio, echo handling and device switching

Exercise the native audio paths with real calls and hardware.

Done when:

- [ ] Teams, Zoom and a browser call retain both microphone and computer-audio tracks, with aligned transcripts and usable archives.
- [ ] Headphone and speaker tests record whether echo processing avoids duplicate call audio.
- [ ] AirPods and USB-headset switches, permission changes and a device interruption produce truthful capture state and usable recovery.
- [ ] Safari/WebKit and helper-process variants have recorded detection results; manual recording remains available where classification fails.
- [ ] Dictation immediately after stopping a meeting works, and window closing/reopening retains the recording and drafts correctly.

Sources: [pilot scenarios](improvement-pilot.md#required-mac-scenarios), [audio and errors](desktop.md#audio-errors-and-privacy), [historical T1–T3 and T6](open-items.md).

## 3. Validate meeting review, speaker naming, cited answers and export

Complete the review workflow after a real meeting, using the chosen local or user-configured providers.

Done when:

- [ ] Speaker recognition, naming and renaming preserve transcript attribution after reopening the meeting.
- [ ] Summaries, action items and templates work alongside personal notes without overwriting those notes.
- [ ] Questions open the correct transcript, summary or note source and return to the original conversation position.
- [ ] Changed or ambiguous source passages have a useful explanation.
- [ ] Copy and export are checked in Notion, Slack and Mail; record the formats and platform limitations actually observed.

Sources: [model setup and source navigation](desktop.md#model-setup-and-verification), [speaker recognition](desktop.md#speaker-recognition), [historical T4](open-items.md).

## 4. Validate Windows 11 installers and real-device workflows

Run the packaged shared application on Windows 11 x64. Build and cross-target checks are supporting evidence, not device acceptance.

Done when:

- [ ] Both `.exe` and `.msi` packages install and launch; microphone permissions and missing-access explanations are checked.
- [ ] Right Ctrl gestures, commands, foreground checks and clipboard preservation work in representative receiving applications.
- [ ] Elevated-app input restrictions produce a useful recovery path instead of a false success.
- [ ] Real Teams/Zoom/browser calls retain both audio tracks; available endpoint echo effects and device switches have recorded results.
- [ ] Archives play back, speaker processing completes and overlays behave correctly across the tested displays.

Sources: [platform behavior](desktop.md#platform-behavior), [hotkeys and clipboard](desktop.md#hotkeys-and-clipboard), [remaining migration acceptance](shared-app-migration.md#acceptance-before-native-replacement-or-publication).

## 5. Validate Linux X11 packages, audio and desktop integration

Exercise the installed Debian package and AppImage on an actual X11 desktop.

Done when:

- [ ] Both package formats launch with the documented WebKitGTK, codec and helper dependencies.
- [ ] Global gestures, selection reads, guarded paste and supported clipboard restoration work in representative applications.
- [ ] PulseAudio or PipeWire call observations, microphone and monitor capture work during a real call and device switch.
- [ ] New archives and the Mac AAC compatibility fixture decode and play correctly; missing codecs/helpers have clear explanations.
- [ ] Recording prompts, the dictation pill and multi-monitor placement preserve focus; headset guidance matches the absence of Linux echo processing.

Sources: [Linux runtime prerequisites](linux-packaging.md#runtime-prerequisites), [Linux desktop acceptance](linux-packaging.md#desktop-acceptance), [platform behavior](desktop.md#platform-behavior).

## 6. Validate Linux Wayland shortcuts, recording and clipboard delivery

Exercise the installed Debian package and AppImage on a native Wayland desktop.

Done when:

- [ ] Configured compositor shortcuts invoke `pennel --toggle-dictation` and `pennel --toggle-meeting`, including an AppImage executable path.
- [ ] Dictation starts/stops with the shortcut, copies the result and supports manual Ctrl+V delivery.
- [ ] Selection, automatic paste and foreground-style limitations remain visible and accurate.
- [ ] Manual browser-meeting recording, native call-app observations, audio archives and device changes have recorded results.
- [ ] Missing helpers/codecs and unavailable audio connections have useful explanations; prompts and the pill are checked with the compositor's placement rules.

Sources: [Wayland behavior](desktop.md#linux-x11-and-wayland), [Linux desktop acceptance](linux-packaging.md#desktop-acceptance).

## 7. Establish production signing and installer trust

Move beyond the current self-signed Mac and unsigned Windows installers using the project's existing release tooling and securely configured credentials.

Done when:

- [ ] Mac candidates use Developer ID signing and pass signature, notarization and stapling checks.
- [ ] A clean Mac installation passes Gatekeeper checks, and subsequent updates retain the expected credential and permission identities.
- [ ] Windows installers have verified Authenticode signatures; record actual Windows installation and SmartScreen behavior without promising reputation-based warnings will disappear immediately.
- [ ] Release credentials remain outside the repository, with no certificate or key generated or replaced as an incidental build step.
- [ ] The accepted installer hashes, signature results and installation instructions are recorded for the candidate.

Sources: [release process](releasing.md), [verification and release state](desktop.md#verification-and-release-state), [candidate artifacts](linux-packaging.md#candidate-and-update-artifacts).

## 8. Validate upgrades, recovery and signed updates

Preserve existing installations while proving the complete update and recovery paths.

Done when:

- [ ] Existing provider/model choices, credential relationships, history, custom library paths and meeting files survive the upgrade.
- [ ] Concurrent old/new applications, interrupted migration and rollback to the retained snapshot are tested safely.
- [ ] The native Mac Sparkle-to-Tauri transition and subsequent shared-app signed updates work with actual prior installations.
- [ ] Windows updates and Linux AppImage updates use the exact accepted artifacts; Linux X11 and Wayland have separate acceptance evidence.
- [ ] Debian instructions use verified packages or package-manager updates, and the update manifest is prepared only after its required gates pass.

Sources: [data and upgrades](desktop.md#data-preview-and-upgrades), [migration acceptance](shared-app-migration.md#acceptance-before-native-replacement-or-publication), [update artifact gates](linux-packaging.md#candidate-and-update-artifacts).

## 9. Measure a sixty-minute recording and desktop performance

Collect comparable measurements from the release candidate rather than generic framework estimates.

Done when:

- [ ] A real sixty-minute call retains aligned microphone/computer audio, transcript, notes and usable final archives.
- [ ] Pending audio remains bounded in memory, with recovery files and final transcript agreeing after finalization.
- [ ] Startup, shortcut response, stop-to-result latency, idle CPU and memory include all application processes.
- [ ] Model-loaded and model-unloaded comparisons use the same hardware and configuration, with provider latency identified separately.
- [ ] Package download size and installed size are measured from the exact artifacts; published figures cite that configuration and candidate.

Sources: [pilot measurements](improvement-pilot.md#measurements-and-admission), [migration performance acceptance](shared-app-migration.md#acceptance-before-native-replacement-or-publication).

## 10. Complete the Mac user pilot and accessibility acceptance

Run the documented one-week pilot with 3–5 Mac users after the relevant candidate gates pass.

Done when:

- [ ] Dictation and meetings receive equal coverage using the isolated Pennel Preview profile and recorded candidate configuration.
- [ ] Keyboard-only tabs, dialogs, menus, source navigation and recovery actions have usable names and focus behavior.
- [ ] VoiceOver, enlarged text and tested multi-monitor configurations retain critical controls and announcements.
- [ ] Quit/update with drafts or recovery data preserves the user's work or keeps the application open with an explanation.
- [ ] Outcomes and blocking bugs are recorded without private content; admission follows the pilot's pass conditions.

Sources: [pilot checklist](improvement-pilot.md), [packaged interface acceptance](shared-app-migration.md#acceptance-before-native-replacement-or-publication).

## Using the GitHub roadmap

[Pennel roadmap](https://github.com/orgs/pennel-app/projects/1) is the shared tracking project for `pennel-app/pennel` and `pennel-app/pennel-cloud`. Keep issues in their owning repository. Private Cloud issues retain their repository access restrictions, including after the board becomes public.

Move work through **Backlog → Ready → In progress → In review → Done**. Use these fields to keep the full roadmap readable:

| Field | Values | Meaning |
| --- | --- | --- |
| Area | Desktop, Core, iPhone, Cloud, Website, Infrastructure | The part of Pennel the work affects. |
| Horizon | Launch, Next, Later | Release readiness, planned follow-up, or longer-term work; not a promised delivery date. |
| Priority | P1, P2, P3 | Release or enabling work, planned improvements, or exploration. |

Use **Board** for current status, **Launch readiness** for Launch tasks, and **Full roadmap** for work grouped by Horizon and sorted by Priority. Leave new issues unassigned and without deadlines until someone takes responsibility for them.

The automation adds open `pennel` issues with `is:issue is:open`, places new items in **Backlog**, and moves closed issues and merged pull requests to **Done**. Add Cloud issues manually from the issue's Projects selector or the board's add-item control. Add existing app issues manually when needed: auto-add does not backfill them. Moving a card must not close its issue, and completed work remains visible instead of being automatically archived.

Before creating a task, check for an existing issue or pull request. Describe the current state, a concrete completion checklist, source links, dependencies and the evidence needed to close it. Implemented code, automated checks, deployed behavior and real-device acceptance are separate evidence. The acceptance recording rules above apply to every device or release task.

An organisation owner must change the board to public and verify signed-out access to public work. Keep the [public visibility task](https://github.com/pennel-app/pennel/issues/25) open until the change is verified; private Cloud details must remain restricted. Verify automatic Backlog and Done placement with a real completed setup task.
