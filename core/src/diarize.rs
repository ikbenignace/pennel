use std::path::Path;

use crate::error::PennelError;
use crate::meetings::{Segment, Speaker};

/// Upper bound for a transcript chunk; the live session never cuts longer pieces.
const MAX_SEGMENT_MS: u64 = 30_000;
/// Generous speaking pace, so a short remark before a long silence doesn't borrow the next speaker's voice.
const MS_PER_WORD: u64 = 450;

/// A stretch of audio attributed to one voice: `(start_ms, end_ms, voice)`.
pub type Turn = (u64, u64, u32);

/// Runs the diarization model over a whole track (16 kHz mono).
#[cfg(feature = "local-models")]
pub fn diarize(model_path: &Path, samples: &[f32]) -> Result<Vec<Turn>, PennelError> {
    use transcribe_cpp::Model;

    if !model_path.exists() {
        return Err(PennelError::ModelNotDownloaded);
    }
    let error = |e: transcribe_cpp::Error| PennelError::InvalidResponse {
        message: format!("sprekers herkennen: {e}"),
    };
    let model = Model::load(model_path).map_err(error)?;
    let mut session = model.diarize_session().map_err(error)?;
    let turns = session.run(samples, &Default::default()).map_err(error)?;
    Ok(turns
        .into_iter()
        .filter(|t| t.speaker_id > 0 && t.t1_ms > t.t0_ms)
        .map(|t| (t.t0_ms.max(0) as u64, t.t1_ms as u64, t.speaker_id as u32))
        .collect())
}

#[cfg(not(feature = "local-models"))]
pub fn diarize(_model_path: &Path, _samples: &[f32]) -> Result<Vec<Turn>, PennelError> {
    Err(PennelError::local_models_unavailable())
}

/// Gives each "Anderen" segment the voice that overlaps it most. Transcripts store start times only, so a
/// segment ends at the next "Anderen" start, at 30 s, or when its words would have been spoken, whichever is first.
pub fn assign_voices(segments: &mut [Segment], turns: &[Turn]) {
    let starts: Vec<u64> = segments
        .iter()
        .filter(|s| s.speaker == Speaker::Others)
        .map(|s| s.start_ms)
        .collect();
    for segment in segments.iter_mut().filter(|s| s.speaker == Speaker::Others) {
        if segment.failed || segment.text.trim().is_empty() {
            segment.voice = None;
            continue;
        }
        let next = starts
            .iter()
            .copied()
            .filter(|s| *s > segment.start_ms)
            .min();
        let words = segment.text.split_whitespace().count() as u64;
        let cap = segment.start_ms + MAX_SEGMENT_MS.min(1_000 + words * MS_PER_WORD);
        let end = next.map_or(cap, |n| n.min(cap)).max(segment.end_ms);
        let mut overlap: Vec<(u32, u64)> = Vec::new();
        for &(t0, t1, voice) in turns {
            let shared = t1.min(end).saturating_sub(t0.max(segment.start_ms));
            if shared == 0 {
                continue;
            }
            match overlap.iter_mut().find(|(v, _)| *v == voice) {
                Some((_, total)) => *total += shared,
                None => overlap.push((voice, shared)),
            }
        }
        segment.voice = overlap
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|(voice, _)| voice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn others(start_ms: u64, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms,
            speaker: Speaker::Others,
            text: text.into(),
            failed: false,
            voice: None,
        }
    }

    #[test]
    fn assign_voices_by_overlap() {
        let long = "woord ".repeat(30);
        let mut segments = vec![
            others(0, &long),
            Segment {
                speaker: Speaker::Me,
                ..others(3_000, "ik")
            },
            others(5_000, &long),
            others(15_000, &long),
        ];
        let turns = [
            (0, 4_600, 1),
            (5_100, 8_900, 2),
            (9_500, 14_200, 1),
            (14_600, 15_100, 2),
            (15_100, 18_500, 3),
        ];
        assign_voices(&mut segments, &turns);
        let voices: Vec<Option<u32>> = segments.iter().map(|s| s.voice).collect();
        assert_eq!(voices, [Some(1), None, Some(1), Some(3)]);
    }

    #[test]
    fn short_remark_before_silence_keeps_its_own_voice() {
        let mut segments = vec![others(0, "ja klopt"), others(28_000, "verder")];
        assign_voices(&mut segments, &[(0, 1_500, 1), (3_000, 27_000, 2)]);
        assert_eq!(segments[0].voice, Some(1));
    }

    #[test]
    fn ties_go_to_lower_voice() {
        let mut segments = vec![others(0, &"woord ".repeat(30)), others(10_000, "b")];
        assign_voices(&mut segments, &[(0, 5_000, 2), (5_000, 10_000, 1)]);
        assert_eq!(segments[0].voice, Some(1));
    }

    #[test]
    fn assign_voices_without_turns() {
        let mut segments = vec![others(0, "a"), others(40_000, "b")];
        assign_voices(&mut segments, &[]);
        assert!(segments.iter().all(|s| s.voice.is_none()));
        assign_voices(&mut segments, &[(100_000, 120_000, 1)]);
        assert!(segments.iter().all(|s| s.voice.is_none()));
    }

    #[test]
    fn failed_segments_get_no_voice() {
        let mut segments = vec![
            Segment {
                failed: true,
                ..others(0, "")
            },
            others(5_000, "b"),
        ];
        assign_voices(&mut segments, &[(0, 10_000, 1)]);
        assert_eq!(segments[0].voice, None);
        assert_eq!(segments[1].voice, Some(1));
    }

    /// Needs the Sortformer model in the app folder and `/tmp/diar/meeting.wav`; run with `--ignored`.
    #[test]
    #[ignore]
    fn real_diarization_splits_voices() {
        let home = std::env::var("HOME").unwrap();
        let model = std::path::PathBuf::from(home)
            .join("Library/Application Support/Pennel/models")
            .join(format!("{}.gguf", crate::models::DIARIZER_ID));
        let wav =
            std::fs::read(std::env::var("DIAR_WAV").unwrap_or("/tmp/diar/meeting.wav".into()))
                .unwrap();
        let (samples, _) = crate::wav::decode_wav(&wav).unwrap();
        let turns = diarize(&model, &samples).unwrap();
        let voices: std::collections::HashSet<u32> = turns.iter().map(|t| t.2).collect();
        eprintln!("{turns:?}");
        assert_eq!(voices.len(), 3);
    }

    #[cfg(not(feature = "local-models"))]
    #[test]
    fn diarize_without_feature_is_dutch_error() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(
            diarize(file.path(), &[0.0; 16_000]),
            Err(PennelError::local_models_unavailable())
        );
    }
}
