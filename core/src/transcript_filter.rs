use std::collections::HashSet;

use crate::meetings::{Segment, Speaker};

const ECHO_WINDOW_MS: u64 = 5_000;
const ECHO_OVERLAP: f32 = 0.8;
const MIN_ECHO_TOKENS: usize = 3;

fn tokens(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

fn is_echo(me: &Segment, others: &Segment) -> bool {
    if me.start_ms.abs_diff(others.start_ms) > ECHO_WINDOW_MS {
        return false;
    }
    let (a, b) = (tokens(&me.text), tokens(&others.text));
    let smallest = a.len().min(b.len());
    smallest >= MIN_ECHO_TOKENS && a.intersection(&b).count() as f32 / smallest as f32 >= ECHO_OVERLAP
}

fn mostly_dictation(segment: &Segment, dictations: &[(u64, u64)]) -> bool {
    let length = segment.end_ms.saturating_sub(segment.start_ms);
    if length == 0 {
        return false;
    }
    let covered: u64 = dictations
        .iter()
        .map(|(start, end)| segment.end_ms.min(*end).saturating_sub(segment.start_ms.max(*start)))
        .sum();
    covered * 2 > length
}

/// Removes "Ik" segments that are speaker echo of the others or that the user dictated elsewhere.
pub fn filter_segments(segments: &[Segment], dictations: &[(u64, u64)]) -> Vec<Segment> {
    let others: Vec<&Segment> = segments.iter().filter(|s| s.speaker == Speaker::Others && !s.failed).collect();
    segments
        .iter()
        .filter(|s| {
            s.speaker != Speaker::Me
                || s.failed
                || !(mostly_dictation(s, dictations) || others.iter().any(|o| is_echo(s, o)))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::Speaker;

    fn seg(start_s: u64, end_s: u64, speaker: Speaker, text: &str) -> Segment {
        Segment { start_ms: start_s * 1000, end_ms: end_s * 1000, speaker, text: text.into(), failed: false }
    }

    #[test]
    fn echo_of_others_is_removed_from_me() {
        let segments = [
            seg(10, 14, Speaker::Others, "We schuiven de release naar november."),
            seg(11, 15, Speaker::Me, "we schuiven de release naar november op"),
        ];
        let kept = filter_segments(&segments, &[]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].speaker, Speaker::Others);
    }

    #[test]
    fn different_text_is_kept() {
        let segments = [
            seg(10, 14, Speaker::Others, "We schuiven de release naar november."),
            seg(11, 15, Speaker::Me, "Dat is prima, dan plan ik de demo later."),
        ];
        assert_eq!(filter_segments(&segments, &[]).len(), 2);
    }

    #[test]
    fn far_apart_duplicates_are_kept() {
        let segments = [
            seg(10, 14, Speaker::Others, "we schuiven de release naar november"),
            seg(30, 34, Speaker::Me, "we schuiven de release naar november"),
        ];
        assert_eq!(filter_segments(&segments, &[]).len(), 2);
    }

    #[test]
    fn short_segments_are_not_deduped() {
        let segments = [seg(10, 11, Speaker::Others, "Ja."), seg(10, 11, Speaker::Me, "ja")];
        assert_eq!(filter_segments(&segments, &[]).len(), 2);
    }

    #[test]
    fn dictation_period_removes_me_segment() {
        let segments = [seg(30, 40, Speaker::Me, "stuur dit naar Sarah"), seg(31, 39, Speaker::Others, "ok")];
        let kept = filter_segments(&segments, &[(29_000, 41_000)]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].speaker, Speaker::Others);
    }

    #[test]
    fn partial_dictation_overlap_keeps_segment() {
        let segments = [seg(30, 40, Speaker::Me, "lange zin")];
        assert_eq!(filter_segments(&segments, &[(38_000, 45_000)]).len(), 1);
    }
}
