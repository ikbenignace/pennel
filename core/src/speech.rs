const MIN_SECONDS: f32 = 0.3;
const MIN_RMS: f32 = 0.005;

/// Phrases speech-to-text models emit on silence or noise, compared after normalizing.
const HALLUCINATIONS: &[&str] = &[
    "",
    "you",
    "thank you",
    "thanks for watching",
    "thank you for watching",
    "ondertiteld door de amara.org gemeenschap",
    "ondertiteling door de amara.org gemeenschap",
    "bedankt voor het kijken",
    "tot de volgende keer",
];

pub fn has_speech(samples: &[f32], sample_rate: u32) -> bool {
    if (samples.len() as f32) < MIN_SECONDS * sample_rate as f32 {
        return false;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    mean_square.sqrt() >= MIN_RMS
}

pub fn is_hallucination(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    let normalized = normalized.trim_end_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());
    HALLUCINATIONS.contains(&normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * 16000.0) as usize)
            .map(|i| amplitude * (i as f32 * 440.0 * std::f32::consts::TAU / 16000.0).sin())
            .collect()
    }

    #[test]
    fn short_audio_is_not_speech() {
        assert!(!has_speech(&sine(0.2, 0.3), 16000));
    }

    #[test]
    fn silence_is_not_speech() {
        assert!(!has_speech(&vec![0.0; 32000], 16000));
        assert!(!has_speech(&sine(2.0, 0.001), 16000));
    }

    #[test]
    fn normal_audio_is_speech() {
        assert!(has_speech(&sine(1.0, 0.2), 16000));
    }

    #[test]
    fn filters_known_hallucinations() {
        assert!(is_hallucination(" Ondertiteld door de Amara.org gemeenschap "));
        assert!(is_hallucination("Thank you."));
        assert!(is_hallucination(""));
        assert!(!is_hallucination("Thank you, Sarah, see you tomorrow."));
    }
}
