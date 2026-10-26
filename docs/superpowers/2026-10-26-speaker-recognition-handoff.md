# Speaker recognition v2 — handoff (2026-10-26)

Design: `docs/superpowers/specs/2026-10-26-speaker-recognition-v2-design.md`.

## Technology decision

sherpa-onnx (official `sherpa-onnx` 2.0.0 crate, static ONNX Runtime) running pyannote segmentation-3.0 + 3D-Speaker CAM++ zh-en "common advanced" + fast agglomerative clustering. Same recipe as OpenWhispr's local diarization. No Python, no PyTorch, no GPU.

| Option | Runtime | Download | Voiceprints | Licence | Verdict |
|---|---|---|---|---|---|
| pyannote.audio 3.1 | Python + PyTorch | ~33 MB models, GBs of runtime | yes | MIT (gated) | reference accuracy (DER 11.3% VoxConverse, 18.8% AMI), far too heavy to ship |
| WhisperX | Python + PyTorch + CTranslate2 | GBs | via pyannote | BSD-2 + pyannote | an ASR stack; its diarization is pyannote |
| Sortformer 4spk (today) | transcribe.cpp (linked) | 139 MB | no | NVIDIA open model | max 4 speakers, no enrollment or cross-meeting recognition |
| **sherpa-onnx + pyannote-seg + CAM++** | ONNX Runtime, static | **34 MB** | **yes** | Apache-2.0 / MIT / MIT / Apache-2.0 | **chosen** |

Numbers (Apple M2 Pro unless noted, window shift 0.25, 4 threads):
- 5-minute four-speaker file: 9.0 s (≈3% of real time), peak RSS 250 MB; 57 s file 1.2–2.0 s; 16 s file 0.5–0.6 s.
- Server (arm64 Docker, 4 vCPU): 10-minute chunk 20.5 s, ~365 MiB peak.
- Embedding model measured on public test speech + Dutch/English synthetic voices: CAM++ zh-en same speaker 0.67–0.94, different ≤ 0.46. The VoxCeleb-only exports (CAM++ and WeSpeaker ResNet34) did not separate the same clips and were rejected.
- Binary size: `libpennel_core.dylib` release 11.2 MB → 27.1 MB (+15.9 MB, static ONNX Runtime). Server image 125 MB → 184 MB (+34 MB models).

## What was built

**pennel (`feat/speaker-recognition`)**
- `core/speakers/` — new crate `pennel-speakers`, shared with the server: pure voiceprint maths, chunk merging, identification (always compiled); `diarize`/`embed` behind feature `onnx`.
- Core feature `speakers` (default) = `pennel-speakers/onnx`. Without it (iPhone, `--no-default-features`) the uniffi surface is identical; local recognition returns a Dutch error, cloud diarization + on-device matching still work.
- Core: model entries (`models.rs`), voice store (`voices.rs`: `<data dir>/voices/voiceprints.json`, meeting voiceprints by hashed meeting uid), transcript labelling for system or shared-microphone track (`diarize.rs`), Pennel Cloud client in 10-minute WAV chunks (`cloud/diarize.rs`), engine surface (`speakers.rs`): `speaker_model_ids`, `local_speaker_recognition_available`, `speaker_recognition_ready`, `voice_enrollment_text`, `enroll_voice`, `voice_profile`, `forget_voice_profile`, `known_voices`, `rename_known_voice`, `forget_known_voice`, `forget_all_voices`, `recognize_speakers(id, microphone, system)`, `meeting_voices`, `remember_speaker`; `diarize_meeting` kept for the macOS app. Setting `speaker_recognition: local|cloud`; onboarding step `voice`.
- Desktop: postprocess and manual rerun use `recognize_speakers` (system track decoded first, microphone only when needed); in-memory voice enrollment recording (max 60 s); Instellingen → Meetings (Op dit toestel / Via Pennel Cloud, 34 MB download, Je stem, Bekende stemmen, Vergeet alle stemmen); onboarding step "Leer je stem" (5 of 5, skippable); "Onthoud deze stem" switch when naming a voice. Visual cases added to `scripts/fixtures`; screenshots reviewed (light/dark, 14 cases).
- Windows: `.cargo/config.toml` builds all native code for the static C runtime (sherpa ships /MT only; mixing failed with LNK2038 in CI).
- Legacy macOS app: only a compile fix (`.voice` handled like practice).

**pennel-cloud (`feat/speaker-recognition-cloud`)**
- `POST /v1/audio/diarize`: bearer auth, `audio/wav` 16-bit mono 16 kHz, ≤ 630 s, ≤ 25 MB; monthly audio quota shared with transcription (seconds recorded on success), per-account limiter, global CPU pool (`PENNEL_DIARIZE_CONCURRENCY`, default 2) taken only after the upload, `spawn_blocking`, usage recorded even on disconnect. Response `{model, turns, speakers}`. Logs: account, seconds, speaker count, duration only. 503 `speakers_unavailable` without models.
- Docker: models downloaded in a stage with `ADD --checksum`, `PENNEL_SPEAKER_MODELS=/opt/pennel/models`, `libstdc++6` in runtime, `g++` in builder. `deploy/docker-stack.yml` server memory 512M → 1G.

## Branches, commits, CI

- pennel `feat/speaker-recognition`: 17 commits from `4b7fe8f` to `7278e77` (base `342dea5`). GitHub `desktop` workflow (macOS, Windows, Linux core + app jobs): green on `7278e77` (run 40247689002); the handoff commit after it only adds this file. The `ci` workflow only runs on pull requests; its steps (theme check, speaker crate fmt/clippy/tests, `make core`, both Swift test suites, `make app`) pass locally on macOS.
- pennel-cloud `feat/speaker-recognition-cloud`: 6 commits `7682380`..`6d8705f`; `ci` (dispatched on the branch): fmt, clippy, tests, web — green. Docker image built locally (arm64 only).

## Model downloads

| id | file | size | sha256 | licence |
|---|---|---|---|---|
| `pyannote-segmentation-3.0` | HF `csukuangfj/sherpa-onnx-pyannote-segmentation-3-0@9403a69…/model.onnx` | 5,992,913 | `220ad67c…1079` | MIT |
| `3dspeaker-campplus-zh-en-advanced` | HF `csukuangfj/speaker-embedding-models@0743f30…/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx` | 28,281,164 | `aa3cfc16…eba2` | Apache-2.0 |

The prebuilt sherpa-onnx libraries are downloaded by the `sherpa-onnx-sys` build script from GitHub releases at build time (cached in `target/sherpa-onnx-prebuilt`).

## Server env / Docker

- `PENNEL_SPEAKER_MODELS` (image default `/opt/pennel/models`), `PENNEL_DIARIZE_CONCURRENCY` (default 2; 0 fails at startup).
- Memory: two concurrent 10-minute chunks peak ≈ 750 MB → limit raised to 1G. Check the host has headroom for a rolling deploy (old + new task), or set concurrency 1 (~365 MB).

## Merge order

1. Merge pennel `feat/speaker-recognition` first (the cloud depends on its `core/speakers` crate).
2. Bump `pennel-speakers` in pennel-cloud from `rev = 44f390e…` (branch commit; may disappear after a squash merge) to the merged main commit, `cargo update -p pennel-speakers`, then merge and deploy pennel-cloud. The crate did not change after `4b7fe8f`, so behaviour is identical.
3. Release the desktop app after the server is deployed (cloud mode answers 404 until then; local mode works independently).
4. `feat/iphone` conflicts: `core/Cargo.toml` `[features]` (both add `default`; combine to `default = ["local-models", "speakers"]`) and `core/src/diarize.rs` (Sortformer `diarize` gated by `local-models`; the new `from_sortformer`/`apply` code is pure and stays ungated). The iPhone build should use `--no-default-features` (no sherpa) and cloud mode.

## Known gaps

- Not run on real hardware or real meetings: microphone enrollment, model download, and Dutch multi-person meetings are only covered by fixtures, unit/integration tests and synthetic/public clips. Thresholds (cluster 0.6, me 0.50, known 0.55, merge 0.55) need tuning on real recordings.
- Transcript lines are 30-second-ish chunks without word timings; a line gets its dominant speaker, so fast turn-taking inside one line is attributed to one person.
- Hybrid meetings (a call plus people in the room on the microphone): only the system track is split; room voices stay "Ik".
- Cloud upload is uncompressed WAV (an hour ≈ 115 MB in six requests); FLAC/Opus would halve or better.
- Older Pennel versions don't know voices 5–12; their "retry failed chunks" can drop such lines in a synced meeting (new versions refuse to rewrite unparsed lines).
- Windows paths with non-ASCII characters are passed to sherpa-onnx as UTF-8; untested whether ONNX Runtime opens them on Windows.
- Desktop maps core errors to Dutch text itself (core `InvalidResponse` displays with an English prefix); speaker model sizes are duplicated in TS for combined progress.
- Recording a voice is refused during a meeting or dictation.

## Judgment calls (cost if wrong)

1. **CAM++ zh-en instead of a VoxCeleb model** — measured best separation; if Dutch real-world voices behave differently, swap the model id/url (voiceprints of the old model are ignored automatically; users retrain).
2. **Windows static C runtime for the whole app** — CI green; if another native dependency ever needs /MD, Windows linking breaks again (alternative: sherpa `shared` + ship two DLLs).
3. **Microphone track only split when the user trained their voice and it is found** — no risk of losing "Ik"; costs recognition of in-person meetings for users who never train.
4. **Voices up to 12** (was 4) — more real speakers named; small compatibility risk above with old clients.
5. **Sortformer kept as fallback, no longer offered for download** — old installs keep working; removing it later is a small change.
6. **Known voice names override a name the user typed for that voice on rerun** — a corrected misrecognition comes back on rerun until the wrong known voice is forgotten.
7. **Cloud diarization counts against the transcription audio quota and the daily cost-cap estimate** (spec: count like transcription) — a meeting transcribed and diarized via cloud uses ≈2× its length; the cost cap may trip early with heavy diarization.
8. **Onboarding completes after the voice step** (practice no longer marks it completed) — closing on the voice step shows the wizard again / marks skipped.
9. **"Onthoud deze stem" is opt-in per name, default off** — fewer remembered voices, never a voiceprint the user did not ask for.
10. **Enrollment through Pennel Cloud uploads the 25 s reading** when the user chose cloud mode — consistent with that choice, documented in the UI copy.
11. **Server memory limit 1G** — needs host headroom; otherwise set concurrency 1.
