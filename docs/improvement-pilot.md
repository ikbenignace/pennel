# Daily-use improvement pilot

The first pilot is for 3–5 Mac users over one working week, with dictation and meetings weighted equally. Windows and Linux retain the same shared interface and enter the pilot only after their device acceptance passes. No telemetry or automatic reports are added.

The [usability and adoption study kit](ux-adoption.md) adds two separate formative rounds of five nontechnical professionals, with a revision between rounds, followed by a 15–20-person/four-week voluntary cohort. It includes neutral tasks, blank observation sheets and invitation drafts. Those stages do not replace this exact-candidate device pilot: formative task success, real audio/receiving-app behavior and repeat use are different evidence. No human sessions or cohort results are implied by preparing the kit.

## Candidate and evidence

Use the isolated **Pennel Preview** application, not canonical settings, credentials or meeting folders. Record the candidate commit, macOS version, CPU, input/output devices, selected models/providers and artifact SHA-256. Never put dictated text, meeting content, API keys or recovery codes into the checklist or diagnostic logs.

Build with `make desktop-preview`. `scripts/verify-desktop-bundle.py` checks the preview identity and signature without launching it; the preview image helper verifies its disk image. A valid development signature is not production notarization or release certification. The retired native source remains in Git; preserve actual previously installed packages and recovery data when testing upgrades. Retirement does not close the existing release gates.

Automated evidence uses owned temporary databases, synthetic audio/HTTP responses and loopback-only UI fixtures. A fixture screenshot or injected persistence fault is not proof of a real microphone, receiving application, Cloud service, installer or update.

## Required Mac scenarios

| Workflow | Exercise | Pass condition |
| --- | --- | --- |
| First use | Start with isolated profiles for both dictation-first and meeting-first tasks; use the recommended local setup, relevant access and real capture | A real dictation result or usable meeting transcript appears; denied/revoked access has a useful repair action; optional voice enrollment does not block the task |
| Repeat use and setup | Start a second meeting, try the safe in-app dictation rehearsal, then configure missing summaries from an existing meeting | Primary actions stay discoverable; rehearsal preserves first-use progress; setup returns to the same meeting with personal notes, selected view and reading position intact |
| Global dictation | Use an unsent Mail draft, browser text field and chat draft, with the Apple shortcut and an external-keyboard shortcut | Text reaches the intended field exactly once; ordinary clipboard contents are preserved |
| Destination safety | Change the target while processing, close an owned pad/practice session and try a slow receiving application | No replacement recipient or late owned delivery; the correct completed result can be copied from History |
| Recovery | Make a preview provider unavailable, then repair it and retry; inspect the automated storage-failure fixture separately | Transcription retry and save-only retry are distinct; no automatic recovery paste; explicit discard is required for unsaved text |
| Processing clarity | Select local speech with remote cleanup, use Snel, then change the text provider | Audio and text destinations match the selected mode; task tests send only their fixed example and invalidate after changes |
| Downloads | Open Settings while onboarding downloads a model, cancel, then revisit both surfaces | One worker owns the model; current progress is visible; a stale view never restarts a cancelled download |
| Settings continuity | From a meeting's Transcript or an active recording, open Settings, change a category and return; repeat from the menu/tray and shortcut | Settings stays inside the main window, reopening retains its category, and returning restores the same task, draft and reading position without stopping or starting capture |
| Workspace continuity | Prepare text in the pad, inspect an earlier dictation, return to Meetings and then continue the pad; repeat during an active meeting | Dictation, Meetings, History and Settings reuse main. Saved pad text and meeting state survive; a listening/processing pad requires an explicit finish or cancel rather than being silently abandoned. Exact History links select the intended result |
| Meetings | Record a real 60-minute test call with microphone and system audio, using headphones and then a speaker test | Both tracks and transcript align; errors/gaps are visible; files and notes remain usable after stop |
| Interruptions | Switch AirPods/USB devices, revoke access, navigate away and close/reopen windows during recording | Capture health is truthful; a fatal error stops safely; notes/title are retained; other windows remain unaffected |
| Slow/offline recognition | Use a deliberately unavailable preview endpoint during recording | Audio continues to be saved; recognition pauses after repeated failures; finalization does not repeatedly request every queued chunk |
| Meeting review | Follow transcript, summary and personal-note sources, return to an answer, regenerate and export | Correct source and conversation position return; changed sources are reported; personal notes survive |
| Keyboard/accessibility | Use keyboard-only tabs, dialogs, menus and recovery actions; inspect with VoiceOver and normal-window content enlarged to 200%. On Mac use `Cmd+=` (zoom in / plus) and `Cmd+-`; on Windows/Linux use `Ctrl+=` and `Ctrl+-`, without Shift. Pill/prompt windows stay fixed | Names, focus, announcements and actions remain usable; no focus theft, clipping or hidden critical controls. Record actual app/device results rather than inferring acceptance from enabled zoom shortcuts |
| Quit/update | Quit with drafts and retained recovery data; validate upgrade/restore and signed updates with accepted candidates | Saving completes or Pennel stays open; no retained session is silently replaced; existing data/provider relationships survive |

Do not fill a real disk or damage a user's database to manufacture storage failures. The regression suites inject those faults into owned test stores; the synthetic recovery surfaces let users review the corresponding actions.

## Measurements and admission

Collect a manual checklist with success/failure, duration and device/provider configuration. Compare shortcut response, stop-to-result time, pending work and memory on the same hardware and model; distinguish provider time from local capture/delivery overhead. During the hour-long call, check that pending audio does not grow without a bound in RAM and that the transcript/archive/recovery files agree afterwards.

Admission requires every relevant scenario to pass with the exact candidate. Silent data loss, wrong-target insertion, an unreported capture failure or an unresolved P0 defect blocks the pilot. Record synthetic, packaged-app and real-device evidence separately. Billing/runner restrictions on CI remain an external verification gap rather than a successful check.

## Implemented behavior and boundaries

- Temporary History recovery is process-local: at most twenty reserved/retained results and a five-MiB soft budget. An oversized completed result is preserved intact and blocks more capture. Quit/update retries saving; a crash cannot preserve an unsaved memory-only result.
- Meetings persist WAV chunks before recognition and commit transcript data before cleaning recovery audio. Exact-millisecond recovery names and optional millisecond transcript stamps prevent collisions; older whole-second recordings remain readable. No database migration or new dependency is required.
- A meeting freezes its speech settings and credentials. Cloud identity changes are rejected while it is active; same-account token refresh remains available. Optional summaries/questions use the provider selected when requested.
- Recovery, provider explanations and reading/accessibility changes live in the shared Tauri interface. The frozen native UI receives only the compatibility safeguard preventing automatic insertion of a result that could not be saved.
- Public distribution, production signing/notarization, Windows/Linux device acceptance and a completed human pilot remain separate from implementation and local regression checks.
