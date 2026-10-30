use std::collections::{HashMap, HashSet, VecDeque};

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::settings::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CleanupAssessment {
    pub accepted: bool,
    pub reason_code: &'static str,
}

impl CleanupAssessment {
    fn reject(reason_code: &'static str) -> Self {
        Self {
            accepted: false,
            reason_code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Number,
    Protected,
    Question,
}

#[derive(Debug, Clone)]
struct Token {
    text: String,
    kind: Kind,
    start: usize,
    end: usize,
}

pub(crate) fn assess_cleanup(
    raw: &str,
    cleaned: &str,
    style: Style,
    dictionary: &[String],
) -> CleanupAssessment {
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return CleanupAssessment::reject("empty_output");
    }
    if cleaned.chars().count() > raw.chars().count().saturating_mul(2).saturating_add(40) {
        return CleanupAssessment::reject("output_too_long");
    }
    let source = tokenize(raw, style);
    let mut candidate = tokenize(cleaned, style);
    // Matching is bounded rather than silently shortening a long dictation.
    if source.len() > 8_192 || candidate.len() > 8_192 {
        return CleanupAssessment::reject("guard_input_limit");
    }
    if candidate.is_empty() {
        return CleanupAssessment::reject("empty_content");
    }
    let source_questions = source.iter().filter(|t| t.kind == Kind::Question).count();
    let candidate_questions = candidate
        .iter()
        .filter(|t| t.kind == Kind::Question)
        .count();
    if source_questions == 0
        && candidate_questions == 1
        && candidate.last().is_some_and(|t| t.kind == Kind::Question)
        && interrogative(&source, style)
    {
        candidate.pop();
    } else if source_questions != candidate_questions {
        return CleanupAssessment::reject("question_intent_changed");
    }
    let dictionary = dictionary_signatures(dictionary);
    match equivalent(raw, &source, &candidate, style, &dictionary) {
        Some(true) => CleanupAssessment {
            accepted: true,
            reason_code: "preserved",
        },
        Some(false) => CleanupAssessment::reject("unsupported_content_change"),
        None => CleanupAssessment::reject("guard_work_limit"),
    }
}

fn prose_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '…' | '。' | '，' | '！')
}

fn word_joiner(c: char) -> bool {
    matches!(c, '\'' | '’' | '-' | '‐' | '_' | '/' | '\\' | '.' | '@')
}

fn numeric_joiner(c: char) -> bool {
    matches!(c, '.' | ',' | ':' | '/' | '-' | '–' | '—')
}

fn numeric_prefix(text: &str) -> bool {
    let unsigned = text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('+'))
        .or_else(|| text.strip_prefix('−'))
        .unwrap_or(text)
        .trim_start();
    unsigned.chars().next().is_some_and(|c| c.is_ascii_digit())
        || unsigned.starts_with(['.', ','])
            && unsigned[1..]
                .trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        || unsigned.split_whitespace().next().is_some_and(|word| {
            let word = word
                .trim_end_matches(|c| prose_punctuation(c) || c == '?')
                .to_lowercase();
            cardinal_en(&word.split('-').collect::<Vec<_>>()).is_some()
                || cardinal_nl(&[word.as_str()]).is_some()
        })
}

fn numeric_suffix(tokens: &[Token]) -> bool {
    tokens.last().is_some_and(|token| {
        token.kind == Kind::Number
            || token.kind == Kind::Word
                && (token.text == "hundred" || cardinal(std::slice::from_ref(token)).is_some())
    })
}

fn measurement_code(word: &str) -> bool {
    [
        "b", "KB", "MB", "GB", "TB", "PB", "KiB", "MiB", "GiB", "TiB", "m", "mm", "cm", "dm", "km",
        "um", "µm", "μm", "nm", "pm", "g", "mg", "kg", "ug", "µg", "μg", "L", "mL", "cL", "dL",
        "kL", "s", "ms", "us", "µs", "μs", "ns", "h", "Hz", "kHz", "MHz", "GHz", "W", "mW", "kW",
        "GW", "V", "mV", "kV", "A", "mA", "kA", "J", "kJ", "MJ", "N", "kN", "Pa", "kPa", "MPa",
        "bar", "mbar", "C", "F", "K",
    ]
    .iter()
    .any(|code| code.eq_ignore_ascii_case(word))
}

fn code_operator(c: char) -> bool {
    matches!(
        c,
        '=' | ':'
            | ','
            | ';'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '+'
            | '-'
            | '*'
            | '/'
            | '<'
            | '>'
            | '!'
            | '&'
            | '|'
            | '^'
            | '%'
    )
}

fn tokenize(text: &str, style: Style) -> Vec<Token> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < chars.len() {
        let (start, c) = chars[i];
        let next = chars.get(i + 1).map(|(_, c)| *c);
        if c.is_whitespace() {
            if matches!(c, '\n' | '\r') {
                line_start = true;
            }
            i += 1;
            continue;
        }
        let at_line_start = line_start;
        line_start = false;
        let after_marker = text[start + c.len_utf8()..].trim_start();
        let numeric_item = numeric_prefix(after_marker);
        if style != Style::Literal
            && matches!(c, '-' | '*' | '•')
            && at_line_start
            && next.is_some_and(char::is_whitespace)
            && !numeric_item
        {
            i += 1;
            continue;
        }
        let leading_decimal = matches!(c, '.' | ',') && next.is_some_and(|c| c.is_ascii_digit());
        let signed_decimal = matches!(c, '-' | '+' | '−')
            && next.is_some_and(|c| matches!(c, '.' | ','))
            && chars.get(i + 2).is_some_and(|(_, c)| c.is_ascii_digit());
        let numeric_separator =
            matches!(c, '.' | ',' | ':') && numeric_suffix(&tokens) && numeric_item;
        let numeric = c.is_ascii_digit()
            || leading_decimal
            || signed_decimal
            || matches!(c, '-' | '+' | '−') && next.is_some_and(|c| c.is_ascii_digit());
        let kind = if numeric {
            i += 1;
            while i < chars.len() {
                let current = chars[i].1;
                if current.is_ascii_digit()
                    || numeric_joiner(current)
                        && chars.get(i + 1).is_some_and(|(_, c)| c.is_ascii_digit())
                {
                    i += 1;
                } else {
                    break;
                }
            }
            if chars
                .get(i)
                .is_some_and(|(_, c)| c.is_alphabetic() || *c == '_')
            {
                while i < chars.len() {
                    let current = chars[i].1;
                    let next_alphanumeric =
                        chars.get(i + 1).is_some_and(|(_, c)| c.is_alphanumeric());
                    if current.is_alphanumeric()
                        || is_combining_mark(current)
                        || word_joiner(current) && next_alphanumeric
                        || current == '+'
                            && i > 0
                            && matches!(chars[i - 1].1, 'e' | 'E')
                            && next_alphanumeric
                    {
                        i += 1;
                    } else {
                        break;
                    }
                }
                Kind::Protected
            } else {
                Kind::Number
            }
        } else if c.is_alphabetic() || is_combining_mark(c) {
            let leading_identifier_mark =
                i > 0 && matches!(chars[i - 1].1, '_' | '/' | '\\' | '.' | '@');
            i += 1;
            while i < chars.len() {
                let current = chars[i].1;
                if current.is_alphanumeric()
                    || is_combining_mark(current)
                    || word_joiner(current)
                        && chars.get(i + 1).is_some_and(|(_, c)| c.is_alphanumeric())
                {
                    i += 1;
                } else {
                    break;
                }
            }
            let end = chars.get(i).map_or(text.len(), |(p, _)| *p);
            let word = &text[start..end];
            let adjacent_code = text[..start]
                .trim_end()
                .chars()
                .next_back()
                .is_some_and(code_operator)
                || text[end..]
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(code_operator);
            let code_keyword = matches!(
                word.to_ascii_lowercase().as_str(),
                "let"
                    | "const"
                    | "var"
                    | "fn"
                    | "def"
                    | "class"
                    | "struct"
                    | "enum"
                    | "return"
                    | "import"
                    | "pub"
                    | "static"
                    | "mut"
            ) && text
                .chars()
                .any(|c| matches!(c, '=' | '(' | '[' | '{' | ';'));
            let identifier = word
                .chars()
                .any(|c| matches!(c, '_' | '/' | '\\' | '.' | '@'))
                || measurement_code(word) && numeric_suffix(&tokens)
                || style == Style::Literal
                    && (word.chars().skip(1).any(char::is_uppercase)
                        || leading_identifier_mark
                        || adjacent_code
                        || code_keyword
                        || chars.get(i).is_some_and(|(_, c)| matches!(c, '(' | '[')));
            if identifier {
                Kind::Protected
            } else {
                Kind::Word
            }
        } else {
            i += 1;
            if c == '?' {
                Kind::Question
            } else if numeric_separator
                || c == ':' && next == Some('/')
                || style == Style::Literal && matches!(c, ';' | ':' | ',')
            {
                Kind::Protected
            } else if prose_punctuation(c) {
                continue;
            } else {
                Kind::Protected
            }
        };
        let end = chars.get(i).map_or(text.len(), |(p, _)| *p);
        let original = &text[start..end];
        let text = if kind == Kind::Word {
            original
                .nfc()
                .flat_map(char::to_lowercase)
                .map(|c| if c == '’' { '\'' } else { c })
                .collect()
        } else {
            original.nfc().collect()
        };
        tokens.push(Token {
            text,
            kind,
            start,
            end,
        });
    }
    tokens
}

fn removable(tokens: &[Token], i: usize, style: Style) -> bool {
    let token = &tokens[i];
    if style == Style::Literal && i > 0 {
        return false;
    }
    if token.text == "um"
        && i > 0
        && (tokens[i - 1].kind == Kind::Number
            || small_en(&tokens[i - 1].text).is_some()
            || small_nl(&tokens[i - 1].text).is_some())
    {
        return false;
    }
    token.kind == Kind::Word
        && (matches!(
            token.text.as_str(),
            "uh" | "um" | "uhm" | "umm" | "euh" | "eh" | "ehm" | "uhh"
        ) || token.text == "enzo" && tokens[i + 1..].iter().all(|t| t.kind == Kind::Question))
}

fn interrogative(source: &[Token], style: Style) -> bool {
    let words: Vec<&str> = source
        .iter()
        .enumerate()
        .filter(|(i, t)| t.kind == Kind::Word && !removable(source, *i, style))
        .take(2)
        .map(|(_, t)| t.text.as_str())
        .collect();
    matches!(
        words.as_slice(),
        [
            "what" | "where" | "when" | "why" | "who" | "how",
            "is" | "are" | "do" | "does" | "can" | "should"
        ] | [
            "can"
                | "could"
                | "would"
                | "will"
                | "should"
                | "do"
                | "does"
                | "did"
                | "have"
                | "has"
                | "is"
                | "are"
                | "was"
                | "were",
            "i" | "you" | "we" | "they" | "he" | "she" | "it"
        ] | [
            "wat" | "waar" | "wanneer" | "waarom" | "wie" | "hoe",
            "is" | "zijn" | "kan" | "kun" | "moet"
        ] | ["hoe", "gaat"]
            | [
                "kan"
                    | "kun"
                    | "kunt"
                    | "mag"
                    | "moet"
                    | "wil"
                    | "zou"
                    | "heb"
                    | "heeft"
                    | "is"
                    | "zijn",
                "ik" | "je"
                    | "jij"
                    | "u"
                    | "we"
                    | "wij"
                    | "ze"
                    | "zij"
                    | "hij"
                    | "het"
                    | "dit"
                    | "dat"
            ]
    )
}

fn negation(token: &Token) -> bool {
    token.kind == Kind::Word
        && (matches!(
            token.text.as_str(),
            "no" | "not"
                | "never"
                | "neither"
                | "nor"
                | "none"
                | "nothing"
                | "nobody"
                | "without"
                | "cannot"
                | "niet"
                | "geen"
                | "nooit"
                | "niemand"
                | "niets"
                | "zonder"
                | "nee"
        ) || token.text.ends_with("n't"))
}

fn sensitive_word_boundary(token: &Token) -> bool {
    let word = signature(&token.text).unwrap_or_else(|| token.text.clone());
    negation(token)
        || small_en(&word).is_some()
        || small_nl(&word).is_some()
        || [
            "all", "any", "some", "every", "each", "both", "either", "few", "many", "much", "most",
            "several", "more", "less", "enough", "another", "other", "such", "just", "even", "can",
            "could", "may", "might", "must", "shall", "should", "will", "would", "do", "does",
            "did", "be", "am", "is", "are", "was", "were", "been", "being", "have", "has", "had",
            "alle", "alles", "elk", "elke", "ieder", "iedere", "sommige", "beide", "veel",
            "weinig", "meer", "minder", "meest", "genoeg", "ander", "andere", "slechts", "kan",
            "kun", "kunt", "kunnen", "wil", "willen", "mag", "mogen", "moet", "moeten", "zou",
            "zouden", "zal", "zullen", "ben", "bent", "zijn", "waren", "wordt", "worden", "heb",
            "hebt", "heeft", "hebben", "hadden",
        ]
        .contains(&word.as_str())
        || matches!(
            word.as_str(),
            "a" | "an"
                | "the"
                | "i"
                | "me"
                | "my"
                | "mine"
                | "myself"
                | "you"
                | "your"
                | "yours"
                | "yourself"
                | "we"
                | "us"
                | "our"
                | "ours"
                | "ourselves"
                | "they"
                | "them"
                | "their"
                | "theirs"
                | "themselves"
                | "he"
                | "him"
                | "his"
                | "himself"
                | "she"
                | "her"
                | "hers"
                | "herself"
                | "it"
                | "its"
                | "itself"
                | "this"
                | "that"
                | "these"
                | "those"
                | "to"
                | "for"
                | "from"
                | "in"
                | "on"
                | "at"
                | "by"
                | "with"
                | "of"
                | "about"
                | "before"
                | "after"
                | "during"
                | "since"
                | "until"
                | "over"
                | "under"
                | "between"
                | "through"
                | "into"
                | "onto"
                | "near"
                | "beside"
                | "beyond"
                | "out"
                | "off"
                | "up"
                | "down"
                | "and"
                | "or"
                | "but"
                | "yet"
                | "so"
                | "because"
                | "if"
                | "when"
                | "while"
                | "although"
                | "though"
                | "unless"
                | "as"
                | "than"
                | "which"
                | "who"
                | "whose"
                | "now"
                | "here"
                | "there"
                | "then"
                | "today"
                | "tomorrow"
                | "yesterday"
                | "tonight"
                | "already"
                | "still"
                | "soon"
                | "later"
                | "early"
                | "recently"
                | "currently"
                | "immediately"
                | "everywhere"
                | "nowhere"
                | "anywhere"
                | "somewhere"
                | "hundred"
                | "thousand"
                | "million"
                | "de"
                | "het"
                | "een"
                | "ik"
                | "mij"
                | "mijn"
                | "jij"
                | "jou"
                | "je"
                | "jouw"
                | "u"
                | "uw"
                | "hij"
                | "hem"
                | "zijn"
                | "zij"
                | "haar"
                | "ze"
                | "wij"
                | "ons"
                | "onze"
                | "jullie"
                | "hun"
                | "hen"
                | "dit"
                | "dat"
                | "deze"
                | "die"
                | "naar"
                | "voor"
                | "van"
                | "op"
                | "aan"
                | "bij"
                | "met"
                | "zonder"
                | "onder"
                | "tussen"
                | "door"
                | "tot"
                | "uit"
                | "na"
                | "om"
                | "tijdens"
                | "sinds"
                | "tegen"
                | "langs"
                | "achter"
                | "binnen"
                | "buiten"
                | "boven"
                | "beneden"
                | "en"
                | "maar"
                | "want"
                | "dus"
                | "omdat"
                | "als"
                | "terwijl"
                | "hoewel"
                | "tenzij"
                | "zoals"
                | "dan"
                | "wel"
                | "nu"
                | "hier"
                | "daar"
                | "toen"
                | "vandaag"
                | "morgen"
                | "overmorgen"
                | "gisteren"
                | "eergisteren"
                | "straks"
                | "binnenkort"
                | "overal"
                | "nergens"
                | "ergens"
                | "soms"
                | "altijd"
                | "vaak"
                | "reeds"
                | "al"
                | "nog"
                | "pas"
                | "weer"
                | "ook"
                | "alleen"
                | "juist"
                | "niet"
                | "geen"
                | "nooit"
                | "niemand"
                | "niets"
                | "nee"
                | "honderd"
                | "duizend"
                | "miljoen"
        )
}

fn signature(text: &str) -> Option<String> {
    let mut result = String::new();
    for c in text.nfd().filter(|c| !is_combining_mark(*c)) {
        if c.is_alphanumeric() {
            result.extend(c.to_lowercase());
        } else if !c.is_whitespace() {
            return None;
        }
    }
    (!result.is_empty()).then_some(result)
}

fn dictionary_signatures(dictionary: &[String]) -> HashSet<String> {
    let mut counts = HashMap::new();
    for term in dictionary {
        if let Some(key) = signature(term) {
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(|(key, count)| (count == 1).then_some(key))
        .collect()
}

fn dictionary_spans(
    tokens: &[Token],
    start: usize,
    dictionary: &HashSet<String>,
) -> Vec<(usize, String)> {
    let mut key = String::new();
    let mut matches = Vec::new();
    let mut sensitive = false;
    for (i, token) in tokens.iter().enumerate().skip(start).take(8) {
        if token.kind != Kind::Word {
            break;
        }
        let current_sensitive = sensitive_word_boundary(token);
        // A preferred spelling cannot authorize erasing a meaningful word boundary.
        if i > start && (sensitive || current_sensitive) {
            break;
        }
        sensitive |= current_sensitive;
        let Some(part) = signature(&token.text) else {
            break;
        };
        key.push_str(&part);
        if key.len() > 256 {
            break;
        }
        if dictionary.contains(&key) {
            matches.push((i + 1, key.clone()));
        }
    }
    matches
}

fn equivalent(
    raw: &str,
    source: &[Token],
    candidate: &[Token],
    style: Style,
    dictionary: &HashSet<String>,
) -> Option<bool> {
    let mut queue = VecDeque::from([(0, 0)]);
    let mut seen = HashSet::new();
    while let Some((i, j)) = queue.pop_front() {
        if !seen.insert((i, j)) {
            continue;
        }
        if seen.len() > 100_000 {
            return None;
        }
        if i == source.len() && j == candidate.len() {
            return Some(true);
        }
        if i == source.len() {
            continue;
        }
        if j < candidate.len()
            && source[i].text == candidate[j].text
            && source[i].kind == candidate[j].kind
        {
            queue.push_back((i + 1, j + 1));
        }
        if removable(source, i, style) {
            queue.push_back((i + 1, j));
        }
        if style == Style::Literal {
            continue;
        }
        if j < candidate.len() && candidate[j].kind == Kind::Number {
            for end in i + 1..=(i + 6).min(source.len()) {
                if let Some(value) = cardinal(&source[i..end])
                    && candidate[j].text == value.to_string()
                {
                    queue.push_back((end, j + 1));
                }
            }
        }
        if j < candidate.len() {
            let output_terms = dictionary_spans(candidate, j, dictionary);
            for (end, key) in dictionary_spans(source, i, dictionary) {
                for (output_end, output_key) in &output_terms {
                    if *output_key == key {
                        queue.push_back((end, *output_end));
                    }
                }
            }
        }
        for replacement in correction_replacements(raw, source, i, dictionary) {
            queue.push_back((replacement, j));
        }
    }
    Some(false)
}

fn small_en(word: &str) -> Option<u16> {
    [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ]
    .iter()
    .position(|w| *w == word)
    .map(|v| v as u16)
    .or_else(|| {
        [
            "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
        ]
        .iter()
        .position(|w| *w == word)
        .map(|v| (v as u16 + 2) * 10)
    })
}

fn sub100_en(words: &[&str]) -> Option<u16> {
    match words {
        [word] => small_en(word),
        [tens, unit] => {
            let tens = small_en(tens)?;
            let unit = small_en(unit)?;
            (tens >= 20 && tens % 10 == 0 && (1..=9).contains(&unit)).then_some(tens + unit)
        }
        _ => None,
    }
}

fn cardinal_en(words: &[&str]) -> Option<u16> {
    if let [hundreds, "hundred", rest @ ..] = words {
        let hundreds = small_en(hundreds)?;
        if !(1..=9).contains(&hundreds) {
            return None;
        }
        let rest = if let Some(suffix) = rest.strip_prefix(&["and"]) {
            if suffix.is_empty() {
                return None;
            }
            suffix
        } else {
            rest
        };
        let suffix = if rest.is_empty() { 0 } else { sub100_en(rest)? };
        hundreds.checked_mul(100)?.checked_add(suffix)
    } else {
        sub100_en(words)
    }
}

fn small_nl(word: &str) -> Option<u16> {
    [
        "nul",
        "een",
        "twee",
        "drie",
        "vier",
        "vijf",
        "zes",
        "zeven",
        "acht",
        "negen",
        "tien",
        "elf",
        "twaalf",
        "dertien",
        "veertien",
        "vijftien",
        "zestien",
        "zeventien",
        "achttien",
        "negentien",
    ]
    .iter()
    .position(|w| *w == word)
    .map(|v| v as u16)
    .or_else(|| {
        [
            "twintig", "dertig", "veertig", "vijftig", "zestig", "zeventig", "tachtig", "negentig",
        ]
        .iter()
        .position(|w| *w == word)
        .map(|v| (v as u16 + 2) * 10)
    })
}

fn sub100_nl(word: &str) -> Option<u16> {
    if let Some(value) = small_nl(word) {
        return Some(value);
    }
    for tens in [
        "twintig", "dertig", "veertig", "vijftig", "zestig", "zeventig", "tachtig", "negentig",
    ] {
        if let Some(unit) = word.strip_suffix(tens).and_then(|s| s.strip_suffix("en")) {
            let unit = small_nl(unit)?;
            return (1..=9).contains(&unit).then_some(small_nl(tens)? + unit);
        }
    }
    None
}

fn cardinal_nl(words: &[&str]) -> Option<u16> {
    let compound: String = words.iter().copied().collect();
    // Accent folding applies only inside recognized Dutch cardinal grammar.
    let compound: String = compound.nfd().filter(|c| !is_combining_mark(*c)).collect();
    if let Some((prefix, suffix)) = compound.split_once("honderd") {
        let hundreds = if prefix.is_empty() {
            1
        } else {
            small_nl(prefix)?
        };
        if !(1..=9).contains(&hundreds) {
            return None;
        }
        let suffix = if let Some(suffix) = suffix.strip_prefix("en") {
            if suffix.is_empty() {
                return None;
            }
            suffix
        } else {
            suffix
        };
        let value = if suffix.is_empty() {
            0
        } else {
            sub100_nl(suffix)?
        };
        hundreds.checked_mul(100)?.checked_add(value)
    } else {
        sub100_nl(&compound)
    }
}

fn cardinal(tokens: &[Token]) -> Option<u16> {
    if tokens.is_empty() || tokens.iter().any(|t| t.kind != Kind::Word) {
        return None;
    }
    let joined: String = tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let words: Vec<&str> = joined.split([' ', '-']).collect();
    cardinal_en(&words)
        .or_else(|| cardinal_nl(&words))
        .filter(|v| *v <= 999)
}

fn weekday(word: &str) -> Option<&'static str> {
    if [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ]
    .contains(&word)
    {
        Some("weekday_en")
    } else if [
        "maandag",
        "dinsdag",
        "woensdag",
        "donderdag",
        "vrijdag",
        "zaterdag",
        "zondag",
    ]
    .contains(&word)
    {
        Some("weekday_nl")
    } else if ["vandaag", "morgen", "overmorgen", "gisteren", "eergisteren"].contains(&word) {
        Some("relative_nl")
    } else if ["today", "tomorrow", "yesterday"].contains(&word) {
        Some("relative_en")
    } else {
        None
    }
}

fn value_class(
    raw: &str,
    tokens: &[Token],
    start: usize,
    end: usize,
    dictionary: &HashSet<String>,
) -> Option<String> {
    let span = &tokens[start..end];
    if span.len() == 1
        && span[0].kind == Kind::Word
        && let Some(class) = weekday(&span[0].text)
    {
        return Some(class.into());
    }
    if span.len() == 1 && span[0].kind == Kind::Number || cardinal(span).is_some() {
        return Some("number".into());
    }
    if span.len() >= 2
        && matches!(
            span.last()?.text.as_str(),
            "euro"
                | "euros"
                | "dollar"
                | "dollars"
                | "percent"
                | "procent"
                | "eur"
                | "usd"
                | "€"
                | "$"
                | "%"
        )
    {
        let value = &span[..span.len() - 1];
        if value.len() == 1 && value[0].kind == Kind::Number || cardinal(value).is_some() {
            return Some(format!("amount_{}", span.last()?.text));
        }
    }
    if dictionary_spans(tokens, start, dictionary)
        .iter()
        .any(|(matched_end, _)| *matched_end == end)
    {
        return Some("name".into());
    }
    let named_context = start == 0
        || matches!(
            tokens[start - 1].text.as_str(),
            "to" | "naar" | "for" | "voor" | "from" | "van" | "with" | "met" | "dear" | "beste"
        );
    if named_context
        && span.len() <= 4
        && span.iter().all(|token| {
            token.kind == Kind::Word
                && raw[token.start..token.end]
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase)
                && !matches!(
                    token.text.as_str(),
                    "i" | "you"
                        | "we"
                        | "he"
                        | "she"
                        | "it"
                        | "they"
                        | "ik"
                        | "jij"
                        | "je"
                        | "u"
                        | "wij"
                        | "hij"
                        | "zij"
                        | "ze"
                        | "het"
                )
        })
    {
        return Some("name".into());
    }
    None
}

fn correction_replacements(
    raw: &str,
    tokens: &[Token],
    start: usize,
    dictionary: &HashSet<String>,
) -> Vec<usize> {
    let mut replacements = Vec::new();
    for marker in start + 1..=(start + 8).min(tokens.len().saturating_sub(1)) {
        if !matches!(tokens[marker].text.as_str(), "no" | "nee") {
            continue;
        }
        let explicit = tokens.get(marker + 1).is_some_and(|t| {
            matches!(
                (tokens[marker].text.as_str(), t.text.as_str()),
                ("no", "wait") | ("nee", "wacht")
            )
        });
        let replacement = marker + if explicit { 2 } else { 1 };
        if replacement >= tokens.len() {
            continue;
        }
        let before_comma = raw[tokens[marker - 1].end..tokens[marker].start].contains(',');
        let delimited =
            before_comma && raw[tokens[marker].end..tokens[replacement].start].contains(',');
        let Some(class) = value_class(raw, tokens, start, marker, dictionary) else {
            continue;
        };
        if !explicit && !delimited && !(before_comma && class == "relative_nl") {
            continue;
        }
        for end in replacement + 1..=(replacement + 8).min(tokens.len()) {
            let replacement_class = value_class(raw, tokens, replacement, end, dictionary);
            let named_replacement = class == "name"
                && tokens[replacement..end].iter().all(|token| {
                    token.kind == Kind::Word
                        && raw[token.start..token.end]
                            .chars()
                            .next()
                            .is_some_and(char::is_uppercase)
                });
            if replacement_class.as_ref() == Some(&class) || named_replacement {
                replacements.push(replacement);
            }
        }
    }
    replacements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted(raw: &str, candidate: &str) -> bool {
        assess_cleanup(raw, candidate, Style::Neutral, &[]).accepted
    }

    #[test]
    fn benign_formatting_preserves_order_and_content() {
        for (raw, candidate) in [
            ("uh send the draft to Sophie", "Send the draft to Sophie."),
            ("euh stuur de mail naar Jan", "Stuur de mail naar Jan."),
            (
                "first send the draft then call Jan",
                "First send the draft.\nThen call Jan.",
            ),
            ("buy apples and pears", "Buy\n- apples\n- and pears."),
            ("I like this proposal", "I like this proposal."),
            ("uh I like this proposal", "Uh, I like this proposal."),
            ("don't send the draft", "Don’t send the draft."),
            ("kun je het verslag sturen", "Kun je het verslag sturen?"),
            ("what is the deadline", "What is the deadline?"),
            ("schrijf een mail naar Jan", "Schrijf een mail naar Jan."),
        ] {
            assert!(accepted(raw, candidate), "rejected {raw:?} → {candidate:?}");
        }
    }

    #[test]
    fn source_spoken_cardinals_can_become_equal_digits_only() {
        for (raw, candidate) in [
            ("send fifty euro to Jan", "Send 50 euro to Jan."),
            (
                "send one hundred and fifty euro to Jan",
                "Send 150 euro to Jan.",
            ),
            ("send twenty-one euro to Jan", "Send 21 euro to Jan."),
            ("send nine hundred ninety nine euro", "Send 999 euro."),
            ("stuur vijftig euro naar Jan", "Stuur 50 euro naar Jan."),
            (
                "stuur tweeëntwintig euro naar Jan",
                "Stuur 22 euro naar Jan.",
            ),
            ("stuur negenhonderdnegenennegentig euro", "Stuur 999 euro."),
            (
                "stuur negen honderd negen en negentig euro",
                "Stuur 999 euro.",
            ),
            ("stuur nul euro", "Stuur 0 euro."),
        ] {
            assert!(accepted(raw, candidate), "rejected {raw:?} → {candidate:?}");
        }
        for (raw, candidate) in [
            ("send fifty euro", "Send 500 euro."),
            ("send fifty euro", "Send vijftig euro."),
            ("send fifty euro", "Send +50 euro."),
            ("send fifty euro", "Send 050 euro."),
            ("send one thousand euro", "Send 1000 euro."),
            ("send one point five euro", "Send 1.5 euro."),
            ("stuur een komma vijf euro", "Stuur 1,5 euro."),
        ] {
            assert!(
                !accepted(raw, candidate),
                "accepted {raw:?} → {candidate:?}"
            );
        }
    }

    #[test]
    fn dictionary_spelling_requires_the_same_complete_letters() {
        let dictionary = vec!["OpenAI".into(), "Café".into(), "Sarah".into()];
        assert!(
            assess_cleanup(
                "open ai bouwt dit",
                "OpenAI bouwt dit.",
                Style::Neutral,
                &dictionary
            )
            .accepted
        );
        assert!(
            assess_cleanup(
                "meet at the cafe",
                "Meet at the Café.",
                Style::Formal,
                &dictionary
            )
            .accepted
        );
        assert!(
            !assess_cleanup(
                "send it to Sara",
                "Send it to Sarah.",
                Style::Neutral,
                &dictionary
            )
            .accepted
        );
        assert!(!assess_cleanup("ask Sarah", "Ask Sara.", Style::Neutral, &dictionary).accepted);
        assert!(
            !assess_cleanup(
                "meet at the cafe",
                "Meet at the Café.",
                Style::Neutral,
                &["Cafe".into(), "Café".into()]
            )
            .accepted
        );
        assert!(!accepted("meet at the cafe", "Meet at the Café."));
        assert!(
            assess_cleanup(
                "use kube ctl",
                "Use kubectl.",
                Style::Neutral,
                &["kubectl".into()]
            )
            .accepted
        );
        assert!(
            assess_cleanup(
                "the file is here",
                "Thé file is here.",
                Style::Neutral,
                &["Thé".into()]
            )
            .accepted
        );
    }

    #[test]
    fn bounded_spoken_corrections_keep_the_replacement() {
        for (raw, candidate) in [
            ("send it Tuesday, no, Thursday", "Send it Thursday."),
            ("send it Tuesday no wait Thursday", "Send it Thursday."),
            (
                "stuur het dinsdag nee wacht donderdag",
                "Stuur het donderdag.",
            ),
            ("euh ik kom morgen, nee overmorgen", "Ik kom overmorgen."),
            ("stuur vijftig nee wacht zestig euro", "Stuur 60 euro."),
            ("send fifty euro, no wait, sixty euro", "Send 60 euro."),
            ("send it to Sophie, no wait, Sarah", "Send it to Sarah."),
        ] {
            assert!(accepted(raw, candidate), "rejected {raw:?} → {candidate:?}");
        }
        for (raw, candidate) in [
            ("send it Tuesday no Thursday", "Send it Thursday."),
            ("stuur het dinsdag nee donderdag", "Stuur het donderdag."),
            ("do not send it Thursday", "Send it Thursday."),
            ("send it Tuesday no wait donderdag", "Send it donderdag."),
            (
                "send fifty euro no wait sixty dollars",
                "Send sixty dollars.",
            ),
            ("I no wait you approve it", "You approve it."),
        ] {
            assert!(
                !accepted(raw, candidate),
                "accepted {raw:?} → {candidate:?}"
            );
        }
    }

    #[test]
    fn critical_punctuation_and_question_boundaries_are_protected() {
        for (raw, candidate) in [
            ("send -50 euro", "Send 50 euro."),
            ("use 50 um", "Use 50."),
            ("use fifty um", "Use 50."),
            ("- 50 euro", "50 euro."),
            ("- fifty euro", "50 euro."),
            ("- Twenty-One euro", "21 euro."),
            ("- tweehonderd euro", "200 euro."),
            ("send 50,5 euro", "Send 50.5 euro."),
            ("meet 03/04 at 10:30", "Meet 04/03 at 10:30."),
            ("keep 5–10 items", "Keep 510 items."),
            ("pay €50", "Pay 50."),
            ("why now? is it ready?", "Why now is it ready?"),
            ("is it ready? why now?", "Is it ready why? Now?"),
            ("the draft is ready", "The draft is ready?"),
            ("go to http://example.com", "Go to http//example.com."),
            ("use draft_v2.txt", "Use draftv2.txt."),
        ] {
            assert!(
                !accepted(raw, candidate),
                "accepted {raw:?} → {candidate:?}"
            );
        }
    }

    #[test]
    fn literal_keeps_technical_spelling_and_number_words() {
        assert!(
            assess_cleanup(
                "uh use fooBar in draft_v2.txt",
                "Use fooBar in draft_v2.txt.",
                Style::Literal,
                &[]
            )
            .accepted
        );
        for (raw, candidate) in [
            ("use fooBar", "Use foobar."),
            ("use _Foo", "Use _foo."),
            ("call foo()", "Call Foo()."),
            ("use uh as the identifier", "Use as the identifier."),
            ("use draft_v2.txt", "Use draft_v3.txt."),
            ("send fifty euro", "Send 50 euro."),
            ("open ai", "OpenAI"),
            ("let x = 1;", "Let x = 1"),
        ] {
            assert!(!assess_cleanup(raw, candidate, Style::Literal, &["OpenAI".into()]).accepted);
        }
    }

    #[test]
    fn rejects_reviewed_question_negation_and_literal_corruptions() {
        let cases = [
            (
                "Will sends the draft",
                "Will sends the draft?",
                Style::Neutral,
                vec![],
            ),
            ("x: int = 1", "x int = 1", Style::Literal, vec![]),
            ("(1, 2)", "(1 2)", Style::Literal, vec![]),
            ("let x = 1", "let X = 1", Style::Literal, vec![]),
            (
                "I am not able",
                "I am notable.",
                Style::Neutral,
                vec!["notable".into()],
            ),
            (
                "dat is niet waar",
                "Dat is nietwaar.",
                Style::Neutral,
                vec!["nietwaar".into()],
            ),
            (
                "The files are now here",
                "The files are nowhere.",
                Style::Neutral,
                vec!["nowhere".into()],
            ),
            (
                "They are a part",
                "They are apart.",
                Style::Neutral,
                vec!["apart".into()],
            ),
            (
                "the rapist",
                "therapist",
                Style::Neutral,
                vec!["therapist".into()],
            ),
        ];
        let accepted: Vec<_> = cases
            .into_iter()
            .filter(|(raw, candidate, style, dictionary)| {
                assess_cleanup(raw, candidate, *style, dictionary).accepted
            })
            .map(|(raw, _, _, _)| raw)
            .collect();
        assert!(accepted.is_empty(), "accepted corruptions for {accepted:?}");
    }

    #[test]
    fn protects_leading_decimals_and_spaced_numeric_separators() {
        let corruptions = [
            ("pay .5 euro", "Pay 5 euro."),
            ("pay ,5 euro", "Pay 5 euro."),
            ("pay -.5 euro", "Pay - 5 euro."),
            ("pay +,5 euro", "Pay + 5 euro."),
            ("meet at 5 : 30", "Meet at 5 30."),
            ("pay 5 . 30 euro", "Pay 5 30 euro."),
            ("pay 5 , 30 euro", "Pay 5 30 euro."),
            ("pay one . five euro", "Pay 1 5 euro."),
            ("meet at five : thirty", "Meet at 5 30."),
            ("pay 5 : -.5 euro", "Pay 5 -.5 euro."),
            ("- .5 euro", ".5 euro."),
        ];
        let accepted_corruptions: Vec<_> = corruptions
            .into_iter()
            .filter(|(raw, candidate)| accepted(raw, candidate))
            .map(|(raw, _)| raw)
            .collect();
        assert!(
            accepted_corruptions.is_empty(),
            "accepted numeric corruptions for {accepted_corruptions:?}"
        );
        for (raw, candidate) in [
            ("pay .5 euro", "Pay .5 euro."),
            ("pay ,5 euro", "Pay ,5 euro."),
            ("pay -.5 euro", "Pay -.5 euro."),
            ("pay +,5 euro", "Pay +,5 euro."),
            ("meet at 5 : 30", "Meet at 5 : 30."),
            ("pay 5 . 30 euro", "Pay 5 . 30 euro."),
            ("pay 5 , 30 euro", "Pay 5 , 30 euro."),
        ] {
            assert!(
                accepted(raw, candidate),
                "rejected unchanged numeric form {raw:?}"
            );
        }
    }

    #[test]
    fn protects_case_sensitive_units_and_numeric_identifiers() {
        let corruptions = [
            ("ship 5 MB", "Ship 5 Mb."),
            ("ship five mL", "Ship 5 ml."),
            ("ship 5 m", "Ship 5 M."),
            ("wait 5 s", "Wait 5 S."),
            ("send to 3M", "Send to 3 m."),
            ("use 1e3", "Use 1 e3."),
            ("ship 5MB", "Ship 5mb."),
        ];
        let accepted_corruptions: Vec<_> = corruptions
            .into_iter()
            .filter(|(raw, candidate)| accepted(raw, candidate))
            .map(|(raw, _)| raw)
            .collect();
        assert!(
            accepted_corruptions.is_empty(),
            "accepted unit/identifier corruptions for {accepted_corruptions:?}"
        );
        for (raw, candidate) in [
            ("ship 5 MB", "Ship 5 MB."),
            ("ship five mL", "Ship 5 mL."),
            ("send to 3M", "Send to 3M."),
            ("use 1e3", "Use 1e3."),
            ("ship 5MB", "Ship 5MB."),
            ("pay fifty euro", "Pay 50 Euro."),
            ("a simple sentence", "A simple sentence."),
            ("um send the draft", "Send the draft."),
        ] {
            assert!(
                accepted(raw, candidate),
                "rejected unchanged unit/identifier form {raw:?}"
            );
        }
    }
}
