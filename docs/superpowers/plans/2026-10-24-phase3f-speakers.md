# Fase 3f — sprekers herkennen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After a meeting, split "Anderen" into separate voices (Anderen 1, 2, …) and let the user name them; names flow into the transcript, summaries, search, chat and export.

**Architecture:** Sortformer 4-speaker (transcribe-cpp, 139 MB, downloaded on demand like a local model) runs once over the full "Anderen" track after recording stops. Swift decodes `anderen.m4a` to 16 kHz mono and hands it to the core; the core assigns each Others segment the voice with the most overlap and rewrites `transcript.md` (`**Anderen 2:**`). Names live in `meeting.md` frontmatter as `speakers: 1=Jan; 2=Els`.

**Tech Stack:** Rust (transcribe-cpp diarize), UniFFI, AVFoundation decode, SwiftUI.

**Spec:** `docs/superpowers/specs/2026-10-24-phase3-design.md` (row 3f)

## Global Constraints

- Spike (2026-10-24, M2 Pro): 27 s synthetic 3-voice meeting → correct voices, 0.42 s.
- Opt-in setting `diarization` (default off); model id `diar_streaming_sortformer_4spk-v2.1` (Q8_0, sha `a5dacdc6…`), stored like local models but never listed as a speech model nor used as a fallback.
- Segment gains `voice: Option<u32>` (1–4); `None` = undiarized ("Anderen"). "Ik" segments never get a voice.
- Label rule everywhere: Me → "Ik"; Others with voice and name → name; with voice → "Anderen N"; else "Anderen".
- Overlap assignment: voice with the largest overlap in ms; ties → lower id; no overlap → `None`. A segment's end is the next Others segment's start (max 30 s), because `transcript.md` stores start times only.
- v1 limitation (decision): a 15–30 s "Anderen" chunk gets one voice, the majority speaker. Re-transcribing per diarized turn is a later improvement.
- Diarization failure never blocks the summary; it logs and continues.

## Review Focus

1. Old transcripts (`**Anderen:**`) still parse; new `**Anderen 3:**` and named lines round-trip. (Test `transcript_voice_round_trip`.)
2. Rename to a name containing `;`, `=` or `:` → sanitized, frontmatter stays valid. (Test `speaker_names_are_sanitized`.)
3. Diarization model not downloaded but setting on → summary still runs; UI offers download. (Engine test `diarize_without_model_is_model_not_downloaded`.)
4. Audio longer than the transcript or silent track → no voices assigned, no crash. (Test `assign_voices_without_turns`.)
5. Echo-filtered or failed segments keep `voice: None`. (Test `failed_segments_get_no_voice`.)

---

### Task 1: Voices in the transcript model
Files: `core/src/meetings.rs` (Segment.voice, label/parse/write, `speaker_names`, `rename_speaker`), `core/src/summary.rs`, `core/src/meeting_index.rs`, `core/src/export.rs`, `core/src/session.rs`/`transcript_filter.rs` (construct `voice: None`).
- [ ] Tests: `transcript_voice_round_trip`, `speaker_names_are_sanitized`, `labels_use_names` (summary transcript_text + export show "Jan"). Commit `feat(core): speaker voices and names in transcripts`.

### Task 2: Diarization
Files: create `core/src/diarize.rs` (`assign_voices(segments: &mut [Segment], turns: &[(u64, u64, u32)])`, `Diarizer::run(path, samples) -> Vec<(u64,u64,u32)>`), `core/src/models.rs` (diarizer entry, excluded from `local_models`/`downloaded`), `core/src/settings.rs` (`diarization: bool`), `core/src/engine.rs` (`diarize_meeting(id, samples: Vec<f32>) -> Result<u32, MusliError>` = number of voices; `diarizer_model_id() -> String`).
- [ ] Tests: `assign_voices_by_overlap`, `assign_voices_without_turns`, `failed_segments_get_no_voice`, `diarizer_not_listed_as_speech_model`, `diarize_without_model_is_model_not_downloaded`, ignored `real_diarization_splits_voices` (uses `/tmp/diar/meeting.wav` when present). Commit `feat(core): diarize meetings`.

### Task 3: macOS
Files: create `Meetings/AudioDecoder.swift` (m4a → 16 kHz mono Float), modify `MeetingController.swift` (after stop: if `settings.diarization` and model downloaded → decode + `diarizeMeeting` before summarizing), `Windows/MeetingsPane.swift` (toggle "Sprekers herkennen" + download/progress via `ModelDownloads`), `Meetings/MeetingViews.swift` (labels via `segment.voice` + names, click a voice label → rename popover with attendee suggestions; per-voice colour; "Sprekers herkennen" button on an existing meeting).
- [ ] Build, commit `feat(macos): name speakers in meetings`.
