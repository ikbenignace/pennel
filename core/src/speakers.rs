#![allow(dead_code)]

use std::path::Path;

use crate::error::PennelError;

pub(crate) const UNAVAILABLE: &str = "Sprekers herkennen op dit toestel is niet beschikbaar in deze versie van Pennel. Kies ‘Via Pennel Cloud’ in Instellingen → Meetings.";

pub(crate) fn unavailable() -> PennelError {
    PennelError::InvalidResponse {
        message: UNAVAILABLE.into(),
    }
}

#[cfg(feature = "speakers")]
pub(crate) fn diarize_local(
    segmentation: &Path,
    embedding: &Path,
    samples: &[f32],
) -> Result<pennel_speakers::Diarization, PennelError> {
    let models = pennel_speakers::Models {
        segmentation,
        embedding,
    };
    pennel_speakers::diarize(&models, samples, &pennel_speakers::Options::default()).map_err(
        |error| match error {
            pennel_speakers::Error::Model(_) => PennelError::ModelNotDownloaded,
            other => PennelError::InvalidResponse {
                message: other.to_string(),
            },
        },
    )
}

#[cfg(not(feature = "speakers"))]
pub(crate) fn diarize_local(
    _segmentation: &Path,
    _embedding: &Path,
    _samples: &[f32],
) -> Result<pennel_speakers::Diarization, PennelError> {
    Err(unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "speakers")]
    #[test]
    fn local_diarization_without_models_needs_a_download() {
        let missing = Path::new("/nonexistent/pennel/model.onnx");
        assert_eq!(
            diarize_local(missing, missing, &[0.0; 32_000]),
            Err(PennelError::ModelNotDownloaded)
        );
    }

    #[cfg(not(feature = "speakers"))]
    #[test]
    fn local_diarization_without_feature_is_the_dutch_error() {
        let missing = Path::new("/nonexistent/pennel/model.onnx");
        assert_eq!(
            diarize_local(missing, missing, &[0.0; 32_000]),
            Err(unavailable())
        );
    }
}
