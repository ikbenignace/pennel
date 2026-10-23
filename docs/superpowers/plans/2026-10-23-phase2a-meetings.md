# Fase 2a — Meetings opnemen en samenvatten: implementatieplan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Handmatig een meeting opnemen (microfoon + systeemaudio als twee sporen), live transcript, eigen notities tijdens de call, en na afloop aangevulde notities of een samenvatting met beslissingen en actiepunten — alles als Markdown in `~/Musli`.

**Architecture:** Swift neemt beide sporen op (AVAudioEngine voor de microfoon, Core Audio process tap + aggregate device voor systeemaudio), schrijft per spoor een `.m4a` en duwt 16 kHz mono samples naar een `MeetingSession` in de Rust-kern. De kern knipt per spoor op stiltes in stukken van 15–30 s, transcribeert ze op een worker-thread via de meeting-provider (cloud of lokaal), en houdt `transcript.md` bij. Na stoppen maakt de kern via de LLM-provider `meeting.md` (samenvatting of aangevulde notities) op basis van een sjabloon. De app heeft een Musli-hoofdvenster (zijbalk met meetings + zoeken) en een meetingvenster.

**Tech Stack:** bestaand; CoreAudio (`CATapDescription`, `AudioHardwareCreateProcessTap`, aggregate device), AVFoundation (`AVAudioFile` AAC), SwiftUI.

**Spec:** `docs/superpowers/specs/2026-10-23-musli-design.md` §7 (opnemen, tijdens/na de call, sjablonen, audio, lange opnames), §8 (opslag), §9 (UI). Fase 2b (call-detectie, regels, agenda, echo-onderdrukking, dicteren tijdens meeting) valt buiten dit plan.

## Global Constraints

- Alles uit eerdere plannen blijft gelden (geen telemetrie, providers zelf gekozen, sleutels in Keychain, Nederlandse UI, warm papier + Figtree, Instrument Serif voor titels).
- Opname start enkel na een expliciete actie van de gebruiker (knop of sneltoets ⌘⇧R in het menu). Tijdens opname is altijd een indicator zichtbaar (menubalk-icoon wordt een rode stip + meetingvenster toont "Opname loopt · mm:ss").
- Handmatig opnemen = alle systeemaudio (globale tap, Musli zelf uitgesloten).
- Opslag: één map per meeting in `meetings_dir` (standaard `~/Musli`), naam `YYYY-MM-DD HHmm <titel>` (titel ontdaan van `/:\` en ingekort tot 60 tekens). Bestanden: `meeting.md` (frontmatter + AI-notities), `notes.md` (eigen notities, ongewijzigd), `transcript.md`, `ik.m4a`, `anderen.m4a`. Markdown-bestanden zijn de bron; de app leest ze bij elke weergave opnieuw.
- `meeting.md`-formaat:
  ```
  ---
  title: <titel>
  start: <ISO 8601 met tijdzone>
  duration_min: <int>
  template: <sjabloon-id>
  ---

  ## Samenvatting
  …
  ## Beslissingen
  …
  ## Actiepunten
  - [ ] …
  ```
- `transcript.md`-regels: `[mm:ss] **Ik:** tekst` of `[mm:ss] **Anderen:** tekst` (uren als `[h:mm:ss]` boven 60 min), gesorteerd op starttijd.
- Stukken: minimum 15 s, maximum 30 s, knippen bij ≥ 0,6 s stilte (RMS van 100 ms-frames < 0,01); stukken zonder spraak (`has_speech == false`) worden overgeslagen maar de tijd loopt door.
- Waar de eigen notities voorkomen in de AI-notities (zelfde regel na normaliseren) toont de UI ze zwart, de rest grijs.
- Lange transcripten: boven 40 000 tekens eerst samenvatten per blok van 30 000 tekens, dan die deelsamenvattingen samenvoegen.

## Review Focus

1. **Provider faalt midden in een meeting** (wifi weg) → opname loopt door, mislukte stukken worden bewaard als WAV in `failed/` en zijn achteraf opnieuw te transcriberen; transcript toont waar een gat zit. → Task 3 test.
2. **App crasht of wordt gesloten tijdens een opname** → wat al getranscribeerd is staat op schijf (`transcript.md` wordt na elk stuk herschreven), de meeting verschijnt in de lijst. → Task 3 test.
3. **Geen notities getypt** → gewone samenvatting; **wel notities** → aangevulde notities waarin elke eigen regel letterlijk terugkomt. → Task 4 tests.
4. **Systeemaudio-toestemming geweigerd of niet beschikbaar** → opname gaat door met enkel de microfoon, met een duidelijke melding. → Task 5 (handmatig).
5. **Gebruiker bewerkt `meeting.md` in Obsidian** → app toont de bewerkte versie, overschrijft ze niet tenzij de gebruiker "Opnieuw genereren" kiest. → Task 1 test.

---

### Task 1: Meetingopslag, Markdown-formaat, lijst en zoeken

**Files:** Create `core/src/meetings.rs`; Modify `core/src/settings.rs` (`meetings_dir: String` — leeg = `~/Musli`, `meeting_audio_retention_days: u32` = 0 (altijd bewaren), `default_template: String` = `"algemeen"`; v2-bestanden zonder deze velden krijgen defaults via `#[serde(default)]`), `core/src/lib.rs`.

**Interfaces — Produces:**
- `#[derive(uniffi::Enum)] enum Speaker { Me, Others }`
- `#[derive(uniffi::Record)] struct Segment { start_ms: u64, end_ms: u64, speaker: Speaker, text: String, failed: bool }`
- `#[derive(uniffi::Record)] struct MeetingInfo { id: String, title: String, start_iso: String, duration_min: u32, has_summary: bool }` (`id` = mapnaam)
- `#[derive(uniffi::Record)] struct NoteLine { text: String, mine: bool }`
- `struct MeetingStore { dir: PathBuf }`: `create(title: &str, start: chrono::DateTime<Local>) -> Result<String, MusliError>` (maakt map, unieke naam met ` (2)` bij botsing), `dir_of(id)`, `write_notes(id, &str)`, `read_notes(id) -> String`, `write_transcript(id, &[Segment])`, `read_transcript(id) -> Vec<Segment>` (parset `transcript.md`; gat = `failed: true`, tekst `…`), `write_meeting(id, title, start_iso, duration_min, template, body_md)`, `read_meeting(id) -> Option<String>` (body zonder frontmatter), `list() -> Vec<MeetingInfo>` (nieuwste eerst; mappen met `transcript.md` of `meeting.md`), `search(query) -> Vec<MeetingInfo>` (hoofdletterongevoelig in titel, notes, transcript, meeting), `rename(id, title) -> Result<String>` (mapnaam + frontmatter), `delete(id)` (map naar Prullenmand via `trash` is Swift-zijde; kern verwijdert), `prune_audio(retention_days, now)` (verwijdert `*.m4a` ouder dan termijn; 0 = niets).
- `fn mark_lines(enhanced: &str, notes: &str) -> Vec<NoteLine>` (uniffi export): normaliseren = trim, lowercase, `- `/`* `/`[ ] ` voorvoegsels weg; lege regels `mine: false`.
- Dependency: `chrono` (features `clock`).

- [ ] **Step 1: Failing tests:** `create_uses_dated_folder_name` (`2026-10-23 1400 Sprint review`), `create_sanitizes_and_dedupes_title` (`a/b:c` → `abc`; tweede keer → ` (2)`), `transcript_roundtrips_with_speakers_and_gaps`, `transcript_hours_format` (start 3 723 000 ms → `[1:02:03]`), `meeting_md_has_frontmatter_and_body`, `read_meeting_returns_external_edit` (bestand handmatig wijzigen → `read_meeting` geeft nieuwe body), `list_newest_first_and_includes_in_progress`, `search_matches_transcript_text`, `prune_audio_keeps_markdown`, `mark_lines_matches_own_notes` (notes `"- export naar Excel"`, enhanced `"- Export naar Excel\n- Tom vond de oorzaak"` → `[mine, not mine]`), `v2_settings_without_meeting_fields_load_defaults`.
- [ ] **Step 2:** `cargo test meetings settings` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets -- -D warnings` → PASS.
- [ ] **Step 5:** Commit `feat(core): add meeting store with markdown files`

### Task 2: Stukken knippen op stiltes

**Files:** Create `core/src/chunker.rs`.

**Interfaces — Produces:** `struct Chunk { start_ms: u64, samples: Vec<f32> }`; `struct Chunker` met `new(sample_rate: u32) -> Self`, `push(&mut self, samples: &[f32]) -> Vec<Chunk>`, `flush(&mut self) -> Option<Chunk>` (rest ≥ 0,3 s), waarden uit Global Constraints.

- [ ] **Step 1: Failing tests** (16 kHz): `cuts_at_silence_after_minimum` (20 s toon + 1 s stilte + 5 s toon → eerste stuk eindigt tussen 20,0 en 21,0 s), `hard_cut_at_maximum` (40 s doorlopende toon → eerste stuk precies 30 s), `start_times_are_cumulative` (tweede stuk `start_ms` = lengte eerste), `silent_chunks_are_skipped_but_time_advances` (30 s stilte + 20 s toon → één stuk met `start_ms ≥ 30_000`), `flush_returns_remainder`, `push_in_small_buffers_equals_one_big_push`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(core): add silence-aware audio chunker`

### Task 3: Meetingsessie met achtergrondtranscriptie

**Files:** Create `core/src/session.rs`; Modify `core/src/engine.rs` (transcriptie van samples herbruikbaar maken: `fn transcribe_samples(&self, choice: &ModelChoice, samples: &[f32], keys: &ApiKeys) -> Result<String, MusliError>` — cloud via WAV, lokaal via whisper; dictionary als prompt; hallucinaties → lege tekst), `core/src/lib.rs`.

**Interfaces — Produces:**
- `MusliEngine::start_meeting(self: Arc<Self>, title: String, keys: ApiKeys) -> Result<Arc<MeetingSession>, MusliError>`
- `MusliEngine::meetings() -> Vec<MeetingInfo>`, `search_meetings(query) -> Vec<MeetingInfo>`, `meeting_notes(id) -> String`, `meeting_markdown(id) -> Option<String>`, `meeting_transcript(id) -> Vec<Segment>`, `meeting_dir(id) -> String`, `rename_meeting(id, title) -> Result<String>`, `delete_meeting(id)`, `retranscribe_failed(id, keys) -> Result<u32>` (aantal hersteld).
- `#[derive(uniffi::Object)] MeetingSession`: `id() -> String`, `dir() -> String`, `push_audio(speaker: Speaker, samples: Vec<f32>)` (16 kHz mono; niet-blokkerend), `set_notes(text: String) -> Result<()>` (schrijft `notes.md`), `transcript() -> Vec<Segment>`, `pending() -> u32` (stukken in wachtrij of bezig), `stop() -> Result<(), MusliError>` (flush beide chunkers, wacht tot worker klaar is, zet `duration_min`).
- Worker: één thread per sessie, `mpsc`-wachtrij; per stuk max. 2 pogingen; mislukt → segment `failed: true` + WAV in `<meeting>/failed/<speaker>-<start_ms>.wav`; na elk stuk `transcript.md` herschrijven.

- [ ] **Step 1: Failing tests** (mockito als meeting-provider, `meeting_stt` naar mock): `session_transcribes_both_speakers_in_order` (Me-toon 0–20 s, Others-toon 5–25 s → twee segmenten gesorteerd op start, `transcript.md` bevat `**Ik:**` en `**Anderen:**`), `transcript_is_written_after_each_chunk` (na eerste stuk en vóór `stop` bestaat `transcript.md` — simuleert crash), `failed_chunk_is_saved_and_retranscribed` (eerste respons 503 twee keer → `failed: true` + bestand in `failed/`; `retranscribe_failed` met 200 → 1 hersteld, segment niet meer failed), `stop_flushes_remainder`, `notes_are_written`, `meetings_lists_session`.
- [ ] **Step 2–4:** FAIL → implement → PASS (incl. clippy).
- [ ] **Step 5:** Commit `feat(core): add meeting sessions with background transcription`

### Task 4: Samenvatten en notities aanvullen met sjablonen

**Files:** Create `core/src/summary.rs`; Modify `core/src/engine.rs`, `core/src/lib.rs`.

**Interfaces — Produces:**
- `#[derive(uniffi::Record)] struct MeetingTemplate { id: String, name: String }`; `fn meeting_templates() -> Vec<MeetingTemplate>`: `algemeen` "Algemeen", `een-op-een` "1-op-1", `klantgesprek` "Klantgesprek", `standup` "Standup".
- `fn summary_messages(template_id: &str, notes: &str, transcript: &str) -> Vec<ChatMessage>`; `fn transcript_text(segments: &[Segment]) -> String` (`[mm:ss] Ik: …` per regel, failed overgeslagen).
- `MusliEngine::summarize_meeting(id: String, template_id: String, keys: ApiKeys) -> Result<String, MusliError>`: leest notes + transcript, map-reduce boven 40 000 tekens (blokken van 30 000, elk via `chunk_messages` naar bullets, dan samenvoegen), schrijft `meeting.md` (frontmatter met template) en geeft de body terug. Leeg transcript én lege notities → `Err(NoSpeech)`.

System-prompt (exact, `{TEMPLATE}` en `{MODE}` invullen):
```
You write meeting notes from a transcript. "Ik" is the user; "Anderen" are the other participants.
Write in the language the meeting was held in (Dutch or English). Use Markdown with exactly these sections:
## Samenvatting
## Beslissingen
## Actiepunten
Under Actiepunten use "- [ ] Person — action" lines; write "- Geen" when there are none. Never invent facts that are not in the transcript or notes.
{TEMPLATE}
{MODE}
```
- MODE zonder notities: `Summarize the meeting in short bullet points under Samenvatting.`
- MODE met notities: `The user took the notes below during the meeting. Under Samenvatting, keep every line of the user's notes verbatim (you may reorder them) and add missing important details from the transcript as separate bullet lines.`
- TEMPLATE: algemeen `Focus on what was discussed and agreed.`; een-op-een `This is a 1-on-1. Include feedback, wellbeing and personal follow-ups.`; klantgesprek `This is a client meeting. Capture the client's needs, questions, commitments made to them and next steps.`; standup `This is a standup. Per person: done, next, blockers.`
- User-bericht: `<notes>\n{notes}\n</notes>\n<transcript>\n{transcript}\n</transcript>` (notes-blok weggelaten als leeg).

- [ ] **Step 1: Failing tests:** `prompt_without_notes_summarizes`, `prompt_with_notes_keeps_lines_verbatim` (bevat `keep every line of the user's notes verbatim` en `<notes>`), `template_instructions_are_included` (klantgesprek), `transcript_text_labels_speakers_and_skips_failed`, `summarize_writes_meeting_md` (mock LLM → `meeting.md` met frontmatter `template: algemeen` en de body), `long_transcript_uses_map_reduce` (45 000 tekens → LLM-mock `.expect(3)`: 2 blokken + 1 samenvoeging), `empty_meeting_is_no_speech`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(core): add meeting summaries with templates`

### Task 5: Opnemen in de app — microfoon + systeemaudio

**Files:** Create `apps/macos/Sources/Musli/Meetings/SystemAudioTap.swift`, `Meetings/MeetingRecorder.swift`; Modify `apps/macos/Resources/Info.plist` (`NSAudioCaptureUsageDescription` = `Musli neemt de audio van je call op om er een transcript van te maken.`), `MusliKit/AudioInput.swift` (herbruikbare `Resampler`-logica blijft in app; geen wijziging nodig als die er niet is).

**Interfaces — Produces:**
- `final class SystemAudioTap` — `start(onSamples: @escaping @Sendable ([Float], Double) -> Void) throws` (globale stereo-tap die het eigen proces uitsluit, private aggregate device met de standaard-uitvoer als hoofd-subdevice, IOProc-blok op eigen queue; levert Float32-frames + sample rate), `stop()` (IOProc stoppen/vernietigen, aggregate device en tap vernietigen).
- `@MainActor final class MeetingRecorder` — `start(session: MeetingSession) throws`, `stop() async`, `elapsed: TimeInterval`, `systemAudioAvailable: Bool`. Microfoon via eigen `AVAudioEngine`; beide stromen → (a) `AVAudioFile` AAC 32 kbps mono 16 kHz (`ik.m4a`, `anderen.m4a` in `session.dir()`), (b) naar 16 kHz mono geconverteerd en elke ~1 s gebundeld naar `session.pushAudio(speaker:samples:)` op een achtergrondqueue. Faalt de tap → `systemAudioAvailable = false`, opname gaat door met microfoon.

- [ ] **Step 1:** `swift build` → PASS; `make app` → bundle bouwt.
- [ ] **Step 2 (gebruiker):** opname starten tijdens een YouTube-video + praten → na stoppen bestaan `ik.m4a` en `anderen.m4a`, beide afspeelbaar; transcript bevat beide sprekers.
- [ ] **Step 3 (gebruiker):** systeemaudio-toestemming weigeren → melding "Systeemaudio niet beschikbaar — enkel je microfoon wordt opgenomen", microfoon-transcript werkt.
- [ ] **Step 4:** Commit `feat(macos): record microphone and system audio for meetings`

### Task 6: Meetingvenster

**Files:** Create `Meetings/MeetingWindow.swift` (SwiftUI), `Meetings/MeetingController.swift`, `Meetings/AudioScrubber.swift`.

**Interfaces — Consumes:** `MeetingSession`, `MeetingRecorder`, engine-meeting-API, `markLines`, `meetingTemplates`.

UI (stijl mockup `visual-style-v2.html`):
- **Tijdens opname:** titelveld (Instrument Serif 30, standaard "Meeting <tijd>"), rode stip + "Opname loopt · mm:ss" (+ waarschuwing als systeemaudio ontbreekt), groot notitieveld (TextEditor, Figtree 14, autosave naar `set_notes` 1 s na typen), uitklapbaar "Live transcript" (standaard dicht; ververst elke 2 s via `transcript()`), knop **Stop opname**.
- **Na stoppen:** "Bezig met samenvatten…" (wacht op `pending() == 0` via `stop()`, dan `summarize_meeting` met gekozen sjabloon) → tabs **Notities | Transcript**. Notities: Markdown-weergave per regel via `markLines` (eigen regels `Theme.ink`, AI-regels `#8c857b`), checkboxes als ☐. Bovenaan: sjabloon-picker + **Opnieuw genereren**, **Toon in Finder**. Transcript: regels `[mm:ss] Ik/Anderen: tekst`, klik → `AudioScrubber` speelt het bijbehorende `.m4a` vanaf `start_ms`; mislukte stukken tonen "Transcriptie mislukt" + knop **Opnieuw proberen** (`retranscribe_failed`).
- Samenvatten mislukt → foutmelding + knop **Opnieuw proberen**; transcript blijft zichtbaar.

- [ ] **Step 1:** `swift build` → PASS.
- [ ] **Step 2 (gebruiker):** korte meeting zonder notities → samenvatting met drie secties; met notities → eigen regels zwart, aanvullingen grijs; klik op transcriptregel speelt audio.
- [ ] **Step 3:** Commit `feat(macos): add meeting window with notes, transcript and summary`

### Task 7: Musli-hoofdvenster, menubalk en meetinginstellingen

**Files:** Create `Meetings/LibraryWindow.swift`; Modify `MenuBarController.swift`, `MusliApp.swift`, `Windows/SettingsView.swift` (+ tab **Meetings**: map kiezen (`NSOpenPanel`, iCloud-waarschuwing als pad onder `~/Library/Mobile Documents` of `~/Documents`/`~/Desktop` met iCloud-sync), standaardsjabloon, audio bewaren 30/90/365 dagen/altijd), `AppState.swift`.

- **Hoofdvenster** (mockup): zijbalk met zoekveld + meetings gegroepeerd per Vandaag / Deze week / Eerder (titel + tijd · duur), rechts het meetingvenster-inhoud in "na stoppen"-toestand; rechtsklik: Hernoemen, Toon in Finder, Verwijderen (bevestiging).
- **Menubalk:** bovenaan lopende opname (titel + mm:ss) met **Stop opname ⌘⇧R**, anders **Start opname ⌘⇧R**; **Nieuwe notitie** = start opname + venster; sectie **Recent** (laatste 3 meetings); **Open Musli ⌘O**; bestaand: Geschiedenis, Instellingen, Stop. Icoon wordt rode stip tijdens opname.
- Bij opstarten: `prune_audio` volgens instelling.

- [ ] **Step 1:** `make test && make app` → PASS.
- [ ] **Step 2 (gebruiker):** meeting verschijnt in het hoofdvenster, zoeken op een woord uit het transcript vindt ze, hernoemen past de mapnaam aan.
- [ ] **Step 3:** Commit `feat(macos): add meeting library window and menu bar recording controls`
