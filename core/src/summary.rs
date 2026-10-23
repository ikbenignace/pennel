use crate::llm::ChatMessage;
use crate::meetings::{timestamp, Segment, Speaker};

pub(crate) const MAP_REDUCE_ABOVE: usize = 40_000;
pub(crate) const BLOCK_CHARS: usize = 30_000;

const SYSTEM_PROMPT: &str = "You write meeting notes from a transcript. \"Ik\" is the user; \"Anderen\" are the other participants.
Write in the language the meeting was held in (Dutch or English). Use Markdown with exactly these sections:
## Samenvatting
## Beslissingen
## Actiepunten
Under Actiepunten use \"- [ ] Person — action\" lines; write \"- Geen\" when there are none. Never invent facts that are not in the transcript or notes.";

const MODE_SUMMARY: &str = "Summarize the meeting in short bullet points under Samenvatting.";
const MODE_NOTES: &str = "The user took the notes below during the meeting. Under Samenvatting, keep every line of the user's notes verbatim (you may reorder them) and add missing important details from the transcript as separate bullet lines.";
const MAP_PROMPT: &str = "Summarize this part of a meeting transcript as concise bullet points: facts, decisions and action items with who is responsible. Keep the language of the meeting. \"Ik\" is the user.";

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MeetingTemplate {
    pub id: String,
    pub name: String,
}

const TEMPLATES: &[(&str, &str, &str)] = &[
    ("algemeen", "Algemeen", "Focus on what was discussed and agreed."),
    ("een-op-een", "1-op-1", "This is a 1-on-1. Include feedback, wellbeing and personal follow-ups."),
    (
        "klantgesprek",
        "Klantgesprek",
        "This is a client meeting. Capture the client's needs, questions, commitments made to them and next steps.",
    ),
    ("standup", "Standup", "This is a standup. Per person: done, next, blockers."),
];

#[uniffi::export]
pub fn meeting_templates() -> Vec<MeetingTemplate> {
    TEMPLATES.iter().map(|(id, name, _)| MeetingTemplate { id: (*id).into(), name: (*name).into() }).collect()
}

fn template_instruction(id: &str) -> &'static str {
    TEMPLATES.iter().find(|(t, _, _)| *t == id).map_or(TEMPLATES[0].2, |(_, _, i)| i)
}

pub fn summary_messages(template_id: &str, notes: &str, transcript: &str) -> Vec<ChatMessage> {
    let notes = notes.trim();
    let mode = if notes.is_empty() { MODE_SUMMARY } else { MODE_NOTES };
    let system = format!("{SYSTEM_PROMPT}\n{}\n{mode}", template_instruction(template_id));
    let mut user = String::new();
    if !notes.is_empty() {
        user.push_str(&format!("<notes>\n{notes}\n</notes>\n"));
    }
    user.push_str(&format!("<transcript>\n{transcript}\n</transcript>"));
    vec![ChatMessage { role: "system", content: system }, ChatMessage { role: "user", content: user }]
}

pub fn chunk_messages(part: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage { role: "system", content: MAP_PROMPT.into() },
        ChatMessage { role: "user", content: format!("<transcript>\n{part}\n</transcript>") },
    ]
}

pub fn transcript_text(segments: &[Segment]) -> String {
    segments
        .iter()
        .filter(|s| !s.failed && !s.text.is_empty())
        .map(|s| {
            let who = if s.speaker == Speaker::Me { "Ik" } else { "Anderen" };
            format!("[{}] {who}: {}", timestamp(s.start_ms), s.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits on line boundaries into blocks of at most `max` characters.
pub fn blocks(text: &str, max: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in text.lines() {
        let current = out.last_mut().expect("non-empty");
        if !current.is_empty() && current.len() + line.len() + 1 > max {
            out.push(String::new());
        }
        let current = out.last_mut().expect("non-empty");
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{ApiKeys, MusliEngine};
    use crate::meetings::{Segment, Speaker};
    use std::collections::HashMap;

    fn seg(start_ms: u64, speaker: Speaker, text: &str, failed: bool) -> Segment {
        Segment { start_ms, end_ms: start_ms, speaker, text: text.into(), failed }
    }

    #[test]
    fn templates_are_builtin() {
        let ids: Vec<String> = meeting_templates().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["algemeen", "een-op-een", "klantgesprek", "standup"]);
    }

    #[test]
    fn prompt_without_notes_summarizes() {
        let messages = summary_messages("algemeen", "", "[00:00] Ik: hallo");
        assert!(messages[0].content.contains("Summarize the meeting in short bullet points"));
        assert!(messages[0].content.contains("## Actiepunten"));
        assert!(!messages[1].content.contains("<notes>"));
        assert!(messages[1].content.contains("<transcript>\n[00:00] Ik: hallo\n</transcript>"));
    }

    #[test]
    fn prompt_with_notes_keeps_lines_verbatim() {
        let messages = summary_messages("algemeen", "- export naar Excel", "[00:00] Ik: hallo");
        assert!(messages[0].content.contains("keep every line of the user's notes verbatim"));
        assert!(messages[1].content.starts_with("<notes>\n- export naar Excel\n</notes>"));
    }

    #[test]
    fn template_instructions_are_included() {
        let messages = summary_messages("klantgesprek", "", "x");
        assert!(messages[0].content.contains("This is a client meeting"));
        let unknown = summary_messages("bestaat-niet", "", "x");
        assert!(unknown[0].content.contains("Focus on what was discussed and agreed."));
    }

    #[test]
    fn transcript_text_labels_speakers_and_skips_failed() {
        let text = transcript_text(&[
            seg(0, Speaker::Me, "Hallo", false),
            seg(5_000, Speaker::Others, "", true),
            seg(65_000, Speaker::Others, "Dag", false),
        ]);
        assert_eq!(text, "[00:00] Ik: Hallo\n[01:05] Anderen: Dag");
    }

    fn engine(dir: &tempfile::TempDir, server: &mockito::Server) -> std::sync::Arc<MusliEngine> {
        let engine = MusliEngine::new(dir.path().join("data").to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.providers[0].base_url = server.url();
        settings.meetings_dir = dir.path().join("meetings").to_string_lossy().into_owned();
        engine.update_settings(settings).unwrap();
        engine
    }

    fn keys() -> ApiKeys {
        ApiKeys { by_provider: HashMap::from([("groq".to_owned(), "k".to_owned())]) }
    }

    fn llm(server: &mut mockito::Server, text: &str) -> mockito::Mock {
        server
            .mock("POST", "/chat/completions")
            .with_body(serde_json::json!({ "choices": [{ "message": { "content": text } }] }).to_string())
            .create()
    }

    #[test]
    fn summarize_writes_meeting_md() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let store = engine.meeting_store();
        let id = store.create("Klant", chrono::Local::now()).unwrap();
        store.write_transcript(&id, &[seg(0, Speaker::Others, "We willen een Excel-export", false)]).unwrap();
        llm(&mut server, "## Samenvatting\n- Klant wil Excel-export\n## Beslissingen\n- Geen\n## Actiepunten\n- Geen");
        let body = engine.summarize_meeting(id.clone(), "algemeen".into(), keys()).unwrap();
        assert!(body.contains("Klant wil Excel-export"));
        let md = std::fs::read_to_string(store.dir_of(&id).join("meeting.md")).unwrap();
        assert!(md.contains("template: algemeen"), "{md}");
        assert!(engine.meeting_info(id).unwrap().has_summary);
    }

    #[test]
    fn long_transcript_uses_map_reduce() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let store = engine.meeting_store();
        let id = store.create("Lang", chrono::Local::now()).unwrap();
        let line = "x".repeat(90);
        let segments: Vec<Segment> = (0..450).map(|i| seg(i * 1000, Speaker::Me, &line, false)).collect();
        store.write_transcript(&id, &segments).unwrap();
        let mock = llm(&mut server, "## Samenvatting\n- ok").expect(3);
        engine.summarize_meeting(id, "algemeen".into(), keys()).unwrap();
        mock.assert();
    }

    #[test]
    fn empty_meeting_is_no_speech() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let id = engine.meeting_store().create("Leeg", chrono::Local::now()).unwrap();
        assert_eq!(engine.summarize_meeting(id, "algemeen".into(), keys()).unwrap_err(), crate::error::MusliError::NoSpeech);
    }
}
