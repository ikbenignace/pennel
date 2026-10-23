# Fase 2b — Calls herkennen, regels, agenda, echo en dicteren tijdens meetings

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Musli merkt zelf dat je in een call zit en vraagt "Opnemen?" (of neemt automatisch op / negeert volgens je regels per app, agenda en meeting), stopt vanzelf als de call eindigt, haalt titel en deelnemers uit je agenda, onderdrukt echo als je via speakers belt, houdt gedicteerde stukken uit het meetingtranscript en heeft een globale sneltoets ⌘⇧R.

**Architecture:** Beslislogica (welke regel wint) en transcript-opkuis (echo-ontdubbeling, dictaat-uitsluiting) zitten getest in de Rust-kern. De call-detectie is een pure state machine in `MusliKit` (getest), gevoed door een Core Audio-monitor die elke 2 s leest welke processen de microfoon gebruiken. Agenda via EventKit. Bij een herkende call tapt Musli enkel de audio van die app.

**Spec:** `docs/superpowers/specs/2026-10-23-musli-design.md` §7 (call-detectie, einde, regels, toestemmingszin, echo, AirPods, dicteren tijdens meeting), §12 (echo-risico, agenda-risico).

## Global Constraints

- Opname via detectie start enkel bij regel **Altijd** of na **Ja** in de melding. Standaard voor elke app: **Vragen**.
- Regelvolgorde: meeting-titelregel > agendaregel > app-regel > standaard (Vragen).
- Call-einde: de call-app gebruikt de microfoon > 30 s niet meer → automatisch stoppen (enkel voor opnames die door detectie startten).
- Browsers (`com.google.Chrome`, `com.apple.Safari`, `company.thebrowser.Browser`, `com.microsoft.edgemac`, `org.mozilla.firefox`, `com.brave.Browser`): enkel als call tellen als een venstertitel `Meet`, `Teams`, `Zoom`, `Whereby` of `Webex` bevat (via Accessibility).
- Bij een herkende call: tap enkel op de processen van die app (`CATapDescription(stereoMixdownOfProcesses:)`); handmatig blijft globaal.
- Agenda: EventKit, enkel als de gebruiker het aanzet (`calendar_enabled`, standaard uit). Lopend event = niet hele dag, `start - 5 min ≤ nu ≤ end`. Titel → meetingtitel; deelnemers → frontmatter `attendees: [naam, …]`.
- Echo: microfoon met `setVoiceProcessingEnabled(true)` (instelling `echo_cancellation`, standaard aan) + ontdubbelen: een "Ik"-segment vervalt als een "Anderen"-segment binnen ±5 s ≥ 80 % dezelfde woorden heeft (≥ 80 % van de eigen woorden komt voor bij de anderen én de lengtes verschillen hoogstens 40 %).
- Dicteren tijdens een meeting: "Ik"-segmenten die voor > 50 % binnen een dictaatperiode vallen, vervallen.
- Globale sneltoets ⌘⇧R (start/stop opname), instelling `record_shortcut_enabled` (standaard aan), wordt ingeslikt zodat de app vooraan hem niet krijgt.
- Toestemmingszin (instelling `consent_enabled`, standaard uit): bij start wordt `Ik neem notities met transcript via Musli.` naar het klembord gekopieerd en meldt het pilletje "Toestemmingszin gekopieerd — plak in de chat".
- Standaard bekende call-apps: Teams `com.microsoft.teams2`, Zoom `us.zoom.xos`, Slack `com.tinyspeck.slackmacgap`, FaceTime `com.apple.FaceTime`, WhatsApp `net.whatsapp.WhatsApp`, Webex `Cisco-Systems.Spark`, Discord `com.hnc.Discord` + de browsers.

## Review Focus

1. **Kort micro-gebruik** (dicteren in een andere app, spraakmemo) → geen "Opnemen?"-melding; enkel na ≥ 5 s doorlopend gebruik door een bekende call-app of browser-call. → Task 3 test.
2. **Even dempen tijdens een call** (micro kort los) → opname loopt door; pas na 30 s stoppen. → Task 3 test.
3. **Musli zelf gebruikt de micro** (dicteren, opname) → telt nooit als call. → Task 3 test.
4. **Regel Nooit voor app, maar agenda zegt Altijd** → agenda wint; meeting-titelregel wint van beide. → Task 1 test.
5. **Speakers zonder koptelefoon** → wat de anderen zeggen staat niet dubbel onder "Ik". → Task 2 test.

---

### Task 1: Regels en beslissing in de kern

**Files:** Create `core/src/rules.rs`; Modify `core/src/settings.rs`, `core/src/lib.rs`.

**Interfaces — Produces:**
- `#[derive(uniffi::Enum, Serialize, Deserialize)] enum RecordAction { Ask, Always, Never }`
- `#[derive(uniffi::Record)] struct AppRule { bundle_id: String, action: RecordAction }`, `CalendarRule { calendar_id: String, calendar_name: String, action }`, `EventRule { title: String, action }` (titel exact, hoofdletterongevoelig).
- Settings (alle `#[serde(default)]`): `app_rules: Vec<AppRule>` (standaard leeg = Vragen), `calendar_rules`, `event_rules`, `calendar_enabled: bool` (false), `echo_cancellation: bool` (true), `record_shortcut_enabled: bool` (true), `consent_enabled: bool` (false), `auto_detect: bool` (true).
- `fn decide_recording(settings: Settings, bundle_id: String, calendar_id: Option<String>, event_title: Option<String>) -> RecordAction` (uniffi export).
- `fn known_call_apps() -> Vec<String>` en `fn browser_apps() -> Vec<String>` (uniffi export, lijsten uit Global Constraints).

- [ ] **Step 1: Failing tests:** `default_is_ask`, `app_rule_applies`, `calendar_rule_beats_app_rule`, `event_rule_beats_calendar_and_app` (titel `"Standup"` vs `"standup"`), `old_settings_get_detection_defaults` (v2-JSON zonder de nieuwe velden → defaults, geen corrupt-backup).
- [ ] **Step 2–4:** FAIL → implement → `cargo test && cargo clippy --all-targets -- -D warnings` PASS.
- [ ] **Step 5:** Commit `feat(core): add recording rules for apps, calendars and meetings`

### Task 2: Echo-ontdubbeling en dictaat-uitsluiting

**Files:** Create `core/src/transcript_filter.rs`; Modify `core/src/session.rs`.

**Interfaces — Produces:**
- `fn filter_segments(segments: &[Segment], dictations: &[(u64, u64)]) -> Vec<Segment>`: (1) Me-segmenten met > 50 % overlap met een dictaatperiode vallen weg; (2) Me-segment valt weg als een Others-segment met |start-verschil| ≤ 5 000 ms een token-overlap ≥ 0,8 heeft (tokens = lowercase woorden zonder leestekens; overlap = gedeelde / tokens van het kortste segment; enkel als het kortste ≥ 3 tokens heeft).
- `MeetingSession::mark_dictation(start_ms: u64, end_ms: u64)` (uniffi): bewaart de periode; transcript en `transcript.md` tonen voortaan de gefilterde segmenten (ruwe segmenten blijven intern).

- [ ] **Step 1: Failing tests:** `echo_of_others_is_removed_from_me` (Others "we schuiven de release naar november" @10 s, Me "we schuiven de release naar november op" @11 s → enkel Others), `different_text_is_kept`, `far_apart_duplicates_are_kept` (20 s uit elkaar), `short_segments_are_not_deduped` ("ja" vs "ja"), `dictation_period_removes_me_segment` (Me 30–40 s, dictaat 29–41 s → weg; Others in dezelfde periode blijft), `session_applies_filters_to_transcript_md`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(core): drop echoed and dictated segments from meeting transcripts`

### Task 3: Call-detectie als state machine

**Files:** Create `apps/macos/Sources/MusliKit/CallDetector.swift`, `Tests/MusliKitTests/CallDetectorTests.swift`.

**Interfaces — Produces:**
- `public struct MicUser: Hashable, Sendable { bundleId: String; isCall: Bool }` (isCall = bekende call-app of browser met call-titel)
- `public enum CallEvent: Equatable, Sendable { case started(String), ended(String) }`
- `public struct CallDetector { init(ownBundleId: String, startAfter: TimeInterval = 5, endAfter: TimeInterval = 30); mutating func update(users: Set<MicUser>, at t: TimeInterval) -> [CallEvent]; private(set) var activeCall: String? }` — één call tegelijk; start pas als dezelfde call-app ≥ `startAfter` doorlopend de micro gebruikt; einde als de actieve app ≥ `endAfter` niet meer in `users` zit; eigen bundle-ID en niet-call-gebruikers worden genegeerd.

- [ ] **Step 1: Failing tests:** `short_use_does_not_start`, `sustained_call_starts_once`, `brief_mute_does_not_end`, `ends_after_thirty_seconds`, `own_app_is_ignored`, `non_call_apps_are_ignored`, `second_app_while_active_is_ignored`.
- [ ] **Step 2–4:** FAIL → implement → `swift test --filter CallDetector` PASS.
- [ ] **Step 5:** Commit `feat(macos): add call detector state machine`

### Task 4: Monitoren — microfoongebruik, browsertitels, agenda

**Files:** Create `Meetings/MicUsageMonitor.swift`, `Meetings/CalendarProvider.swift`; Modify `Resources/Info.plist` (`NSCalendarsFullAccessUsageDescription` = `Musli gebruikt je agenda om meetings een titel en deelnemers te geven.`).

**Interfaces — Produces:**
- `final class MicUsageMonitor` — `start(interval: 2, onUpdate: @MainActor (Set<MicUser>) -> Void)`, `stop()`; leest `kAudioHardwarePropertyProcessObjectList`, per proces `kAudioProcessPropertyIsRunningInput` + `kAudioProcessPropertyBundleID`; browsers krijgen `isCall` op basis van venstertitels via `AXUIElementCreateApplication(pid)` → `kAXWindowsAttribute` → `kAXTitleAttribute`.
- `static func processObjects(bundleId: String) -> [AudioObjectID]` (voor de app-specifieke tap).
- `@MainActor final class CalendarProvider` — `requestAccess() async -> Bool`, `currentEvent() -> CurrentEvent?` met `struct CurrentEvent { title: String; calendarId: String; calendarName: String; attendees: [String] }`, `calendars() -> [(id: String, name: String)]`.

- [ ] **Step 1:** `swift build` PASS.
- [ ] **Step 2 (gebruiker):** Teams- of Meet-call starten → log `mic users` toont de app met `isCall=true`.
- [ ] **Step 3:** Commit `feat(macos): monitor microphone use and read the current calendar event`

### Task 5: Melding "Opnemen?", automatisch starten/stoppen, app-tap

**Files:** Create `Meetings/RecordPromptPanel.swift`, `Meetings/CallCoordinator.swift`; Modify `Meetings/SystemAudioTap.swift` (`prepare(processes: [AudioObjectID]?)`: nil = globaal), `Meetings/MeetingRecorder.swift`, `Meetings/MeetingController.swift` (`start(title:, attendees:, appBundleId:, auto: Bool)`; `startedByDetection`), `core/src/meetings.rs` + engine (`start_meeting_with(title, attendees, keys)` schrijft `attendees: [..]` in frontmatter), `MusliApp.swift`.

- `CallCoordinator`: voedt `CallDetector` met `MicUsageMonitor`-updates (enkel als `auto_detect` aan staat); bij `.started(app)` → `CalendarProvider.currentEvent()` (als aan) → `decideRecording` → Ask: `RecordPromptPanel` (rechtsboven, papierstijl, "Opnemen? · <App> · <event>" met **Ja**, **Nee**, **Nooit voor <App>**; verdwijnt na 60 s = Nee) / Always: meteen starten / Never: niets. "Nooit voor <App>" voegt `AppRule{Never}` toe. Bij `.ended(app)` → stoppen als die opname door detectie startte. Tijdens een lopende handmatige opname: geen melding.
- Tap bij detectie: `MicUsageMonitor.processObjects(bundleId:)`; leeg → globaal.

- [ ] **Step 1:** `swift build` + `make test` PASS.
- [ ] **Step 2 (gebruiker):** call starten → na ~5 s melding; Ja → opname met titel uit agenda; call beëindigen → ~30 s later stopt de opname en volgt de samenvatting; "Nooit voor Teams" → volgende Teams-call geen melding.
- [ ] **Step 3:** Commit `feat(macos): prompt to record detected calls and stop when they end`

### Task 6: Echo, dicteren tijdens meeting, ⌘⇧R, instellingen

**Files:** Modify `Meetings/MeetingRecorder.swift` (voice processing aan/uit volgens instelling; `voiceProcessingOtherAudioDuckingConfiguration` met minimale ducking), `DictationController.swift` (bij start/submit tijdens opname → `session.markDictation` met tijden uit `MeetingRecorder.elapsedMs`), `HotkeyMonitor.swift` (tweede, actieve tap voor keyDown ⌘⇧R → callback + event inslikken), `Windows/SettingsView.swift` + nieuw `Windows/RecordingPane.swift` (tab **Opnemen**: automatisch herkennen aan/uit, lijst app-regels (bekende call-apps + toegevoegde, picker Vragen/Altijd/Nooit), agenda aan/uit + toestemming + per agenda een regel, meetingregels (titel + actie, toevoegen/verwijderen), echo-onderdrukking, sneltoets ⌘⇧R, toestemmingszin), `Meetings/MeetingController.swift` (toestemmingszin).

- [ ] **Step 1:** `make test && make app` PASS.
- [ ] **Step 2 (gebruiker):** call via speakers → transcript "Ik" bevat de anderen niet dubbel; tijdens opname dicteren in Slack → die zin staat niet in het meetingtranscript; ⌘⇧R in een andere app start/stopt de opname zonder dat de app iets doet.
- [ ] **Step 3:** Commit `feat(macos): echo cancellation, dictation during meetings, global record shortcut and recording settings`
