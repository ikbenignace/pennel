use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// Shown first, in this order: the best picks for Dutch and English dictation.
pub const RECOMMENDED: [&str; 5] = [
    "parakeet-tdt-0.6b-v3",
    "whisper-large-v3-turbo",
    "cohere-transcribe-03-2026",
    "canary-1b-v2",
    "whisper-small",
];

/// Scored when no dictation language is set; Pennel's users dictate mostly in Dutch.
pub const DEFAULT_SCORE_LANGUAGE: &str = "nl";

const SPEED_SCALE: f64 = 8.0;
const ACCURACY_SCALE: f64 = 15.0;
const SPEED_FLOOR: u8 = 10;

#[derive(Deserialize)]
struct Root {
    models: Vec<RawModel>,
    #[serde(default)]
    results: HashMap<String, RawResults>,
}

#[derive(Deserialize)]
struct RawModel {
    id: String,
    name: String,
    description: String,
    repository: String,
    revision: String,
    #[serde(default)]
    parameters: Option<String>,
    languages: Vec<String>,
    quant: String,
    role: String,
    files: HashMap<String, CatalogFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogFile {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Deserialize, Default)]
struct RawResults {
    #[serde(default)]
    accuracy: HashMap<String, RawAccuracy>,
    #[serde(default)]
    performance: RawPerformance,
}

#[derive(Deserialize)]
struct RawAccuracy {
    error: f64,
}

#[derive(Deserialize, Default)]
struct RawPerformance {
    accelerated: Option<f64>,
    cpu: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parameters: Option<String>,
    pub languages: Vec<String>,
    pub file: CatalogFile,
    pub url: String,
    pub speed: u8,
    word_error: HashMap<String, f64>,
}

impl CatalogModel {
    pub fn supports(&self, language: &str) -> bool {
        self.languages.iter().any(|l| base_language(l) == language)
    }

    pub fn accuracy(&self, language: &str) -> Option<u8> {
        accuracy_score(self.word_error.get(language).copied())
    }
}

fn base_language(code: &str) -> &str {
    code.split(['-', '_']).next().unwrap_or(code)
}

pub fn speed_score(real_time_factor: Option<f64>) -> u8 {
    match real_time_factor {
        Some(rtf) if rtf > 0.0 => (100.0 * (1.0 - (-rtf / SPEED_SCALE).exp())).round() as u8,
        _ => SPEED_FLOOR,
    }
}

pub fn accuracy_score(word_error_percent: Option<f64>) -> Option<u8> {
    word_error_percent.map(|error| (100.0 * (-error / ACCURACY_SCALE).exp()).round() as u8)
}

fn parse(json: &str) -> Vec<CatalogModel> {
    let root: Root = serde_json::from_str(json).expect("bundled catalog.json matches schema 2");
    root.models
        .into_iter()
        .filter(|m| m.role == "asr")
        .filter_map(|m| {
            let file = m.files.get(&m.quant)?.clone();
            if file.filename.contains('/') {
                return None;
            }
            let results = root.results.get(&m.id);
            let rtf = results.and_then(|r| r.performance.accelerated.or(r.performance.cpu));
            let word_error = results
                .map(|r| {
                    r.accuracy
                        .iter()
                        .map(|(lang, cell)| (lang.clone(), cell.error))
                        .collect()
                })
                .unwrap_or_default();
            let url = format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                m.repository, m.revision, file.filename
            );
            Some(CatalogModel {
                description: if m.description.is_empty() {
                    generated_description(&m.languages)
                } else {
                    m.description
                },
                id: m.id,
                name: m.name,
                parameters: m.parameters,
                languages: m.languages,
                file,
                url,
                speed: speed_score(rtf),
                word_error,
            })
        })
        .collect()
}

fn generated_description(languages: &[String]) -> String {
    match languages {
        [only] => format!("Speech to text, {only} only"),
        _ => format!("Speech to text in {} languages", languages.len()),
    }
}

pub fn catalog() -> &'static [CatalogModel] {
    static CATALOG: OnceLock<Vec<CatalogModel>> = OnceLock::new();
    CATALOG.get_or_init(|| parse(include_str!("catalog.json")))
}

pub fn find(id: &str) -> Option<&'static CatalogModel> {
    catalog().iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_descriptions_use_english_and_preserve_language_codes() {
        assert_eq!(
            generated_description(&["nl".into()]),
            "Speech to text, nl only"
        );
        assert_eq!(
            generated_description(&["nl".into(), "en".into()]),
            "Speech to text in 2 languages"
        );
    }

    #[test]
    fn parses_bundled_catalog() {
        assert!(catalog().len() >= 20);
        let parakeet = find("parakeet-tdt-0.6b-v3").unwrap();
        assert_eq!(
            parakeet.url,
            "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/85ac09ea12fc4b1112fa76810059364bc6adc9de/parakeet-tdt-0.6b-v3-Q8_0.gguf"
        );
        assert_eq!(parakeet.file.size, 739_508_576);
        assert_eq!(
            parakeet.file.sha256,
            "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7"
        );
        assert!(parakeet.supports("nl"));
        for id in RECOMMENDED {
            assert!(find(id).is_some(), "{id}");
        }
    }

    #[test]
    fn speed_formula() {
        assert_eq!(speed_score(Some(25.065)), 96);
        assert_eq!(speed_score(None), 10);
        assert_eq!(speed_score(Some(0.0)), 10);
        assert_eq!(find("parakeet-tdt-0.6b-v3").unwrap().speed, 96);
    }

    #[test]
    fn accuracy_formula() {
        let parakeet = find("parakeet-tdt-0.6b-v3").unwrap();
        assert_eq!(parakeet.accuracy("nl"), Some(60));
        assert_eq!(parakeet.accuracy("en"), Some(72));
        assert_eq!(parakeet.accuracy("ja"), None);
    }

    #[test]
    fn non_asr_and_nested_files_excluded() {
        assert!(find("silero-vad-v6.2").is_none());
        assert!(find("diar_streaming_sortformer_4spk-v2.1").is_none());
        assert!(catalog().iter().all(|m| !m.file.filename.contains('/')));
    }

    #[test]
    fn region_codes_count_as_their_base_language() {
        let model = CatalogModel {
            id: "x".into(),
            name: "x".into(),
            description: String::new(),
            parameters: None,
            languages: vec!["nl-BE".into()],
            file: CatalogFile {
                filename: "x.gguf".into(),
                size: 1,
                sha256: String::new(),
            },
            url: String::new(),
            speed: 10,
            word_error: HashMap::new(),
        };
        assert!(model.supports("nl"));
    }
}
