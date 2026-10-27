# Local meeting transcription implementation plan

Source: the user's approved plan in this Codex chat, 27 October 2026.

## Outcome and constraints

Fix sub-millisecond timestamp rounding that fragments local meeting audio. Add a separate local-only full retranscription action for saved recordings, preserving audio, notes and a recoverable previous transcript. Keep the configured model; never upload private audio or invoke cloud summary generation as part of this operation.

## Tasks

1. Add a failing shared-core session regression for sixty continuous packets of 17,288 samples at 16 kHz. Verify three chunks, all samples retained, true gaps/overlap handling unchanged. Normalize timestamp rounding using the existing desktop timing contract.
2. Add core retranscription staging, frozen local recognition configuration, progress, per-meeting conflict protection, complete required-track validation and atomic commit with a private backup. Decode one archived track at a time through the platform decoder. Rebuild speaker labels only through local recognition and reliable voice matching; keep prior labels in the backup. Track summary provenance so old summaries are visibly stale.
3. Expose `retranscribe_meeting_audio(id)` separately from failed-chunk retry, integrate progress and busy state, and add the same English/Dutch confirmation/action/error states to all desktop platforms. Missing audio/models must have actionable explanations.
4. Run relevant core, speaker, desktop, frontend, formatting and lint checks. Preserve the known Linux AAC decoder failure in the report if still present. Privately compare old/new Parakeet and Whisper Large v3 Turbo against manually checked Dutch reference text when the model files are available. Real Mac/package acceptance stays explicitly pending until tested on that device.

## Verification and review

Write and run regression tests before production edits. Test successful one/two-track replacement, frozen settings, missing/corrupt audio, model and persistence errors, backups, stale summaries, speaker identity safety and conflicting operations. Private user files must not enter repository fixtures. Perform a whole-change review after verification. Follow `docs/desktop-roadmap.md` for issue and project tracking.
