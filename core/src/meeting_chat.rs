use crate::llm::ChatMessage;
use crate::meeting_index::{Passage, PassageKind};
use crate::meetings::timestamp;

pub const WHOLE_MEETING_CHARS: usize = 60_000;
pub const MEETING_PASSAGES: u32 = 25;
pub const ALL_MEETINGS_PASSAGES: u32 = 12;
pub const ASK_TIMEOUT_SECS: u64 = 60;
const HISTORY_TURNS: usize = 6;
const HISTORY_ANSWER_CHARS: usize = 2_000;

const STOP_WORDS: &[&str] = &[
    "aan", "als", "bij", "dan", "dat", "die", "dit", "een", "en", "er", "het", "hoe", "hun", "ik", "in", "is", "je",
    "jij", "maar", "met", "mij", "naar", "niet", "nog", "of", "om", "ook", "op", "over", "te", "tot", "uit", "van",
    "voor", "wat", "waar", "wanneer", "was", "waren", "we", "welke", "wie", "wij", "zijn", "ze", "zij", "hebben",
    "heeft", "had", "werd", "wordt", "kan", "moet", "zou", "the", "and", "for", "are", "was", "what", "when",
    "where", "which", "who", "why", "how", "did", "does", "about", "with", "that", "this", "have", "has", "our",
    "you", "they", "from", "into", "meeting", "meetings",
];

const SYSTEM_PROMPT: &str = "You answer questions about the user's meetings.
Use only the numbered sources. Cite every claim with its source number in square brackets, like [2].
If the sources do not contain the answer, say so plainly instead of guessing.
Answer in the language of the question, concisely, in Markdown.";

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatTurn {
    pub question: String,
    pub answer: String,
}

/// A passage the answer cites, with the `[number]` used in the answer text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Citation {
    pub number: u32,
    pub passage: Passage,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MeetingAnswer {
    pub text: String,
    pub citations: Vec<Citation>,
}

pub fn search_terms(question: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in question.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase) {
        if word.chars().count() >= 3 && !STOP_WORDS.contains(&word.as_str()) && !terms.contains(&word) {
            terms.push(word);
        }
    }
    terms
}

fn source_label(passage: &Passage) -> String {
    let date = passage.start_iso.get(..10).unwrap_or(&passage.start_iso);
    let place = match (passage.kind, passage.start_ms) {
        (PassageKind::Transcript, Some(ms)) => format!("transcript {}", timestamp(ms)),
        (PassageKind::Transcript, None) => "transcript".to_owned(),
        (PassageKind::Notes, _) => "notities".to_owned(),
        (PassageKind::Summary, _) => "samenvatting".to_owned(),
    };
    format!("{} ({date}, {place})", passage.title)
}

pub fn chat_messages(question: &str, history: &[ChatTurn], sources: &[Passage]) -> Vec<ChatMessage> {
    let numbered: String = sources
        .iter()
        .enumerate()
        .map(|(i, p)| format!("[{}] {}\n{}\n", i + 1, source_label(p), p.text))
        .collect::<Vec<_>>()
        .join("\n");
    let context = if numbered.is_empty() { "(geen bronnen gevonden)".to_owned() } else { numbered };
    let mut messages = vec![ChatMessage { role: "system", content: format!("{SYSTEM_PROMPT}\n\nSources:\n{context}") }];
    for turn in &history[history.len().saturating_sub(HISTORY_TURNS)..] {
        messages.push(ChatMessage { role: "user", content: turn.question.clone() });
        messages.push(ChatMessage {
            role: "assistant",
            content: turn.answer.chars().take(HISTORY_ANSWER_CHARS).collect(),
        });
    }
    messages.push(ChatMessage { role: "user", content: question.to_owned() });
    messages
}

/// Sources the answer refers to as `[n]`, ordered by number.
pub fn cited(answer: &str, sources: &[Passage]) -> Vec<Citation> {
    let mut picked: Vec<usize> = Vec::new();
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        for part in rest[..close].split(',') {
            if let Ok(n) = part.trim().parse::<usize>()
                && (1..=sources.len()).contains(&n)
                && !picked.contains(&(n - 1))
            {
                picked.push(n - 1);
            }
        }
        rest = &rest[close + 1..];
    }
    picked.sort_unstable();
    picked.into_iter().map(|i| Citation { number: i as u32 + 1, passage: sources[i].clone() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(title: &str, kind: PassageKind, start_ms: Option<u64>, text: &str) -> Passage {
        Passage {
            meeting_id: title.into(),
            title: title.into(),
            start_iso: "2026-10-20T09:00:00+02:00".into(),
            kind,
            start_ms,
            text: text.into(),
            snippet: String::new(),
        }
    }

    #[test]
    fn search_terms_drop_stop_words() {
        let terms = search_terms("Wat hebben we beslist over het budget?");
        assert_eq!(terms, ["beslist", "budget"]);
    }

    #[test]
    fn chat_prompt_numbers_sources() {
        let sources = [
            passage("Sync", PassageKind::Transcript, Some(754_000), "Anderen: budget is rond"),
            passage("Plan", PassageKind::Notes, None, "budget herzien"),
        ];
        let messages = chat_messages("Wat met het budget?", &[], &sources);
        assert_eq!(messages.len(), 2);
        assert!(messages[0].content.contains("[n]") || messages[0].content.contains("[2]"));
        assert!(messages[0].content.contains("[1] Sync (2026-10-20, transcript 12:34)\nAnderen: budget is rond"));
        assert!(messages[0].content.contains("[2] Plan (2026-10-20, notities)"));
        assert_eq!(messages[1].content, "Wat met het budget?");
    }

    #[test]
    fn history_becomes_alternating_turns() {
        let history = [ChatTurn { question: "Wie was er?".into(), answer: "Jan [1].".into() }];
        let roles: Vec<&str> = chat_messages("En wat zei hij?", &history, &[]).iter().map(|m| m.role).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user"]);
    }

    #[test]
    fn history_is_capped() {
        let history: Vec<ChatTurn> =
            (0..10).map(|i| ChatTurn { question: format!("vraag {i}"), answer: "x".repeat(5_000) }).collect();
        let messages = chat_messages("nu", &history, &[]);
        assert_eq!(messages.len(), 1 + 2 * HISTORY_TURNS + 1);
        assert_eq!(messages[1].content, "vraag 4");
        assert_eq!(messages[2].content.len(), HISTORY_ANSWER_CHARS);
    }

    #[test]
    fn citations_out_of_range_ignored() {
        let sources: Vec<Passage> =
            ["a", "b", "c"].iter().map(|t| passage(t, PassageKind::Notes, None, t)).collect();
        let found = cited("Ja [3] en [7], ook [2, 3].", &sources);
        assert_eq!(found.iter().map(|c| c.number).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(found[0].passage, sources[1]);
        assert!(cited("Geen idee", &sources).is_empty());
        assert!(cited("kapot [", &sources).is_empty());
    }
}
