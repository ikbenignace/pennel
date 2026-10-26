//! Local speech models (transcribe.cpp/ggml) are compiled for a fixed x86 instruction-set baseline;
//! running them on an older processor would crash with an illegal instruction, so check first.

use crate::error::PennelError;

pub const UNSUPPORTED_PROCESSOR: &str =
    "Deze processor is te oud voor lokale modellen. Kies een cloudprovider.";

/// Must match the GGML flags `scripts/desktop-release.py` builds Windows/Linux x64 releases with
/// (x86-64-v3 without AVX-512); the packaging tests compare both lists.
#[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
const REQUIRED_X86_FEATURES: &[&str] = &["sse4.2", "avx", "avx2", "fma", "f16c", "bmi2"];
/// transcribe-cpp-sys pins this floor itself for Intel Macs.
#[cfg(all(target_arch = "x86_64", target_os = "macos"))]
const REQUIRED_X86_FEATURES: &[&str] = &["sse4.2", "avx", "f16c"];
#[cfg(not(target_arch = "x86_64"))]
const REQUIRED_X86_FEATURES: &[&str] = &[];

fn missing_features<'a>(required: &[&'a str], detected: impl Fn(&str) -> bool) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|feature| !detected(feature))
        .collect()
}

fn reason_for(missing: &[&str]) -> Option<&'static str> {
    (!missing.is_empty()).then_some(UNSUPPORTED_PROCESSOR)
}

#[cfg(target_arch = "x86_64")]
fn detected(feature: &str) -> bool {
    match feature {
        "sse4.2" => std::arch::is_x86_feature_detected!("sse4.2"),
        "avx" => std::arch::is_x86_feature_detected!("avx"),
        "avx2" => std::arch::is_x86_feature_detected!("avx2"),
        "fma" => std::arch::is_x86_feature_detected!("fma"),
        "f16c" => std::arch::is_x86_feature_detected!("f16c"),
        "bmi2" => std::arch::is_x86_feature_detected!("bmi2"),
        _ => false,
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn detected(_: &str) -> bool {
    true
}

/// Why local speech and speaker models cannot run on this computer, or `None` when they can.
#[uniffi::export]
pub fn local_models_unavailable_reason() -> Option<String> {
    reason_for(&missing_features(REQUIRED_X86_FEATURES, detected)).map(str::to_owned)
}

pub(crate) fn ensure_local_models_supported() -> Result<(), PennelError> {
    match local_models_unavailable_reason() {
        Some(_) => Err(PennelError::UnsupportedProcessor),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V3: &[&str] = &["sse4.2", "avx", "avx2", "fma", "f16c", "bmi2"];

    #[test]
    fn a_processor_with_every_required_feature_runs_local_models() {
        assert!(missing_features(V3, |_| true).is_empty());
        assert_eq!(reason_for(&missing_features(V3, |_| true)), None);
    }

    #[test]
    fn a_processor_without_avx_is_refused_with_a_dutch_reason() {
        // Intel Pentium Gold/Celeron (Coffee Lake) stop at SSE4.2.
        let pentium = |feature: &str| feature == "sse4.2";
        assert_eq!(
            missing_features(V3, pentium),
            ["avx", "avx2", "fma", "f16c", "bmi2"]
        );
        assert_eq!(
            reason_for(&missing_features(V3, pentium)),
            Some(UNSUPPORTED_PROCESSOR)
        );
    }

    #[test]
    fn one_missing_feature_is_enough_to_refuse() {
        let without_bmi2 = |feature: &str| feature != "bmi2";
        assert_eq!(missing_features(V3, without_bmi2), ["bmi2"]);
        assert!(reason_for(&missing_features(V3, without_bmi2)).is_some());
    }

    #[test]
    fn no_requirements_means_supported() {
        assert_eq!(reason_for(&missing_features(&[], |_| false)), None);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn every_required_feature_is_one_the_runtime_check_knows() {
        let known = ["sse4.2", "avx", "avx2", "fma", "f16c", "bmi2"];
        assert!(REQUIRED_X86_FEATURES.iter().all(|f| known.contains(f)));
        assert!(!detected("avx512f"));
    }

    #[test]
    fn the_build_host_runs_its_own_local_models() {
        assert_eq!(local_models_unavailable_reason(), None);
        assert_eq!(ensure_local_models_supported(), Ok(()));
    }

    #[test]
    fn the_error_shows_the_same_reason() {
        assert_eq!(
            PennelError::UnsupportedProcessor.to_string(),
            UNSUPPORTED_PROCESSOR
        );
    }
}
