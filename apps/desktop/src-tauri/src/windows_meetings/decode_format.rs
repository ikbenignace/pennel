#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PcmFormat {
    pub rate: u32,
    pub channels: u16,
}

impl PcmFormat {
    pub fn float32(
        rate: u32,
        channels: u32,
        bits: u32,
        block_alignment: u32,
        is_float_audio: bool,
    ) -> Result<Self, String> {
        if !is_float_audio
            || rate == 0
            || channels == 0
            || channels > u16::MAX.into()
            || bits != 32
            || block_alignment != channels * 4
        {
            return Err("Ongeldig audioformaat".into());
        }
        Ok(Self {
            rate,
            channels: channels as u16,
        })
    }

    pub fn block_alignment(self) -> u32 {
        u32::from(self.channels) * 4
    }
}

/// The decoder can finish negotiating on its first read, or update unrelated media attributes.
/// Once PCM was consumed, changing its layout would corrupt the streaming resampler's timeline.
pub(crate) fn update_format(
    current: &mut PcmFormat,
    next: PcmFormat,
    received_audio: bool,
) -> Result<bool, String> {
    if *current == next {
        return Ok(false);
    }
    if received_audio {
        return Err("Het audioformaat verandert tijdens het bestand".into());
    }
    *current = next;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(rate: u32, channels: u32) -> PcmFormat {
        PcmFormat::float32(rate, channels, 32, channels * 4, true).unwrap()
    }

    #[test]
    fn initial_decoder_negotiation_uses_the_format_of_the_first_pcm_sample() {
        let mut current = format(16_000, 1);
        assert!(update_format(&mut current, format(32_000, 2), false).unwrap());
        assert_eq!(current.rate, 32_000);
        assert_eq!(current.channels, 2);
        assert_eq!(current.block_alignment(), 8);
    }

    #[test]
    fn equivalent_media_type_notifications_preserve_the_existing_resampler() {
        let mut current = format(16_000, 1);
        for received_audio in [false, true] {
            assert!(!update_format(&mut current, format(16_000, 1), received_audio).unwrap());
        }
    }

    #[test]
    fn rate_or_channel_changes_after_any_pcm_input_are_rejected() {
        let original = format(16_000, 1);
        for next in [format(48_000, 1), format(16_000, 2)] {
            let mut current = original;
            assert!(update_format(&mut current, next, true).is_err());
            assert_eq!(current, original);
        }
    }

    #[test]
    fn unsupported_or_malformed_pcm_is_rejected_even_during_initial_negotiation() {
        for (rate, channels, bits, alignment, float_audio) in [
            (0, 1, 32, 4, true),
            (16_000, 0, 32, 0, true),
            (16_000, u32::from(u16::MAX) + 1, 32, 4, true),
            (16_000, 1, 16, 2, true),
            (16_000, 1, 32, 4, false),
            (16_000, 2, 32, 4, true),
        ] {
            assert!(PcmFormat::float32(rate, channels, bits, alignment, float_audio).is_err());
        }
    }
}
