# Fase 3a — lokale engine + modellencatalogus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace whisper-rs with transcribe-cpp, ship Handy's benchmarked model catalog with speed/accuracy/language info per model, and give cloud providers a searchable model picker.

**Architecture:** Rust core owns the catalog (bundled `catalog.json`), scoring, filtering, sorting and remote model listing; Swift only renders. `LocalEngine` wraps one loaded `transcribe_cpp::Session`, released after idle like today.

**Tech Stack:** Rust 2024, transcribe-cpp 0.4.1 (feature `metal`), UniFFI 0.32, SwiftUI (macOS 14.4), mockito.

**Spec:** `docs/superpowers/specs/2026-10-24-phase3-design.md` (section "3a in detail")

## Global Constraints

- License: transcribe-cpp and `@handy-computer/models` are MIT; keep their copyright in `THIRD-PARTY-LICENSES.md`.
- Catalog version pinned: `@handy-computer/models` 0.2.2, schema 2.
- Speed score: `100·(1 − e^(−rtf/8))`, rtf = `performance.accelerated` else `cpu`; missing → 10. Rounded integer 0–100.
- Accuracy score: `100·e^(−error/15)` from `results[id].accuracy[lang].error`; missing → `None`. Rounded integer.
- Recommended (in this order): `parakeet-tdt-0.6b-v3`, `whisper-large-v3-turbo`, `cohere-transcribe-03-2026`, `canary-1b-v2`, `whisper-small`.
- Legacy ids `whisper-large-v3-turbo-q5_0` and `whisper-small-q5_1` keep their old URL/sha/size/`.bin` filename and keep working; they borrow scores from `whisper-large-v3-turbo` / `whisper-small`.
- No network needed to list local models. No telemetry.
- UI copy in Dutch; no comments restating code.
- App bundle < 15 MB, idle memory < 50 MB (measure, report).

## Review Focus

1. Settings pointing at a legacy id after upgrade → dictation still works, model shows as downloaded and "In gebruik". (Task 2 test `legacy_ids_stay_downloaded`.)
2. Language setting `None` (auto) → accuracy uses `"nl"`. (Task 2 test `auto_language_scores_dutch`.)
3. Model that doesn't support the chosen language while `supported_only` = false → still listed, accuracy `None`, sorted after measured ones. (Task 2 test `unsupported_language_sorts_last`.)
4. Local transcription with a language the model can't take (e.g. `nl` on English-only Parakeet v2) → falls back to autodetect instead of erroring. (Task 1 test via ignored real-model test + unit test of `run_options`.)
5. Provider `/models` response without OpenRouter extras (Groq, Ollama) → ids still listed, price `None`. (Task 3 test `plain_openai_models_list`.)

---

### Task 1: Swap whisper-rs for transcribe-cpp

**Files:**
- Modify: `core/Cargo.toml` (remove `whisper-rs`, add `transcribe-cpp = { version = "0.4.1", default-features = false, features = ["metal"] }`)
- Create: `core/src/local_engine.rs` (replaces `core/src/local_stt.rs`, delete that file)
- Modify: `core/src/lib.rs`, `core/src/engine.rs` (field `local: LocalEngine`), `apps/macos/Package.swift` linker settings if needed
- Create: `THIRD-PARTY-LICENSES.md`

**Interfaces:**
- Produces: `LocalEngine::default()`, `transcribe(&self, model_path: &Path, samples: &[f32], language: Option<&str>, vocabulary: &[String]) -> Result<String, MusliError>`, `unload_if_idle(&self, idle: Duration, now: Instant) -> bool`. Engine passes `settings.dictionary` as vocabulary (no longer a joined prompt for local).
- Produces: `fn run_options(language: Option<&str>, model_languages: &[String], vocabulary: &[String], supports_vocabulary: bool, supports_context: bool) -> transcribe_cpp::RunOptions` — language set only if the model lists it (or lists none); vocabulary into `vocabulary` when supported, else joined into `prompt` when `ContextPrompt` supported, else dropped.

- [ ] **Step 1:** Port the existing tests from `local_stt.rs` (`unloaded_model_is_not_released_again`, `missing_model_file_is_model_not_downloaded`, ignored `real_small_model_transcribes` using legacy `whisper-small-q5_1.bin`) plus new `run_options_drops_unsupported_language` (`run_options(Some("nl"), &["en".into()], &[], false, false).language == None`; with `&["nl".into(),"en".into()]` → `Some("nl")`; with `&[]` → `Some("nl")`) and `run_options_routes_vocabulary` (supports_vocabulary → `vocabulary == ["Musli"]`, `prompt == None`; only context → `prompt == Some("Musli")`; neither → both empty).
- [ ] **Step 2:** `cargo test --lib local_engine` → fails (module missing).
- [ ] **Step 3:** Implement. Load with `Model::load`, read `capabilities()?.languages` and `supports(Feature::Vocabulary|ContextPrompt)` once at load, keep `Session` + those facts in the `Loaded` struct. Map errors to `MusliError::InvalidResponse { message }`.
- [ ] **Step 4:** `cargo test --lib` all green; `cargo test --lib local_engine -- --ignored` transcribes `hello.wav` with the legacy small `.bin` (prints timing).
- [ ] **Step 5:** `make core && make app`; record `du -sh build/Musli.app`. Commit `feat(core): run local models through transcribe-cpp`.

### Task 2: Bundled catalog with scores, filtering and legacy entries

**Files:**
- Create: `core/src/catalog.json` (verbatim from npm `@handy-computer/models@0.2.2`, `catalog.json`), `core/src/catalog.rs`, `scripts/update-catalog.sh` (downloads the tarball for a given version via `npm pack`, copies `catalog.json`)
- Modify: `core/src/models.rs` (catalog source, `final_path` by entry filename extension, `LocalModel` record), `core/src/engine.rs` (`local_models` export moves here or stays free fn)

**Interfaces:**
- Consumes: `ModelStore` download code unchanged.
- Produces: `ModelEntry` gains `filename: String` (final file is `models/{id}.{ext}` where ext = `gguf` or `bin` from filename) — existing `{id}.bin` legacy files resolve unchanged.
- Produces (UniFFI record): `LocalModel { id, name, description, size_bytes: u64, parameters: Option<String>, speed: u8, accuracy: Option<u8>, languages: Vec<String>, recommended: bool, legacy: bool }`.
- Produces (UniFFI export): `local_models(language: Option<String>, supported_only: bool) -> Vec<LocalModel>`; sort: recommended (in pinned order) → others by accuracy desc (None last) → speed desc. Legacy entries included only when downloaded — so this becomes `MusliEngine::local_models(&self, language: Option<String>, supported_only: bool)`; keep free fn removed.

- [ ] **Step 1: Write failing tests in `catalog.rs`:**
  - `parses_bundled_catalog`: ≥ 20 asr models; `parakeet-tdt-0.6b-v3` has url `https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/85ac09ea12fc4b1112fa76810059364bc6adc9de/parakeet-tdt-0.6b-v3-Q8_0.gguf`, size 739508576, sha `5859f779…cc7`, languages contain `"nl"`.
  - `speed_formula`: rtf 25.065 → 96; none → 10.
  - `accuracy_formula`: parakeet v3 `nl` error 7.66 → 60; `en` 4.83 → 72; language not measured → None.
  - `auto_language_scores_dutch`: `language: None` gives same accuracy as `Some("nl")`.
  - `recommended_first_in_pinned_order`: first five ids equal the pinned list.
  - `supported_only_filters_language`: with `Some("nl")`, true → no model lacking `nl` (e.g. `parakeet-tdt-0.6b-v2` absent); false → present.
  - `unsupported_language_sorts_last`: with false, every `accuracy == None` model comes after every `Some` among non-recommended.
  - `non_asr_and_nested_files_excluded`: no `silero-vad-v6.2`, no file with `/`.
  - In `models.rs`: `legacy_ids_stay_downloaded` (create `models/whisper-small-q5_1.bin` in tempdir → `downloaded()` contains it, `local_models` lists it with `legacy == true`, accuracy equals `whisper-small`'s); `gguf_final_path` (`path("parakeet-tdt-0.6b-v3")` resolves `models/parakeet-tdt-0.6b-v3.gguf`).
- [ ] **Step 2:** Run → fail.
- [ ] **Step 3:** Implement with serde structs mirroring only needed fields (see Handy's `catalog/mod.rs` shape: `models[].{id,name,description,repository,revision,languages,quant,role,parameters,files{quant:{filename,size,sha256}}}`, `results{id:{accuracy{lang:{error}},performance{accelerated,cpu}}}`). Parse once (`OnceLock`). Empty description → generated Dutch line: `"{n} talen"` or `"Alleen {taal}"`.
- [ ] **Step 4:** `cargo test --lib` green.
- [ ] **Step 5:** Commit `feat(core): bundle benchmarked model catalog`.

### Task 3: Remote model listing in core

**Files:**
- Create: `core/src/remote_models.rs`
- Modify: `core/src/engine.rs` or free UniFFI export; remove Swift `ConnectionTest.modelIds` use in Task 5.

**Interfaces:**
- Produces: `RemoteModel { id: String, name: Option<String>, context_length: Option<u32>, prompt_price_per_million: Option<f64>, completion_price_per_million: Option<f64>, audio_input: bool, is_router: bool }`.
- Produces: `#[uniffi::export] fn provider_models(base_url: String, api_key: Option<String>) -> Result<Vec<RemoteModel>, MusliError>` — GET `{base}/models`, 10 s timeout, sorted by id. OpenRouter prices are per token strings → ×1 000 000. `audio_input` from `architecture.input_modalities` containing `"audio"`, or id containing `whisper`/`transcribe`/`voxtral`. `is_router` = id `openrouter/auto`, `openrouter/free` or ending in `:free` router ids starting `openrouter/`.

- [ ] **Step 1:** Tests with mockito: `openrouter_models_with_prices` (body with `pricing.prompt "0.0000001"` → 0.1, `context_length 1048576`, modalities `["text","audio"]` → audio_input), `plain_openai_models_list` (`{"data":[{"id":"whisper-large-v3-turbo"},{"id":"llama-3.3-70b-versatile"}]}` → prices None, first audio_input true), `flags_routers` (`openrouter/free` → is_router), `http_error_maps` (401 → `MusliError::Http{status:401,..}`).
- [ ] **Step 2–4:** fail → implement via `http::agent`/`http::with_auth` → pass.
- [ ] **Step 5:** Commit `feat(core): list provider models with prices`.

### Task 4: Local models screen

**Files:**
- Modify: `apps/macos/Sources/Musli/Windows/LocalModelsView.swift`, `apps/macos/Sources/Musli/ModelDownloads.swift` (models come from `engine.localModels(language:supportedOnly:)`), `apps/macos/Sources/Musli/Windows/ModelsPane.swift` (local picker uses names from the same list)
- Create: `apps/macos/Sources/Musli/Windows/ModelCard.swift`, `apps/macos/Sources/MusliKit/LanguageNames.swift` (+ test `LanguageNamesTests`)

**Interfaces:**
- Consumes: `LocalModel`, `MusliEngine.localModels(language:supportedOnly:)`.
- Produces: `LanguageNames.name(_ code: String) -> String` (Dutch name via `Locale(identifier: "nl")`, capitalized, falls back to the code); `LanguageNames.common: [String]` = `["nl","en","fr","de","es","it","pt","pl"]`.

- [ ] **Step 1:** `LanguageNamesTests`: `name("nl") == "Nederlands"`, `name("en") == "Engels"`, `name("xx") == "xx"`.
- [ ] **Step 2:** Implement, `swift test` green.
- [ ] **Step 3:** Screen: header row with Picker "Nauwkeurigheid voor" over `LanguageNames.common` (default `settings.language ?? "nl"`) and Toggle "Alleen modellen die deze taal ondersteunen" (default on). Sections "Aanbevolen" and "Alle modellen" (latter collapsed by default via DisclosureGroup). `ModelCard`: name + badge "In gebruik"/"Aanbevolen", description · size · parameters, two thin bars (label "Snelheid", "Nauwkeurigheid" — `nil` shows "niet gemeten"), "N talen" disclosure listing names joined by ", ", actions as today. Bars use `Theme.accent` on `Theme.line`.
- [ ] **Step 4:** Delete of a model in use shows `confirmationDialog` "Dit model is in gebruik. Verwijderen?".
- [ ] **Step 5:** `make app`, open Settings → Providers → Lokaal and check list renders; commit `feat(macos): model catalog with speed, accuracy and languages`.

### Task 5: Searchable cloud model picker

**Files:**
- Create: `apps/macos/Sources/Musli/Windows/RemoteModelPicker.swift`
- Modify: `apps/macos/Sources/Musli/Windows/ModelsPane.swift` (replace `Menu("Kies")`), `ProvidersPane.swift` (drop `modelIds` if unused)

**Interfaces:**
- Consumes: `providerModels(baseUrl:apiKey:)`, `RemoteModel`.

- [ ] **Step 1:** Popover with search field, list filtered by task (STT: `audioInput`; LLM: `!audioInput`), rows show id, context ("1M ctx"), price "$0,10 / $0,40 per 1M". Selecting sets `choice.model`.
- [ ] **Step 2:** When `choice.model` is a router (`openrouter/free`, `openrouter/auto`) show under the field in `Theme.accent`: "Kiest per aanvraag een willekeurig model — opkuisen wordt onbetrouwbaar."
- [ ] **Step 3:** `make app`; commit `feat(macos): searchable provider model picker`.

### Task 6: Verify, measure, document

- [ ] `make test` all green; ignored real tests: download `parakeet-tdt-0.6b-v3` via `ModelStore` in an ignored test `real_parakeet_transcribes` (asserts text contains "test" for `hello.wav`, prints timing).
- [ ] Measure app size + idle RSS after launch (`ps -o rss`), put numbers in the README "Lokale modellen" section with a line about the catalog and `scripts/update-catalog.sh`.
- [ ] Commit `docs: local model catalog`; push branch `phase-3a-models`.
