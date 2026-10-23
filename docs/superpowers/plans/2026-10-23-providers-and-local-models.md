# Providers apart + lokale modellen: implementatieplan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Providers één keer instellen en per categorie (dicteren, opkuisen, meetings) een provider + model kiezen; Whisper-modellen in de app downloaden en lokaal draaien, zoals Handy.

**Architecture:** `Settings` krijgt een lijst `providers: Vec<ProviderAccount>` en per taak een `ModelChoice { provider_id, model }`; oude instellingen migreren automatisch. Nieuwe provider-soort `Local`: whisper.cpp via `whisper-rs` (Metal) in de Rust-kern, modellen gedownload naar `models/` met hervatten en sha256-controle, geladen bij eerste gebruik en vrijgegeven na inactiviteit.

**Tech Stack:** bestaand + `whisper-rs` 0.16 (feature `metal`), `sha2`; build vereist `cmake` (Homebrew).

**Spec:** `docs/superpowers/specs/2026-10-23-musli-design.md` §5 (providers zelf kiezen, lokaal als optie), §6, §9; aanvulling van de gebruiker op 23/10: providers apart instellen, lokaal model downloaden + activeren zoals Handy.

## Global Constraints

- Alles uit het fase 1-plan blijft gelden (macOS 14.4+, geen telemetrie, sleutels enkel in Keychain, UI in het Nederlands, stijl warm papier + Figtree).
- Bestaande `settings.json` (v1) wordt zonder verlies van sleutels of keuzes gemigreerd.
- Een API-sleutel is enkel verplicht voor OpenAI, OpenRouter, Groq en Mistral; Ollama, LM Studio, Eigen server en Lokaal werken zonder.
- Modellen worden nooit meegebundeld; app-groei door whisper.cpp ≤ 6 MB.
- Lokaal model wordt pas geladen bij het eerste dictaat en vrijgegeven na `local_unload_minutes` (standaard 5) zonder gebruik.
- Modelcatalogus (exact):

| id | naam | bytes | sha256 | url |
|---|---|---|---|---|
| `whisper-large-v3-turbo-q5_0` | Whisper large-v3 turbo | 574041195 | `394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2` | `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin` |
| `whisper-small-q5_1` | Whisper small | 190085487 | `ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb` | `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin` |

## Review Focus

1. **Migratie** van een v1-`settings.json` met een ingevulde Groq-sleutel → na update staat Groq als provider, dicteren/opkuisen wijzen ernaar, sleutel werkt nog. → Task 1 test.
2. **Download onderbroken** (app gestopt, wifi weg) → volgende download gaat verder vanaf het `.part`-bestand; een corrupte download wordt verwijderd en nooit als "gedownload" getoond. → Task 2 tests.
3. **Lokaal gekozen maar model niet (meer) aanwezig** → duidelijke melding "Model nog niet gedownload", geen crash, audio bewaard voor opnieuw proberen. → Task 3 test.
4. **Provider verwijderd die nog in gebruik is** door een categorie → categorie valt terug op de eerste provider die past, geen lege keuze. → Task 1 test.
5. **Geheugen**: model blijft niet eeuwig geladen; na inactiviteit vrijgegeven. → Task 3 test.

---

### Task 1: Instellingen v2 — providers apart, keuzes per taak, migratie

**Files:**
- Modify: `core/src/provider.rs`, `core/src/settings.rs`, `core/src/engine.rs`, `core/src/stt.rs`, `core/src/llm.rs`, `core/src/lib.rs`

**Interfaces:**
- Produces:
  - `ProviderKind` krijgt `Local`; `fn requires_key(kind: ProviderKind) -> bool` (uniffi export): true voor OpenAi, OpenRouter, Groq, Mistral.
  - `#[derive(uniffi::Record)] struct ProviderAccount { id: String, kind: ProviderKind, name: String, base_url: String }` — de Keychain-account is het `id`.
  - `#[derive(uniffi::Record)] struct ModelChoice { provider_id: String, model: String }`
  - `Settings { providers: Vec<ProviderAccount>, dictation_stt: ModelChoice, meeting_stt: ModelChoice, llm: ModelChoice, mode, language, dictionary, style_rules, history_retention_days, hotkey, local_unload_minutes: u32 }` + `version: u32 = 2` in JSON.
  - `fn new_account(kind: ProviderKind, existing: Vec<ProviderAccount>) -> ProviderAccount` (uniffi export): id = kind-slug (`groq`, `openai`, `openrouter`, `mistral`, `ollama`, `lmstudio`, `custom`, `local`), met `-2`, `-3`… als het al bestaat; naam = label; base_url uit preset.
  - `fn default_model(kind: ProviderKind, task: ProviderTask) -> String` (uniffi export, waarden uit de bestaande preset-tabel; Local + stt → `whisper-large-v3-turbo-q5_0`).
  - `Settings::normalized(self) -> Settings`: elke `ModelChoice` met onbekende `provider_id` wijst naar de eerste provider die de taak aankan (Local enkel voor stt) met diens `default_model`; zonder geschikte provider blijft hij ongewijzigd. Wordt toegepast in `load` en `update_settings`.
  - `ProviderConfig` blijft intern als opgeloste vorm `{ kind, base_url, model }` voor de HTTP-clients; `Settings::resolve(&self, choice: &ModelChoice) -> Option<(ProviderAccount, String)>`.
  - `ApiKeys` wordt `struct ApiKeys { by_provider: HashMap<String, String> }`.
  - Engine: `MissingApiKey { task }` enkel als `requires_key(kind)` en geen sleutel; zonder sleutel geen Authorization-header.
- Defaults: providers `[groq (Groq), local (Lokaal, base_url "")]`; dicteren + meetings `{groq, whisper-large-v3-turbo}`, opkuisen `{groq, llama-3.3-70b-versatile}`, `local_unload_minutes` 5.
- Migratie: JSON zonder `version` = v1 (`dictation_stt`/`meeting_stt`/`llm` als `ProviderConfig` met `keychain_account`). Per uniek `keychain_account` (leeg → kind-slug) één `ProviderAccount { id: keychain_account-of-slug, kind, name: label, base_url }`; keuzes wijzen naar die id met het oude model. `local` wordt toegevoegd als die ontbreekt.

- [ ] **Step 1: Write failing tests**
  - `migrates_v1_groq_settings`: v1-JSON (zoals fase 1 schreef, alle drie Groq, account `groq`, dictionary `["Musli"]`) → `providers` bevat `groq` + `local`; `dictation_stt == {groq, whisper-large-v3-turbo}`; `llm.provider_id == "groq"`; dictionary behouden.
  - `migrates_v1_mixed_providers`: stt Groq, llm OpenRouter → twee accounts `groq`, `openrouter`.
  - `removed_provider_falls_back`: verwijder `groq` uit providers, `normalized()` → dicteren wijst naar `local` met `whisper-large-v3-turbo-q5_0`; opkuisen blijft `groq` omdat er geen andere LLM-provider is.
  - `new_account_ids_are_unique`: twee keer Groq → `groq`, `groq-2`.
  - `requires_key_only_for_cloud`: Groq true, Custom/Ollama/LmStudio/Local false.
  - Engine: `custom_provider_without_key_sends_no_auth` (mock zonder Authorization), `cloud_provider_without_key_errors` (bestaande missing-key test aangepast).
  - Bestaande tests aanpassen aan de nieuwe vorm.
- [ ] **Step 2:** `cargo test` → FAIL (compile).
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** `cargo test && cargo clippy --all-targets -- -D warnings` → PASS.
- [ ] **Step 5: Commit** `feat(core): separate provider accounts from per-task model choices`

### Task 2: Modelcatalogus en downloads

**Files:**
- Create: `core/src/models.rs`; Modify: `core/src/engine.rs`, `core/src/lib.rs`, `core/Cargo.toml` (`sha2`)

**Interfaces:**
- Produces:
  - `#[derive(uniffi::Record)] struct LocalModel { id: String, name: String, description: String, size_bytes: u64 }` en `fn local_models() -> Vec<LocalModel>` (catalogus uit Global Constraints; beschrijvingen: turbo "Beste kwaliteit, ook in het Nederlands", small "Sneller en lichter, iets minder nauwkeurig").
  - `#[derive(uniffi::Enum)] enum ModelState { NotDownloaded, Partial { bytes: u64 }, Downloaded }`
  - `#[uniffi::export(with_foreign)] trait DownloadListener: Send + Sync { fn on_progress(&self, downloaded: u64, total: u64); }`
  - `struct ModelStore { dir: PathBuf }` (`data_dir/models`): `state(id) -> ModelState`, `path(id) -> Option<PathBuf>` (enkel als Downloaded), `download(id, base_url_override: Option<&str>, listener, cancel: &AtomicBool) -> Result<(), MusliError>`, `delete(id)`.
  - Download: naar `<id>.part`, `Range: bytes=<len>-` als `.part` bestaat (200 i.p.v. 206 → opnieuw vanaf 0), voortgang per ~1 MB, na afloop sha256 → match: rename naar `<id>.bin`; mismatch: `.part` verwijderen, `MusliError::InvalidResponse { message: "checksum mismatch" }`. Cancel → stopt, `.part` blijft (hervatbaar), `MusliError::Cancelled`.
  - `MusliError` krijgt `Cancelled`, `ModelNotDownloaded`.
  - Engine-exports: `local_models()`, `model_state(id) -> ModelState`, `download_model(id, listener) -> Result<(), MusliError>` (blokkerend), `cancel_download(id)`, `delete_model(id)`. Onbekend id → `NotFound`.
  - `base_url_override` bestaat enkel voor tests (mockito); de catalogus-URL is anders vast.

- [ ] **Step 1: Write failing tests** (mockito; testcatalogus met eigen sha van een kleine payload via een `#[cfg(test)]`-constructor `ModelStore::with_catalog`)
  - `download_verifies_and_marks_downloaded`: 3 MB payload → `Downloaded`, listener kreeg laatste `(total, total)`.
  - `resumes_from_partial`: schrijf eerste 1 MB als `.part`, mock verwacht header `range: bytes=1048576-` en antwoordt 206 met de rest → `Downloaded`, sha klopt.
  - `checksum_mismatch_removes_part`: → `Err(InvalidResponse)`, state `NotDownloaded`.
  - `cancel_keeps_partial`: cancel-vlag na eerste chunk → `Err(Cancelled)`, state `Partial`.
  - `delete_removes_model`.
  - `catalog_has_turbo_and_small` met exacte bytes.
- [ ] **Step 2:** `cargo test models` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** `cargo test` → PASS.
- [ ] **Step 5: Commit** `feat(core): add local model catalog and resumable downloads`

### Task 3: Lokale transcriptie met whisper.cpp

**Files:**
- Create: `core/src/local_stt.rs`; Modify: `core/src/engine.rs`, `core/src/wav.rs` (decoder), `core/Cargo.toml` (`whisper-rs` feature `metal`), `apps/macos/Package.swift` (linker: `Metal`, `MetalKit`, `Accelerate`, `Foundation`, `c++`), `scripts/build-core.sh` (check `cmake`), `README.md` (cmake vereiste)

**Interfaces:**
- Produces:
  - `fn decode_wav(bytes: &[u8]) -> Option<(Vec<f32>, u32)>` (enkel ons eigen 16-bit mono-formaat).
  - `struct LocalWhisper` met `transcribe(&self, model_path: &Path, samples: &[f32], language: Option<&str>, prompt: Option<&str>) -> Result<String, MusliError>` (laadt context als pad anders is of niet geladen; `language` None → `"auto"`; greedy, `n_threads = min(8, cores)`, geen timestamps/prints; segmenten samengevoegd en getrimd), `unload_if_idle(&self, idle: Duration, now: Instant) -> bool`, `is_loaded() -> bool`.
  - Engine: dicteerprovider `Local` → `ModelStore::path(model)` of `Err(ModelNotDownloaded)` (audio bewaard als mislukt); anders `local.transcribe`. Retry van een lokaal item decodeert de bewaarde WAV.
  - Engine-export `unload_idle_models()` — Swift roept die elke 60 s aan; gebruikt `local_unload_minutes`.
- Seam voor tests: `trait SpeechToText { fn transcribe(...) }` niet nodig; engine-tests gebruiken het pad "model ontbreekt" en een `#[ignore]`-test met het echte small-model.

- [ ] **Step 1: Write failing tests**
  - `decode_roundtrips_encode`.
  - `local_without_model_is_model_not_downloaded`: dicteren via `local` + model niet gedownload → `Err(ModelNotDownloaded)`, history 1 item Failed met audio.
  - `unload_if_idle_releases_after_timeout` (met een niet-geladen context: `unload_if_idle` → false; na laden via echte test → zie ignored test).
  - `#[ignore] real_small_model_transcribes_dutch`: verwacht `~/Library/Application Support/Musli/models/whisper-small-q5_1.bin` en `core/tests/fixtures/hallo.wav` (gegenereerd met `say -v Xander "Hallo, dit is een test" -o /tmp/h.aiff && afconvert -f WAVE -d LEI16@16000 -c 1 /tmp/h.aiff core/tests/fixtures/hallo.wav`) → tekst bevat "test" (case-insensitive); daarna `unload_if_idle(0s)` → true, `is_loaded()` false.
- [ ] **Step 2:** `cargo test` → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4:** `cargo test` → PASS; model downloaden via een kleine `cargo run --example`-vrije weg: de ignored test downloadt zelf niet — download eenmalig met `curl` naar het modelpad en draai `cargo test -- --ignored real_small_model` → PASS. Noteer de transcriptietijd.
- [ ] **Step 5:** `make core && swift build --package-path apps/macos` linkt (Metal/Accelerate/c++); `make app` → bundle < 12 MB.
- [ ] **Step 6: Commit** `feat(core): add local whisper.cpp transcription`

### Task 4: App — Providers-tab, Modellen-tab, lokale modellen

**Files:**
- Modify: `AppState.swift` (`keys()` → map voor alle providers met sleutel), `Windows/SettingsView.swift` (tabs: Providers, Modellen, Dicteren, Woordenlijst, Stijlen), `Windows/ProvidersPane.swift` (herschreven), `Windows/OnboardingView.swift` (providerstap: groen als dicteerprovider geen sleutel nodig heeft of er een heeft, of lokaal model gedownload), `MusliApp.swift` (timer 60 s → `unloadIdleModels()`), `DictationController.swift` (melding `ModelNotDownloaded` → "Model nog niet gedownload")
- Create: `Windows/ModelsPane.swift`, `Windows/LocalModelsView.swift`, `ModelDownloads.swift`

**Interfaces:**
- Consumes: `ProviderAccount`, `ModelChoice`, `newAccount`, `defaultModel`, `requiresKey`, `localModels`, `modelState`, `downloadModel`, `cancelDownload`, `deleteModel`, `unloadIdleModels`.
- Produces:
  - `@MainActor @Observable final class ModelDownloads` (in AppState): `progress: [String: Double]`, `states: [String: ModelState]`, `func start(_ id: String)`, `func cancel(_ id: String)`, `func delete(_ id: String)`, `func refresh()`; download op een eigen `DispatchQueue`, listener-klasse post voortgang naar main.

UI:
- **Providers**: lijst (naam, soort, groen bolletje als bruikbaar: sleutel aanwezig of niet nodig); knop "Provider toevoegen" (menu met soorten, Lokaal enkel als nog niet aanwezig); selecteren toont rechts/onder: naam, base URL (niet bij Lokaal), API-sleutel (enkel bij `requiresKey` of Custom), "Test verbinding", "Verwijderen". Bij **Lokaal**: `LocalModelsView` met per model naam, beschrijving, grootte (MB), knop Downloaden / voortgangsbalk + Annuleren / Gebruik + Verwijderen; "Gebruik" zet `dictationStt = {local, id}`; actief model krijgt label "In gebruik". Instelling "Model vrijgeven na" (1 / 5 / 15 / 60 min).
- **Modellen**: drie blokken (Dicteren, Opkuisen, Meetings): picker provider (enkel providers die de taak aankunnen; Lokaal niet bij Opkuisen), model: bij Lokaal een picker met gedownloade modellen (+ link "Download een model" naar Providers), anders tekstveld met suggestie `defaultModel`, en als `GET {base}/models` lukt een menu met de namen.

- [ ] **Step 1:** `make test` → alle Rust- en Swift-tests PASS; `make app` → bouwt.
- [ ] **Step 2 (gebruiker):** bestaande installatie openen → Groq-sleutel nog aanwezig, Providers toont Groq + Lokaal, Modellen toont Groq bij Dicteren en Opkuisen.
- [ ] **Step 3 (gebruiker):** Lokaal → Whisper small downloaden (voortgang zichtbaar), halverwege annuleren, opnieuw → gaat verder; "Gebruik" → dicteren werkt offline (wifi uit, stand Snel).
- [ ] **Step 4 (gebruiker):** na 5 min niet dicteren daalt het geheugen van Musli weer (Activiteitenweergave).
- [ ] **Step 5: Commit** `feat(macos): providers and models tabs with local model downloads`
