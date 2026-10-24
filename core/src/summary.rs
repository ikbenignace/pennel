use crate::llm::ChatMessage;
use crate::meetings::{Segment, SpeakerName, speaker_label, timestamp};
use crate::templates::TemplateSpec;

pub(crate) const MAP_REDUCE_ABOVE: usize = 40_000;
pub(crate) const BLOCK_CHARS: usize = 30_000;

const SYSTEM_PROMPT: &str = "You write meeting notes from a transcript. \"Ik\" is the user; every other label (\"Anderen\", \"Anderen 2\" or a name) is another participant.
Write in the language the meeting was held in (Dutch or English). Never invent facts that are not in the transcript or notes.";

const ACTION_RULE: &str =
    "Under Actiepunten use \"- [ ] Person — action\" lines; write \"- Geen\" when there are none.";
const MAP_PROMPT: &str = "Summarize this part of a meeting transcript as concise bullet points: facts, decisions and action items with who is responsible. Keep the language of the meeting. \"Ik\" is the user.";

fn notes_section(spec: &TemplateSpec) -> &str {
    spec.sections
        .iter()
        .find(|s| s.as_str() == "Samenvatting")
        .unwrap_or(&spec.sections[0])
}

pub fn summary_messages(spec: &TemplateSpec, notes: &str, transcript: &str) -> Vec<ChatMessage> {
    let notes = notes.trim();
    let sections: String = spec.sections.iter().map(|s| format!("## {s}\n")).collect();
    let mut system =
        format!("{SYSTEM_PROMPT}\nUse Markdown with exactly these sections:\n{sections}");
    if spec.sections.iter().any(|s| s == "Actiepunten") {
        system.push_str(ACTION_RULE);
        system.push('\n');
    }
    if !spec.instruction.is_empty() {
        system.push_str(&format!(
            "Template \"{}\": {}\n",
            spec.name, spec.instruction
        ));
    }
    let target = notes_section(spec);
    if notes.is_empty() {
        system.push_str(&format!(
            "Summarize the meeting in short bullet points under {target}."
        ));
    } else {
        system.push_str(&format!(
            "The user took the notes below during the meeting. Under {target}, keep every line of the user's notes verbatim (you may reorder them) and add missing important details from the transcript as separate bullet lines."
        ));
    }
    let mut user = String::new();
    if !notes.is_empty() {
        user.push_str(&format!("<notes>\n{notes}\n</notes>\n"));
    }
    user.push_str(&format!("<transcript>\n{transcript}\n</transcript>"));
    vec![
        ChatMessage {
            role: "system",
            content: system,
        },
        ChatMessage {
            role: "user",
            content: user,
        },
    ]
}

pub fn chunk_messages(part: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "system",
            content: MAP_PROMPT.into(),
        },
        ChatMessage {
            role: "user",
            content: format!("<transcript>\n{part}\n</transcript>"),
        },
    ]
}

pub fn transcript_text(segments: &[Segment], names: &[SpeakerName]) -> String {
    segments
        .iter()
        .filter(|s| !s.failed && !s.text.is_empty())
        .map(|s| {
            format!(
                "[{}] {}: {}",
                timestamp(s.start_ms),
                speaker_label(s, names),
                s.text
            )
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
    use crate::engine::{ApiKeys, PennelEngine};
    use crate::meetings::{Segment, Speaker};
    use std::collections::HashMap;

    fn seg(start_ms: u64, speaker: Speaker, text: &str, failed: bool) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms,
            speaker,
            text: text.into(),
            failed,
            voice: None,
        }
    }

    fn spec(id: &str) -> TemplateSpec {
        crate::templates::TemplateStore::new(std::path::Path::new("/nonexistent")).spec(id)
    }

    #[test]
    fn prompt_without_notes_summarizes() {
        let messages = summary_messages(&spec("algemeen"), "", "[00:00] Ik: hallo");
        assert!(
            messages[0]
                .content
                .contains("Summarize the meeting in short bullet points")
        );
        assert!(messages[0].content.contains("## Actiepunten"));
        assert!(!messages[1].content.contains("<notes>"));
        assert!(
            messages[1]
                .content
                .contains("<transcript>\n[00:00] Ik: hallo\n</transcript>")
        );
    }

    #[test]
    fn prompt_with_notes_keeps_lines_verbatim() {
        let messages = summary_messages(
            &spec("algemeen"),
            "- export naar Excel",
            "[00:00] Ik: hallo",
        );
        assert!(
            messages[0]
                .content
                .contains("keep every line of the user's notes verbatim")
        );
        assert!(
            messages[1]
                .content
                .starts_with("<notes>\n- export naar Excel\n</notes>")
        );
    }

    #[test]
    fn template_instructions_are_included() {
        let messages = summary_messages(&spec("klantgesprek"), "", "x");
        assert!(messages[0].content.contains("This is a client meeting"));
        let unknown = summary_messages(&spec("bestaat-niet"), "", "x");
        assert!(
            unknown[0]
                .content
                .contains("Focus on what was discussed and agreed.")
        );
    }

    #[test]
    fn custom_sections_drive_prompt() {
        let retro = TemplateSpec::parse(
            "Retro",
            "Wat ging goed en wat beter kan.\n## Goed\n## Beter\n## Actiepunten",
        );
        let messages = summary_messages(&retro, "", "x");
        assert!(
            messages[0]
                .content
                .contains("## Goed\n## Beter\n## Actiepunten\n")
        );
        assert!(
            messages[0]
                .content
                .contains("Wat ging goed en wat beter kan.")
        );
        assert!(messages[0].content.contains("- [ ] Person"));
        assert!(!messages[0].content.contains("## Beslissingen"));
    }

    #[test]
    fn notes_rule_targets_first_section_without_samenvatting() {
        let retro = TemplateSpec::parse("Retro", "## Goed\n## Beter");
        let messages = summary_messages(&retro, "- koffie", "x");
        assert!(
            messages[0]
                .content
                .contains("Under Goed, keep every line of the user's notes verbatim")
        );
        assert!(!messages[0].content.contains("- [ ] Person"));
    }

    #[test]
    fn transcript_text_labels_speakers_and_skips_failed() {
        let text = transcript_text(
            &[
                seg(0, Speaker::Me, "Hallo", false),
                seg(5_000, Speaker::Others, "", true),
                seg(65_000, Speaker::Others, "Dag", false),
            ],
            &[],
        );
        assert_eq!(text, "[00:00] Ik: Hallo\n[01:05] Anderen: Dag");
    }

    fn engine(dir: &tempfile::TempDir, server: &mockito::Server) -> std::sync::Arc<PennelEngine> {
        let engine =
            PennelEngine::new(dir.path().join("data").to_string_lossy().into_owned()).unwrap();
        let mut settings = engine.settings();
        settings.providers[0].base_url = server.url();
        settings.meetings_dir = dir.path().join("meetings").to_string_lossy().into_owned();
        engine.update_settings(settings).unwrap();
        engine
    }

    fn keys() -> ApiKeys {
        ApiKeys {
            by_provider: HashMap::from([("groq".to_owned(), "k".to_owned())]),
        }
    }

    fn llm(server: &mut mockito::Server, text: &str) -> mockito::Mock {
        server
            .mock("POST", "/chat/completions")
            .with_body(
                serde_json::json!({ "choices": [{ "message": { "content": text } }] }).to_string(),
            )
            .create()
    }

    #[test]
    fn summarize_writes_meeting_md() {
        let (dir, mut server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let store = engine.meeting_store();
        let id = store.create("Klant", chrono::Local::now()).unwrap();
        store
            .write_transcript(
                &id,
                &[seg(0, Speaker::Others, "We willen een Excel-export", false)],
            )
            .unwrap();
        llm(
            &mut server,
            "## Samenvatting\n- Klant wil Excel-export\n## Beslissingen\n- Geen\n## Actiepunten\n- Geen",
        );
        let body = engine
            .summarize_meeting(id.clone(), "algemeen".into(), keys())
            .unwrap();
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
        let segments: Vec<Segment> = (0..450)
            .map(|i| seg(i * 1000, Speaker::Me, &line, false))
            .collect();
        store.write_transcript(&id, &segments).unwrap();
        let mock = llm(&mut server, "## Samenvatting\n- ok").expect(3);
        engine
            .summarize_meeting(id, "algemeen".into(), keys())
            .unwrap();
        mock.assert();
    }

    #[test]
    fn empty_meeting_is_no_speech() {
        let (dir, server) = (tempfile::tempdir().unwrap(), mockito::Server::new());
        let engine = engine(&dir, &server);
        let id = engine
            .meeting_store()
            .create("Leeg", chrono::Local::now())
            .unwrap();
        assert_eq!(
            engine
                .summarize_meeting(id, "algemeen".into(), keys())
                .unwrap_err(),
            crate::error::PennelError::NoSpeech
        );
    }
}
