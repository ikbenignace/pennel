use std::path::Path;

use pennel_speakers::{Identity, Turn as SpeakerTurn};
use transcribe_cpp::Model;

use crate::error::PennelError;
use crate::meetings::{Segment, Speaker, SpeakerName, VOICES};

/// Upper bound for a transcript chunk; the live session never cuts longer pieces.
const MAX_SEGMENT_MS: u64 = 30_000;
/// Generous speaking pace, so a short remark before a long silence doesn't borrow the next speaker's voice.
const MS_PER_WORD: u64 = 450;

/// A stretch of audio attributed to one voice: `(start_ms, end_ms, voice)`.
pub type Turn = (u64, u64, u32);

/// Runs the Sortformer model over a whole track (16 kHz mono). Kept for installations that only
/// have this model; it finds voices but no voiceprints.
pub fn diarize(model_path: &Path, samples: &[f32]) -> Result<Vec<Turn>, PennelError> {
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

/// Sortformer turns as a diarization without voiceprints.
pub fn from_sortformer(turns: &[Turn]) -> pennel_speakers::Diarization {
    let (turns, count) = pennel_speakers::renumber(
        turns
            .iter()
            .map(|&(start, end, voice)| (start, end, i64::from(voice))),
    );
    let speakers = (0..count as u32)
        .map(|speaker| pennel_speakers::Speaker {
            speaker,
            speech_ms: pennel_speakers::speech_ms(&turns, speaker),
            embedding: Vec::new(),
        })
        .collect();
    pennel_speakers::Diarization { turns, speakers }
}

/// For each segment `include` accepts, the speaker that overlaps it most. Transcripts store start
/// times only, so a segment ends at the next included start, at 30 s, or when its words would have
/// been spoken, whichever is first.
fn dominant_speakers(
    segments: &[Segment],
    turns: &[SpeakerTurn],
    include: impl Fn(&Segment) -> bool,
) -> Vec<Option<u32>> {
    let starts: Vec<u64> = segments
        .iter()
        .filter(|s| include(s))
        .map(|s| s.start_ms)
        .collect();
    segments
        .iter()
        .map(|segment| {
            if !include(segment) || segment.failed || segment.text.trim().is_empty() {
                return None;
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
            for turn in turns {
                let shared = turn
                    .end_ms
                    .min(end)
                    .saturating_sub(turn.start_ms.max(segment.start_ms));
                if shared == 0 {
                    continue;
                }
                match overlap.iter_mut().find(|(s, _)| *s == turn.speaker) {
                    Some((_, total)) => *total += shared,
                    None => overlap.push((turn.speaker, shared)),
                }
            }
            overlap
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(speaker, _)| speaker)
        })
        .collect()
}

/// Gives each "Anderen" segment the voice that overlaps it most (Sortformer voices, 1-based).
#[cfg(test)]
fn assign_voices(segments: &mut [Segment], turns: &[Turn]) {
    let diarization = from_sortformer(turns);
    apply(
        segments,
        &diarization.turns,
        &vec![Identity::Unknown; diarization.speakers.len()],
        Speaker::Others,
    );
}

/// Which recording a diarization came from. A shared microphone holds everyone, so its result may
/// relabel "Ik" lines; the system-audio track only splits "Anderen".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Microphone,
    System,
}

impl From<Speaker> for Track {
    fn from(speaker: Speaker) -> Self {
        match speaker {
            Speaker::Me => Track::Microphone,
            Speaker::Others => Track::System,
        }
    }
}

/// The outcome of [`apply`]: which diarization speaker each meeting voice is, and whether the user
/// was heard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Applied {
    /// `(voice, speaker index)`, voices numbered 1.. in order of first line.
    pub voices: Vec<(u32, usize)>,
    pub me_found: bool,
}

/// Labels transcript lines with the diarized speakers. On a microphone track the user's own speakers
/// become "Ik" and every other speaker an "Anderen N" voice; on the system track only "Anderen" lines
/// change. Lines no speaker overlaps keep their side and lose a stale voice.
pub fn apply(
    segments: &mut [Segment],
    turns: &[SpeakerTurn],
    identities: &[Identity],
    track: impl Into<Track>,
) -> Applied {
    let track = track.into();
    let include = |s: &Segment| track == Track::Microphone || s.speaker == Speaker::Others;
    let dominant = dominant_speakers(segments, turns, include);
    let is_me = |speaker: u32| {
        track == Track::Microphone && identities.get(speaker as usize) == Some(&Identity::Me)
    };
    let mut applied = Applied::default();
    for (segment, speaker) in segments.iter_mut().zip(dominant) {
        if !include(segment) || segment.failed || segment.text.trim().is_empty() {
            continue;
        }
        let Some(speaker) = speaker else {
            segment.voice = None;
            continue;
        };
        if is_me(speaker) {
            segment.speaker = Speaker::Me;
            segment.voice = None;
            applied.me_found = true;
            continue;
        }
        let voice = match applied.voices.iter().find(|(_, s)| *s == speaker as usize) {
            Some((voice, _)) => Some(*voice),
            None => {
                let next = applied.voices.len() as u32 + 1;
                VOICES.contains(&next).then(|| {
                    applied.voices.push((next, speaker as usize));
                    next
                })
            }
        };
        segment.speaker = Speaker::Others;
        segment.voice = voice;
    }
    applied
}

/// Names for the new voices: a recognised known voice keeps its name; otherwise a voice takes the
/// name the user gave the voice most of its lines had before, so naming survives a rerun.
pub fn carry_names(
    before: &[Segment],
    after: &[Segment],
    names_before: &[SpeakerName],
    known: &[(u32, String)],
) -> Vec<SpeakerName> {
    let mut voices: Vec<u32> = after.iter().filter_map(|s| s.voice).collect();
    voices.sort_unstable();
    voices.dedup();
    let mut taken: Vec<String> = Vec::new();
    let mut names = Vec::new();
    for voice in voices {
        let name = known
            .iter()
            .find(|(v, _)| *v == voice)
            .map(|(_, name)| name.clone())
            .or_else(|| {
                let mut counts: Vec<(u32, usize)> = Vec::new();
                for (old, new) in before.iter().zip(after) {
                    if new.voice != Some(voice) {
                        continue;
                    }
                    if let Some(old) = old.voice.filter(|_| old.speaker == Speaker::Others) {
                        match counts.iter_mut().find(|(v, _)| *v == old) {
                            Some((_, n)) => *n += 1,
                            None => counts.push((old, 1)),
                        }
                    }
                }
                counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                counts.into_iter().find_map(|(old, _)| {
                    names_before
                        .iter()
                        .find(|n| n.voice == old)
                        .map(|n| n.name.clone())
                })
            });
        if let Some(name) = name.filter(|n| !taken.contains(n)) {
            taken.push(name.clone());
            names.push(SpeakerName { voice, name });
        }
    }
    names
}

/// Whether a track holds (almost) no sound: under 1% of 30 ms frames reach -40 dBFS.
pub fn mostly_silent(samples: &[f32]) -> bool {
    const FRAME: usize = 480;
    const LOUD: f32 = 0.01;
    let frames = samples.len() / FRAME;
    if frames == 0 {
        return true;
    }
    let loud = samples
        .as_chunks::<FRAME>()
        .0
        .iter()
        .filter(|frame| (frame.iter().map(|x| x * x).sum::<f32>() / FRAME as f32).sqrt() >= LOUD)
        .count();
    loud * 100 < frames
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

    fn me(start_ms: u64, text: &str) -> Segment {
        Segment {
            speaker: Speaker::Me,
            ..others(start_ms, text)
        }
    }

    fn turn(start_ms: u64, end_ms: u64, speaker: u32) -> SpeakerTurn {
        SpeakerTurn {
            start_ms,
            end_ms,
            speaker,
        }
    }

    fn labels(segments: &[Segment]) -> Vec<(Speaker, Option<u32>)> {
        segments.iter().map(|s| (s.speaker, s.voice)).collect()
    }

    #[test]
    fn assign_voices_by_overlap() {
        let long = "woord ".repeat(30);
        let mut segments = vec![
            others(0, &long),
            me(3_000, "ik"),
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
        assert_eq!(voices, [Some(1), None, Some(1), Some(2)]);
    }

    #[test]
    fn short_remark_before_silence_keeps_its_own_voice() {
        let mut segments = vec![others(0, "ja klopt"), others(28_000, "verder")];
        assign_voices(&mut segments, &[(0, 1_500, 1), (3_000, 27_000, 2)]);
        assert_eq!(segments[0].voice, Some(1));
    }

    #[test]
    fn ties_go_to_the_voice_heard_first() {
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

    #[test]
    fn system_track_never_touches_my_lines() {
        let mut segments = vec![me(0, "hallo"), others(2_000, "dag")];
        let applied = apply(
            &mut segments,
            &[turn(0, 4_000, 0)],
            &[Identity::Me],
            Speaker::Others,
        );
        assert_eq!(
            labels(&segments),
            [(Speaker::Me, None), (Speaker::Others, Some(1))]
        );
        assert!(!applied.me_found);
        assert_eq!(applied.voices, [(1, 0)]);
    }

    #[test]
    fn shared_microphone_splits_me_from_the_room() {
        let mut segments = vec![
            me(0, "goedemorgen allemaal"),
            me(4_000, "dank je"),
            me(8_000, "ik ben het niet eens"),
            others(12_000, "vroeger anders gelabeld"),
        ];
        let turns = [
            turn(0, 3_500, 0),
            turn(4_000, 7_500, 1),
            turn(8_000, 11_500, 2),
            turn(12_000, 15_000, 1),
        ];
        let applied = apply(
            &mut segments,
            &turns,
            &[Identity::Unknown, Identity::Me, Identity::Known(0, 0.8)],
            Speaker::Me,
        );
        assert_eq!(
            labels(&segments),
            [
                (Speaker::Others, Some(1)),
                (Speaker::Me, None),
                (Speaker::Others, Some(2)),
                (Speaker::Me, None),
            ]
        );
        assert!(applied.me_found);
        assert_eq!(applied.voices, [(1, 0), (2, 2)]);
    }

    #[test]
    fn voices_beyond_the_limit_stay_plain_anderen() {
        let mut segments: Vec<Segment> = (0..14).map(|i| others(i * 5_000, "a b")).collect();
        let turns: Vec<SpeakerTurn> = (0..14)
            .map(|i| turn(i as u64 * 5_000, i as u64 * 5_000 + 2_000, i))
            .collect();
        apply(
            &mut segments,
            &turns,
            &vec![Identity::Unknown; 14],
            Speaker::Others,
        );
        assert_eq!(segments[11].voice, Some(12));
        assert_eq!(segments[12].voice, None);
    }

    #[test]
    fn names_follow_known_voices_and_earlier_naming() {
        let before = vec![
            Segment {
                voice: Some(3),
                ..others(0, "a")
            },
            Segment {
                voice: Some(3),
                ..others(5_000, "b")
            },
            Segment {
                voice: Some(1),
                ..others(10_000, "c")
            },
            me(15_000, "d"),
        ];
        let mut after = before.clone();
        after[0].voice = Some(1);
        after[1].voice = Some(1);
        after[2].voice = Some(2);
        after[3] = Segment {
            voice: Some(3),
            ..others(15_000, "d")
        };
        let names_before = vec![
            SpeakerName {
                voice: 1,
                name: "Piet".into(),
            },
            SpeakerName {
                voice: 3,
                name: "Anna".into(),
            },
        ];
        let names = carry_names(&before, &after, &names_before, &[(2, "Jan".into())]);
        assert_eq!(
            names,
            [
                SpeakerName {
                    voice: 1,
                    name: "Anna".into()
                },
                SpeakerName {
                    voice: 2,
                    name: "Jan".into()
                },
            ]
        );
    }

    #[test]
    fn silence_detection() {
        assert!(mostly_silent(&[]));
        assert!(mostly_silent(&vec![0.0; 16_000 * 10]));
        assert!(mostly_silent(&vec![0.001; 16_000 * 10]));
        let mut speech = vec![0.0; 16_000 * 10];
        speech[..16_000].iter_mut().for_each(|x| *x = 0.1);
        assert!(!mostly_silent(&speech));
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
}
