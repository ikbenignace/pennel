# Speaker recognition v2 — design

Date: 2026-10-26 · Status: built on `feat/speaker-recognition` (app) and `feat/speaker-recognition-cloud` (server)

## Goal

Recognise who speaks in a meeting, including the user's own trained voice:

- Train your voice once (onboarding, skippable; later in Instellingen → Meetings): read a Dutch text for ~25 s. Only a voiceprint is kept, on the device. Pennel then labels you "Ik", also when everyone shares one microphone.
- Remember other voices across meetings: naming "Anderen 2" as "Jan" with "Onthoud deze stem" stores Jan's voiceprint (opt-in per name), so Jan is named automatically next time.
- Works for the two-track desktop case (microphone = Ik, system audio = Anderen) and for one shared microphone (an in-person meeting, later the iPhone).
- "Sprekers herkennen: Op dit toestel / Via Pennel Cloud". Cloud uploads the track to the Pennel server, which runs the same pipeline in memory and returns turns and voiceprints; audio is not stored or logged. Matching against known voices always stays on the device.
- Lightweight: no Python, no PyTorch, no GPU.

## Technology decision

Requirements that decide it: a voiceprint per speaker (enrollment and cross-meeting recognition need embeddings, not only "who spoke when"), CPU only, embeddable in a Rust core that builds for macOS, Windows and Linux (and a Linux server image), permissive licences, small downloads.

| Option | What it is | Runtime | Download | Gives voiceprints | Licence | Verdict |
|---|---|---|---|---|---|---|
| pyannote.audio 3.1 | Python pipeline: segmentation-3.0 + WeSpeaker ResNet34 + clustering | Python + PyTorch (~1–2 GB installed) | models ~33 MB | yes | MIT (gated on Hugging Face) | Best reference accuracy (DER 11.3% VoxConverse, 18.8% AMI headset, 12.2% AISHELL-4, no collar), but bundling Python/PyTorch in a desktop app and an iPhone-backed server is the opposite of light. |
| WhisperX | faster-whisper ASR + wav2vec2 alignment + pyannote 3.1 diarization | Python + PyTorch + CTranslate2 | GBs | via pyannote | BSD-2 + pyannote's | A transcription stack; Pennel already transcribes. Diarization part is pyannote, with the same weight. |
| Sortformer 4spk v2.1 (today) | end-to-end diarizer GGUF via transcribe.cpp | already linked | 139 MB (Q8) | **no** | NVIDIA open model licence | At most 4 speakers, no embeddings, so no enrollment and no cross-meeting recognition. Adding an embedding model on top would still need a second runtime for it. |
| **sherpa-onnx: pyannote segmentation-3.0 ONNX + 3D-Speaker CAM++ + fast clustering** | the pyannote recipe exported to ONNX, in C++ with a C API and an official Rust crate | ONNX Runtime, linked statically (prebuilt by k2-fsa) | **34 MB** (5.99 MB + 28.3 MB) | **yes** | code Apache-2.0, ONNX Runtime MIT, segmentation MIT, CAM++ Apache-2.0 | **Chosen.** Same approach as OpenWhispr's local diarization (pyannote-seg ONNX + CAM++ + agglomerative clustering at cosine 0.5, no Python). |

Measured on an Apple M2 Pro (release build, 4 threads, window shift 0.25), before other work loaded the machine:

- 16 s two-speaker English clip: 0.53–0.62 s end to end incl. model load (2 speakers found, correct).
- 57 s four-speaker Mandarin clip: 1.2–2.0 s (4 speakers found at cluster thresholds 0.5–0.7). Default window shift 0.1 took 4.8 s; 0.25 found the same speakers 2.4× faster.
- One voiceprint of 3 s audio: 28 ms. Peak RSS for a 5-minute file: ~150 MB.
- OpenWhispr reports ~30 s for a 45-minute meeting on an M1 with the same models.

Binary size: the static sherpa-onnx + ONNX Runtime adds ~15 MB to the core (a stripped LTO prototype was 17 MB); see the handoff for the measured delta of `libpennel_core`.

### Embedding model choice (measured)

Cosine similarity between voiceprints, same pipeline:

| Model (size) | same speaker | different speakers | separation |
|---|---|---|---|
| 3D-Speaker CAM++ VoxCeleb (29.6 MB) | 0.15–0.98 | up to 0.89 | unusable on these clips |
| WeSpeaker ResNet34-LM VoxCeleb (26.5 MB) | 0.88–0.93 | 0.67–0.90 | unusable on these clips |
| 3D-Speaker ERes2Net base zh (39.6 MB) | 0.65–0.83 | ≤ 0.28 | good |
| **3D-Speaker CAM++ zh-en "common advanced" (28.3 MB)** | **0.67–0.94** | **≤ 0.46** | **best** |

Test material (not committed): public Mandarin test speech from the sherpa-onnx release (3 speakers), 8 LibriSpeech utterances of one speaker (same-speaker 0.79–0.92 on CAM++ zh-en), and Dutch/English synthetic voices (Xander, Ellen, Daniel, Samantha: same 0.82–0.94, different ≤ 0.37). The zh-en advanced model is trained on ~200k speakers and separates Dutch voices well; VoxCeleb-only exports did not separate even the clean clips.

Thresholds (in `pennel-speakers`): diarizer cluster distance 0.6 (over-splitting is cheaper than merging two people), the user ≥ 0.50 cosine (enrollment is clean read speech, meetings are not), known voices ≥ 0.55, joining chunk clusters ≥ 0.55. These are first values from the measurements above; they need real meeting data to tune.

### Build and platform notes

- `sherpa-onnx` 2.0.0 (crates.io, official) downloads prebuilt static libraries per target at build time (macOS arm64/x64, Linux x64/aarch64, Windows x64/arm64). No CMake build of ONNX Runtime.
- Windows: those libraries use the static C runtime (/MT). MSVC refuses to link them with transcribe.cpp built for /MD (LNK2038, seen in CI). The repository `.cargo/config.toml` therefore builds every native dependency for the static runtime on Windows (`+crt-static`, `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded`). Side effect: no VC++ runtime dependency.
- iPhone: the core builds with `--no-default-features`; the `speakers` feature (default on) is off, so the native dependency is omitted and local recognition returns the Dutch error "Sprekers herkennen op dit toestel zit niet in deze versie van Pennel. Kies ‘Via Pennel Cloud’…". Matching (pure Rust) still works, so Pennel Cloud diarization plus on-device matching is the iPhone path.

## Architecture

```
core/speakers (crate pennel-speakers)        shared by the app core and the Pennel Cloud server
  always: types (Diarization, Turn, Speaker), cosine, weighted mean, blend, renumber,
          merge_chunks (join chunked results), identify (me / known / unknown)
  feature onnx: diarize(models, samples) -> turns + voiceprint per speaker, embed(...)

core (pennel-core)
  feature speakers = pennel-speakers/onnx (default)
  models.rs    two catalog entries (Speakers kind, .onnx), pinned by commit + sha256
  voices.rs    VoiceStore: <data dir>/voices/voiceprints.json, voices/meetings/<sha256(meeting uid)>.json
  diarize.rs   apply(): label transcript lines from turns on either track; carry_names(); Sortformer fallback
  cloud/diarize.rs  POST /v1/audio/diarize in 10-minute WAV chunks, merge_chunks
  speakers.rs  engine surface (uniffi)

apps/desktop   commands, enrollment recording, postprocess, settings, onboarding, meeting naming
pennel-cloud   POST /v1/audio/diarize, same crate pinned by git rev, models baked into the image
```

### Pipeline

1. Segmentation (pyannote 3.0, 10 s windows, shift 0.25) finds speech and local speaker changes, including overlap.
2. CAM++ embeds each local speaker per window; fast agglomerative clustering (threshold 0.6, no fixed count) gives the meeting's speakers.
3. Per speaker, Pennel builds one voiceprint from its longest turns (≥ 1 s, at most 60 s of speech, joined) — `voiceprint_spans`.
4. `identify`: every speaker that matches the user's voiceprint is "Ik" (a diarizer may split one voice); known voices name at most one speaker each, best similarity first; a known voice that is closer than the user wins.
5. `apply`: each transcript line gets the speaker that overlaps it most (lines store start times only; a line ends at the next line, its estimated speaking time or 30 s). Voices are numbered 1.. in order of first line, up to 12.

Which track is diarized (decided in the core, `recognize_speakers(id, microphone, system)`):

- System track present and not silent (a call): split "Anderen" as before; "Ik" lines are never touched.
- No system audio (in-person meeting on one microphone) and the user trained their voice: split the microphone track; the user's speakers stay "Ik", everybody else becomes "Anderen N". Without a trained voice nothing changes (we cannot tell which speaker is the user).
- The microphone route only relabels when the user's voice is actually found among the speakers; otherwise every line stays as it was (no "Ik" can turn into somebody else). It needs voiceprints, so it never runs on the Sortformer fallback.
- A rerun is idempotent: on the microphone route "Ik" lines and lines an earlier microphone run gave a voice take part; plain "Anderen" lines (from a system track) are left alone, and a line no speaker overlaps keeps its side.
- Recognition and "Onthoud deze stem" are serialised so their read-relabel-write steps never interleave.

### Data model

- Transcript Markdown is unchanged: `[mm:ss] **Ik:** …`, `[mm:ss] **Anderen 2:** …`, names in the meeting frontmatter `speakers: 1=Jan; 2=Anna`. Older Pennel versions read new files; only voices 5–12 (new) are unknown to them, which their existing guard treats as unparsed lines (shown as missing, never rewritten).
- Names survive reruns: a recognised known voice brings its name (recognised voices are named first); otherwise a new voice takes the name of the old voice most of its lines had. Each name is used once.
- Voiceprints never enter the meeting folder (which syncs). `<data dir>/voices/voiceprints.json` (0600, atomic write) holds `{version, model, me: {embedding, speech_ms, created}, known: [{id, name, embedding, meetings, updated}]}`. A file of another embedding model is ignored (voiceprints of different models are not comparable); an unreadable file is never overwritten.
- `<data dir>/voices/meetings/<sha256(meeting uid)>.json` keeps the voiceprint of each voice of a meeting so the user can still choose "Onthoud deze stem" later. Hashed names keep titles out of file names. Deleted with the meeting.
- Remembering the same name again (case-insensitive) blends the new voiceprint into the stored one (weight grows with the number of meetings, capped at 10) instead of creating a duplicate.

### Privacy

- Never audio: enrollment audio lives in memory only and is dropped after the voiceprint is computed; the meeting tracks are the existing m4a files.
- Logs hold durations, counts and errors only; no names, voiceprints, transcript text or audio (core, desktop and server).
- Cloud: only when the user chose "Via Pennel Cloud". The server processes the chunk in memory, does not store or log it, meters audio seconds like transcription and returns turns and voiceprints. Known voices and the user's voiceprint never leave the device; matching is local.
- "Vergeet mijn stem", per-voice "Vergeten" and "Vergeet alle stemmen" (removes the voices folder) in settings.

### UI flows (desktop; same copy on every platform)

- Onboarding: final, skippable step "Leer je stem" after the practice dictation. Shows the enrollment text, record/stop with elapsed time, result or Dutch error. If local models are missing it offers the 34 MB download first; in cloud mode it needs sign-in.
- Instellingen → Meetings → "Sprekers herkennen": toggle "Sprekers herkennen na elke meeting"; choice "Op dit toestel" / "Via Pennel Cloud" (disabled with reason where a route is unavailable); model download; "Je stem" (status, Leer je stem / Opnieuw / Vergeet mijn stem); "Bekende stemmen" (rename, forget, Vergeet alle stemmen).
- Meeting: naming a voice offers "Onthoud deze stem" (off by default, only when this device holds the voiceprint); remembered voices are marked; rerun reports "N stemmen gevonden · M herkend".

### Cloud API

`POST /v1/audio/diarize` with `Authorization: Bearer <access token>` (same device session as the proxy, one forced refresh on 401) and `Content-Type: audio/wav` (16-bit mono 16 kHz PCM, ≤ 10.5 min, ≤ 25 MB, the proxy body limit). Response `{"model": "3dspeaker-campplus-zh-en-advanced", "turns": [{start_ms, end_ms, speaker}], "speakers": [{speaker, speech_ms, embedding}]}`. Errors use the proxy's `{"error", "message"}` shape with Dutch messages; the client rejects another `model` so voiceprints stay comparable. The client sends 10-minute chunks (a tail under 30 s joins the previous chunk) and joins speakers across chunks by voiceprint similarity without merging two speakers of one chunk. Limits: monthly audio quota shared with transcription, per-account limiter and a global concurrency bound for this CPU-heavy route.

### Migration from Sortformer

- Old meetings stay readable; the transcript format did not change.
- New runs use the new pipeline once its models are downloaded. An installation that only has Sortformer keeps diarizing with it (no voiceprints, so no "Ik" or known voices) until the user downloads the new models; enrollment always needs the new models (or the cloud). The app no longer offers the Sortformer download.
- Sortformer and transcribe.cpp's diarizer stay in the code for that fallback; removing them later only needs the fallback branch in `speaker_diarization` and `diarize::diarize`.

## Testing

- `pennel-speakers`: unit tests for maths, renumbering, voiceprint span choice, chunk merging (same voice across chunks, never within one chunk), identification; an ignored test runs the real models on a WAV.
- Core: voice store (round trip, model mismatch, never overwrite a broken file, hashed meeting files), transcript labelling on both tracks, voice limit, name carry-over, silence detection, cloud client (bearer, chunking, Dutch errors, model mismatch, signed out) and an end-to-end engine test (enroll through the cloud mock, recognise "Ik" and a remembered voice on one microphone, remember a new voice).
- All of it runs with and without the `speakers` feature.
