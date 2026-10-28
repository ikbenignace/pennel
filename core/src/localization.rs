use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum InterfaceLanguage {
    #[serde(rename = "nl")]
    Dutch,
    #[default]
    #[serde(rename = "en", other)]
    English,
}

impl InterfaceLanguage {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Dutch => "nl",
        }
    }
}

/// `System` remains a preference; the resolved language is never written over it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum InterfaceLanguagePreference {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "nl")]
    Dutch,
    #[default]
    #[serde(rename = "system", other)]
    System,
}

impl InterfaceLanguagePreference {
    pub fn resolve(self, system_languages: &[String]) -> InterfaceLanguage {
        match self {
            Self::English => InterfaceLanguage::English,
            Self::Dutch => InterfaceLanguage::Dutch,
            Self::System => system_languages
                .iter()
                .find_map(|language| {
                    let code = language.trim().split(['-', '_', '.', '@']).next()?;
                    if code.eq_ignore_ascii_case("en") {
                        Some(InterfaceLanguage::English)
                    } else if code.eq_ignore_ascii_case("nl") {
                        Some(InterfaceLanguage::Dutch)
                    } else {
                        None
                    }
                })
                .unwrap_or_default(),
        }
    }
}

pub fn resolve_interface_language(
    preference: InterfaceLanguagePreference,
    system_languages: &[String],
) -> InterfaceLanguage {
    preference.resolve(system_languages)
}

type Strings = BTreeMap<String, String>;

struct Catalogs {
    english: Strings,
    dutch: Strings,
    backend_sources: Vec<(String, String)>,
}

const ENGLISH: &[&str] = &[
    include_str!("../../assets/locales/common.en.json"),
    include_str!("../../assets/locales/dictation.en.json"),
    include_str!("../../assets/locales/meetings.en.json"),
    include_str!("../../assets/locales/providers.en.json"),
    include_str!("../../assets/locales/settings.en.json"),
    include_str!("../../assets/locales/onboarding.en.json"),
    include_str!("../../assets/locales/account.en.json"),
    include_str!("../../assets/locales/backend.en.json"),
];

const DUTCH: &[&str] = &[
    include_str!("../../assets/locales/common.nl.json"),
    include_str!("../../assets/locales/dictation.nl.json"),
    include_str!("../../assets/locales/meetings.nl.json"),
    include_str!("../../assets/locales/providers.nl.json"),
    include_str!("../../assets/locales/settings.nl.json"),
    include_str!("../../assets/locales/onboarding.nl.json"),
    include_str!("../../assets/locales/account.nl.json"),
    include_str!("../../assets/locales/backend.nl.json"),
];

fn catalogs() -> &'static Catalogs {
    static CATALOGS: OnceLock<Catalogs> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        let english = merge(ENGLISH);
        let dutch = merge(DUTCH);
        let mut sources: Vec<_> =
            parse_strings(include_str!("../../assets/locales/backend.sources.json"))
                .into_iter()
                .flat_map(|(key, legacy)| {
                    let mut aliases = vec![legacy];
                    aliases.extend(english.get(&key).cloned());
                    aliases.extend(dutch.get(&key).cloned());
                    aliases.sort();
                    aliases.dedup();
                    aliases.into_iter().map(move |alias| (key.clone(), alias))
                })
                .collect();
        // More specific messages win over a generic error-prefix template.
        sources.sort_by(|(a_key, a), (b_key, b)| {
            literal_size(b)
                .cmp(&literal_size(a))
                .then_with(|| a_key.cmp(b_key))
        });
        Catalogs {
            english,
            dutch,
            backend_sources: sources,
        }
    })
}

fn parse_strings(source: &str) -> Strings {
    struct UniqueStrings;

    impl<'de> serde::de::Visitor<'de> for UniqueStrings {
        type Value = Strings;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an object with unique string keys and string values")
        }

        fn visit_map<A>(self, mut values: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut result = Strings::new();
            while let Some((key, value)) = values.next_entry::<String, String>()? {
                if result.insert(key.clone(), value).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate translation key: {key}"
                    )));
                }
            }
            Ok(result)
        }
    }

    let mut deserializer = serde_json::Deserializer::from_str(source);
    let strings = serde::de::Deserializer::deserialize_map(&mut deserializer, UniqueStrings)
        .expect("bundled translation catalog must contain valid, unique strings");
    deserializer
        .end()
        .expect("bundled translation catalog must not contain trailing data");
    strings
}

fn merge(fragments: &[&str]) -> Strings {
    let mut result = Strings::new();
    for fragment in fragments {
        for (key, value) in parse_strings(fragment) {
            assert!(
                result.insert(key.clone(), value).is_none(),
                "duplicate translation key across catalogs: {key}"
            );
        }
    }
    result
}

fn argument_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn placeholders(template: &str) -> Vec<(usize, usize, &str)> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(open) = template[offset..].find('{') {
        let open = offset + open;
        let Some(close) = template[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close;
        let name = &template[open + 1..close];
        if argument_name(name) {
            found.push((open, close + 1, name));
            offset = close + 1;
        } else {
            offset = open + 1;
        }
    }
    found
}

fn render(template: &str, arguments: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(template.len());
    let mut offset = 0;
    for (start, end, name) in placeholders(template) {
        result.push_str(&template[offset..start]);
        if let Some((_, value)) = arguments.iter().find(|(key, _)| *key == name) {
            result.push_str(value);
        } else {
            result.push_str(&template[start..end]);
        }
        offset = end;
    }
    result.push_str(&template[offset..]);
    result
}

pub fn ui_text(language: InterfaceLanguage, key: &str, arguments: &[(&str, &str)]) -> String {
    let catalog = catalogs();
    let text = (language == InterfaceLanguage::Dutch)
        .then(|| catalog.dutch.get(key))
        .flatten()
        .or_else(|| catalog.english.get(key));
    text.map_or_else(|| key.into(), |text| render(text, arguments))
}

fn literal_size(template: &str) -> usize {
    template.len()
        - placeholders(template)
            .iter()
            .map(|(start, end, _)| end - start)
            .sum::<usize>()
}

fn has_fixed_word(template: &str) -> bool {
    let mut offset = 0;
    for (start, end, _) in placeholders(template) {
        if template[offset..start].chars().any(char::is_alphabetic) {
            return true;
        }
        offset = end;
    }
    template[offset..].chars().any(char::is_alphabetic)
}

fn capture_template<'a>(template: &str, source: &'a str) -> Option<Vec<(String, &'a str)>> {
    let slots = placeholders(template);
    if slots.is_empty() {
        return (template == source).then(Vec::new);
    }
    if !has_fixed_word(template) || slots.len() > 12 {
        return None;
    }
    let mut literals = Vec::with_capacity(slots.len() + 1);
    let mut names = Vec::with_capacity(slots.len());
    let mut offset = 0;
    for (start, end, name) in slots {
        literals.push(&template[offset..start]);
        names.push(name);
        offset = end;
    }
    literals.push(&template[offset..]);
    if literals[1..literals.len() - 1]
        .iter()
        .any(|literal| literal.is_empty())
    {
        return None;
    }
    let source = source.strip_prefix(literals[0])?;
    let mut matches = Vec::new();
    let mut budget = 256;
    capture_parts(
        source,
        &literals[1..],
        &names,
        &mut Vec::new(),
        &mut matches,
        &mut budget,
    );
    if matches.len() == 1 && budget > 0 {
        matches.pop()
    } else {
        None
    }
}

fn capture_parts<'a>(
    source: &'a str,
    literals: &[&str],
    names: &[&str],
    arguments: &mut Vec<(String, &'a str)>,
    matches: &mut Vec<Vec<(String, &'a str)>>,
    budget: &mut usize,
) {
    if *budget == 0 || matches.len() > 1 {
        return;
    }
    *budget -= 1;
    let Some(name) = names.first() else {
        if source.is_empty() {
            matches.push(arguments.clone());
        }
        return;
    };
    let delimiter = literals[0];
    let positions: Box<dyn Iterator<Item = usize> + '_> = if delimiter.is_empty() {
        Box::new(std::iter::once(source.len()))
    } else {
        Box::new(
            source
                .match_indices(delimiter)
                .map(|(position, _)| position),
        )
    };
    for position in positions {
        let value = &source[..position];
        if arguments
            .iter()
            .any(|(existing, previous)| existing == name && *previous != value)
        {
            continue;
        }
        arguments.push(((*name).into(), value));
        capture_parts(
            &source[position + delimiter.len()..],
            &literals[1..],
            &names[1..],
            arguments,
            matches,
            budget,
        );
        arguments.pop();
        if *budget == 0 || matches.len() > 1 {
            return;
        }
    }
}

/// Translates only known app-owned message templates, never user content or an arbitrary substring.
pub fn localize_ui_text(language: InterfaceLanguage, source: &str) -> String {
    for (key, template) in &catalogs().backend_sources {
        let Some(arguments) = capture_template(template, source) else {
            continue;
        };
        if !catalogs().english.contains_key(key) {
            return source.into();
        }
        let borrowed: Vec<_> = arguments
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        return ui_text(language, key, &borrowed);
    }
    source.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn explicit_languages_serialize_as_stable_codes_with_english_fallback() {
        assert_eq!(InterfaceLanguage::default(), InterfaceLanguage::English);
        for (language, code) in [
            (InterfaceLanguage::English, "en"),
            (InterfaceLanguage::Dutch, "nl"),
        ] {
            assert_eq!(language.code(), code);
            assert_eq!(
                serde_json::to_string(&language).unwrap(),
                format!("\"{code}\"")
            );
        }
        assert_eq!(
            serde_json::from_str::<InterfaceLanguage>("\"fr\"").unwrap(),
            InterfaceLanguage::English
        );
    }

    #[test]
    fn preferences_serialize_stably_and_default_to_system() {
        assert_eq!(
            InterfaceLanguagePreference::default(),
            InterfaceLanguagePreference::System
        );
        for (preference, code) in [
            (InterfaceLanguagePreference::System, "system"),
            (InterfaceLanguagePreference::English, "en"),
            (InterfaceLanguagePreference::Dutch, "nl"),
        ] {
            assert_eq!(
                serde_json::to_string(&preference).unwrap(),
                format!("\"{code}\"")
            );
            assert_eq!(
                serde_json::from_str::<InterfaceLanguagePreference>(&format!("\"{code}\""))
                    .unwrap(),
                preference
            );
        }
        assert_eq!(
            serde_json::from_str::<InterfaceLanguagePreference>("\"fr\"").unwrap(),
            InterfaceLanguagePreference::System
        );
    }

    #[test]
    fn system_language_matching_uses_first_supported_language_and_regions() {
        for (languages, expected) in [
            (vec!["nl-BE", "en-US"], InterfaceLanguage::Dutch),
            (vec!["en-GB", "nl-NL"], InterfaceLanguage::English),
            (vec!["fr-FR", "nl_NL.UTF-8"], InterfaceLanguage::Dutch),
            (
                vec!["de-DE", " EN_us.UTF-8@variant "],
                InterfaceLanguage::English,
            ),
            (vec!["fr", "NL_be@euro"], InterfaceLanguage::Dutch),
            (vec!["fr", "de"], InterfaceLanguage::English),
            (vec!["", "C", "POSIX"], InterfaceLanguage::English),
            (vec![], InterfaceLanguage::English),
        ] {
            let languages: Vec<_> = languages.into_iter().map(String::from).collect();
            assert_eq!(
                InterfaceLanguagePreference::System.resolve(&languages),
                expected,
                "{languages:?}"
            );
        }
    }

    #[test]
    fn explicit_language_overrides_system_without_changing_it() {
        let languages = vec!["nl-BE".to_owned()];
        assert_eq!(
            resolve_interface_language(InterfaceLanguagePreference::English, &languages),
            InterfaceLanguage::English
        );
        assert_eq!(
            resolve_interface_language(InterfaceLanguagePreference::Dutch, &["en-US".to_owned()]),
            InterfaceLanguage::Dutch
        );
        assert_eq!(languages, ["nl-BE"]);
    }

    #[test]
    fn shared_catalogs_translate_both_languages() {
        assert_eq!(
            ui_text(InterfaceLanguage::English, "common.cancel", &[]),
            "Cancel"
        );
        assert_eq!(
            ui_text(InterfaceLanguage::Dutch, "common.cancel", &[]),
            "Annuleren"
        );
    }

    #[test]
    fn substitutions_preserve_literal_user_text_and_never_expand_it_recursively() {
        let private = "{count} — Mijn Nederlandse notitie <script>";
        assert_eq!(
            render(
                "Could not save {title}: {count}",
                &[("title", private), ("count", "2")]
            ),
            "Could not save {count} — Mijn Nederlandse notitie <script>: 2"
        );
        assert_eq!(render("Unknown {argument}", &[]), "Unknown {argument}");
    }

    #[test]
    fn owned_message_matching_is_exact_and_preserves_arguments() {
        assert_eq!(
            capture_template(
                "Model {model} is niet beschikbaar.",
                "Model mijn-model is niet beschikbaar."
            ),
            Some(vec![("model".into(), "mijn-model")])
        );
        assert!(capture_template("Geen opname", "Mijn notitie zegt: Geen opname").is_none());
        assert!(capture_template("{message}", "Mijn eigen inhoud").is_none());
        assert!(capture_template("{message}.", "Mijn eigen inhoud.").is_none());
        assert!(capture_template("A {x}{y} B", "A tekst B").is_none());
        assert!(capture_template("A {x} B {y}", "A first B second B third").is_none());
        assert_eq!(
            localize_ui_text(InterfaceLanguage::English, "Een eigen notitie: Annuleren"),
            "Een eigen notitie: Annuleren"
        );
    }

    #[test]
    fn canonical_english_and_legacy_dutch_messages_follow_the_interface_language() {
        for source in ["Not configured", "Niet ingesteld"] {
            assert_eq!(
                localize_ui_text(InterfaceLanguage::English, source),
                "Not configured"
            );
            assert_eq!(
                localize_ui_text(InterfaceLanguage::Dutch, source),
                "Niet ingesteld"
            );
        }
        let detail = "Mijn eigen notitie: {count}";
        for source in [
            format!("Saving failed: {detail}"),
            format!("Bewaren mislukt: {detail}"),
        ] {
            assert_eq!(
                localize_ui_text(InterfaceLanguage::English, &source),
                format!("Saving failed: {detail}")
            );
            assert_eq!(
                localize_ui_text(InterfaceLanguage::Dutch, &source),
                format!("Bewaren mislukt: {detail}")
            );
        }
        assert_eq!(
            localize_ui_text(InterfaceLanguage::Dutch, "My note says: Not configured"),
            "My note says: Not configured"
        );
    }

    #[test]
    fn both_catalogs_cover_the_same_keys_and_placeholders() {
        let catalog = catalogs();
        assert_eq!(
            catalog.english.keys().collect::<Vec<_>>(),
            catalog.dutch.keys().collect::<Vec<_>>()
        );
        for (key, english) in &catalog.english {
            let names = |text: &str| {
                placeholders(text)
                    .iter()
                    .map(|(_, _, name)| (*name).to_owned())
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(
                names(english),
                names(&catalog.dutch[key]),
                "placeholder mismatch: {key}"
            );
            assert!(
                !english.trim().is_empty(),
                "empty English translation: {key}"
            );
            assert!(
                !catalog.dutch[key].trim().is_empty(),
                "empty Dutch translation: {key}"
            );
        }
        for (key, source) in &catalog.backend_sources {
            assert!(
                catalog.english.contains_key(key),
                "missing backend translation: {key}"
            );
            let names = |text: &str| {
                placeholders(text)
                    .iter()
                    .map(|(_, _, name)| (*name).to_owned())
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(
                names(source),
                names(&catalog.english[key]),
                "source placeholder mismatch: {key}"
            );
            assert!(
                literal_size(source) > 0,
                "unconstrained backend message: {key}"
            );
            assert!(
                placeholders(source).is_empty() || has_fixed_word(source),
                "backend message needs a fixed word: {key}"
            );
        }
    }

    #[test]
    fn canonical_recording_and_transcript_guards_have_exact_dutch_aliases() {
        for (source, dutch) in [
            (
                "the transcript contains lines that Pennel does not recognize",
                "het transcript bevat regels die Pennel niet herkent",
            ),
            (
                "the meeting is still recording or being finalized",
                "de meeting wordt nog opgenomen of afgerond",
            ),
        ] {
            assert_eq!(localize_ui_text(InterfaceLanguage::Dutch, source), dutch);
            assert_eq!(localize_ui_text(InterfaceLanguage::Dutch, dutch), dutch);
        }
    }

    #[test]
    #[should_panic(expected = "duplicate translation key")]
    fn duplicate_catalog_keys_are_rejected() {
        parse_strings(r#"{"duplicate":"first","duplicate":"second"}"#);
    }
}
