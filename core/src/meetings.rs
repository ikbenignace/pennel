use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};

use crate::error::PennelError;

const TITLE_MAX: usize = 60;
const FAILED_MARK: &str = "_(transcriptie mislukt)_";

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Speaker {
    Me,
    Others,
}

impl Speaker {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Speaker::Me => "Ik",
            Speaker::Others => "Anderen",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Speaker,
    pub text: String,
    pub failed: bool,
    /// Which of the other participants spoke, after diarization (1-based); `None` for "Ik" or undiarized.
    pub voice: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SpeakerName {
    pub voice: u32,
    pub name: String,
}

const SPEAKER_NAME_MAX: usize = 40;
/// Sortformer tells at most four voices apart.
pub const VOICES: std::ops::RangeInclusive<u32> = 1..=4;

/// "Ik", a participant's name, "Anderen 2" or "Anderen".
pub fn speaker_label(segment: &Segment, names: &[SpeakerName]) -> String {
    match (segment.speaker, segment.voice) {
        (Speaker::Me, _) => "Ik".to_owned(),
        (Speaker::Others, Some(voice)) => names
            .iter()
            .find(|n| n.voice == voice)
            .map_or_else(|| format!("Anderen {voice}"), |n| n.name.clone()),
        (Speaker::Others, None) => "Anderen".to_owned(),
    }
}

fn sanitize_speaker(name: &str) -> String {
    let clean: String = name.chars().filter(|c| !matches!(c, ';' | '=' | ':' | '\n' | '\r' | '[' | ']' | '*')).collect();
    clean.trim().chars().take(SPEAKER_NAME_MAX).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MeetingInfo {
    pub id: String,
    pub title: String,
    pub start_iso: String,
    pub duration_min: u32,
    pub has_summary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NoteLine {
    pub text: String,
    pub mine: bool,
}

struct Frontmatter {
    title: String,
    start_iso: String,
    duration_min: u32,
    template: String,
    /// Lines the user added (tags, attendees, …), written back untouched.
    extra: Vec<String>,
}

pub struct MeetingStore {
    dir: PathBuf,
}

pub(crate) fn timestamp(ms: u64) -> String {
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

pub(crate) fn parse_timestamp(text: &str) -> Option<u64> {
    let parts: Vec<u64> = text.split(':').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let seconds = match parts.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(seconds * 1000)
}

fn sanitize(title: &str) -> String {
    let clean: String = title.chars().filter(|c| !matches!(c, '/' | ':' | '\\')).collect();
    let clean = clean.trim();
    let clean = if clean.is_empty() { "Meeting" } else { clean };
    clean.chars().take(TITLE_MAX).collect()
}

fn normalize_note(line: &str) -> String {
    let mut text = line.trim();
    for prefix in ["- [ ] ", "- [x] ", "- ", "* ", "[ ] "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
            break;
        }
    }
    text.trim().to_lowercase()
}

#[uniffi::export]
pub fn mark_lines(enhanced: &str, notes: &str) -> Vec<NoteLine> {
    let own: Vec<String> = notes.lines().map(normalize_note).filter(|l| !l.is_empty()).collect();
    enhanced
        .lines()
        .map(|line| {
            let key = normalize_note(line);
            NoteLine { text: line.to_owned(), mine: !key.is_empty() && own.contains(&key) }
        })
        .collect()
}

impl MeetingStore {
    pub fn new(dir: &Path) -> Self {
        Self { dir: dir.to_owned() }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.clone()
    }

    pub fn dir_of(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    pub fn create(&self, title: &str, start: DateTime<Local>) -> Result<String, PennelError> {
        let base = format!("{} {}", start.format("%Y-%m-%d %H%M"), sanitize(title));
        let id = std::iter::once(base.clone())
            .chain((2..).map(|n| format!("{base} ({n})")))
            .find(|candidate| !self.dir.join(candidate).exists())
            .expect("unbounded iterator");
        std::fs::create_dir_all(self.dir_of(&id)).map_err(PennelError::storage)?;
        let front = Frontmatter {
            title: sanitize(title),
            start_iso: start.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            duration_min: 0,
            template: String::new(),
            extra: Vec::new(),
        };
        self.write_file(&id, "meeting.md", &render_meeting(&front, ""))?;
        Ok(id)
    }

    fn write_file(&self, id: &str, name: &str, content: &str) -> Result<(), PennelError> {
        let path = self.dir_of(id).join(name);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, content).map_err(PennelError::storage)?;
        std::fs::rename(&tmp, &path).map_err(PennelError::storage)
    }

    /// Modification time (ns) and length, enough to notice any rewrite of a meeting file.
    pub fn file_stamp(&self, id: &str, name: &str) -> Option<(i64, u64)> {
        let meta = std::fs::metadata(self.dir_of(id).join(name)).ok()?;
        let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
        Some((modified.as_nanos() as i64, meta.len()))
    }

    fn read_file(&self, id: &str, name: &str) -> String {
        std::fs::read_to_string(self.dir_of(id).join(name)).unwrap_or_default()
    }

    pub fn write_notes(&self, id: &str, notes: &str) -> Result<(), PennelError> {
        self.write_file(id, "notes.md", notes)
    }

    pub fn read_notes(&self, id: &str) -> String {
        self.read_file(id, "notes.md")
    }

    pub fn write_transcript(&self, id: &str, segments: &[Segment]) -> Result<(), PennelError> {
        let mut sorted = segments.to_vec();
        sorted.sort_by_key(|s| s.start_ms);
        let body: String = sorted
            .iter()
            .map(|s| {
                let text = if s.failed { FAILED_MARK.to_owned() } else { s.text.split_whitespace().collect::<Vec<_>>().join(" ") };
                let label = match (s.speaker, s.voice) {
                    (Speaker::Others, Some(voice)) => format!("Anderen {voice}"),
                    _ => s.speaker.label().to_owned(),
                };
                format!("[{}] **{label}:** {text}\n", timestamp(s.start_ms))
            })
            .collect();
        self.write_file(id, "transcript.md", &body)
    }

    /// Number of non-empty transcript lines that did not parse; rewriting the transcript would drop them.
    pub fn unparsed_transcript_lines(&self, id: &str) -> usize {
        self.read_file(id, "transcript.md").lines().filter(|l| !l.trim().is_empty() && parse_segment(l).is_none()).count()
    }

    pub fn read_transcript(&self, id: &str) -> Vec<Segment> {
        self.read_file(id, "transcript.md").lines().filter_map(parse_segment).collect()
    }

    fn frontmatter(&self, id: &str) -> Option<(Frontmatter, String)> {
        parse_meeting(&self.read_file(id, "meeting.md"))
    }

    pub fn write_meeting(&self, id: &str, duration_min: u32, template: &str, body: &str) -> Result<(), PennelError> {
        let (mut front, _) = self.frontmatter(id).ok_or(PennelError::NotFound)?;
        front.duration_min = duration_min;
        front.template = template.to_owned();
        self.write_file(id, "meeting.md", &render_meeting(&front, body))
    }

    pub fn set_attendees(&self, id: &str, attendees: &[String]) -> Result<(), PennelError> {
        let (mut front, body) = self.frontmatter(id).ok_or(PennelError::NotFound)?;
        front.extra.retain(|line| !line.starts_with("attendees:"));
        if !attendees.is_empty() {
            let names: Vec<String> = attendees.iter().map(|a| a.replace([',', '[', ']'], " ").trim().to_owned()).collect();
            front.extra.push(format!("attendees: [{}]", names.join(", ")));
        }
        self.write_file(id, "meeting.md", &render_meeting(&front, &body))
    }

    pub fn attendees(&self, id: &str) -> Vec<String> {
        let Some((front, _)) = self.frontmatter(id) else { return Vec::new() };
        front
            .extra
            .iter()
            .find_map(|line| line.strip_prefix("attendees:"))
            .map(|list| {
                list.trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn speaker_names(&self, id: &str) -> Vec<SpeakerName> {
        let Some((front, _)) = self.frontmatter(id) else { return Vec::new() };
        let mut names: Vec<SpeakerName> = front
            .extra
            .iter()
            .find_map(|line| line.strip_prefix("speakers:"))
            .map(|list| {
                list.split(';')
                    .filter_map(|pair| {
                        let (voice, name) = pair.split_once('=')?;
                        let voice = voice.trim().parse().ok().filter(|v| VOICES.contains(v))?;
                        Some(SpeakerName { voice, name: name.trim().to_owned() })
                    })
                    .filter(|n| !n.name.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        names.sort_by_key(|n| n.voice);
        names
    }

    /// An empty name forgets the voice's name.
    pub fn rename_speaker(&self, id: &str, voice: u32, name: &str) -> Result<(), PennelError> {
        if !VOICES.contains(&voice) {
            return Err(PennelError::NotFound);
        }
        let mut names = self.speaker_names(id);
        names.retain(|n| n.voice != voice);
        let name = sanitize_speaker(name);
        if !name.is_empty() {
            names.push(SpeakerName { voice, name });
        }
        names.sort_by_key(|n| n.voice);
        let (mut front, body) = self.frontmatter(id).ok_or(PennelError::NotFound)?;
        front.extra.retain(|line| !line.starts_with("speakers:"));
        if !names.is_empty() {
            let list: Vec<String> = names.iter().map(|n| format!("{}={}", n.voice, n.name)).collect();
            front.extra.push(format!("speakers: {}", list.join("; ")));
        }
        self.write_file(id, "meeting.md", &render_meeting(&front, &body))
    }

    pub fn set_duration(&self, id: &str, duration_min: u32) -> Result<(), PennelError> {
        let (mut front, body) = self.frontmatter(id).ok_or(PennelError::NotFound)?;
        front.duration_min = duration_min;
        self.write_file(id, "meeting.md", &render_meeting(&front, &body))
    }

    pub fn read_meeting(&self, id: &str) -> Option<String> {
        self.frontmatter(id).map(|(_, body)| body).filter(|b| !b.trim().is_empty())
    }

    pub fn info(&self, id: &str) -> Option<MeetingInfo> {
        let (front, body) = self.frontmatter(id)?;
        Some(MeetingInfo {
            id: id.to_owned(),
            title: front.title,
            start_iso: front.start_iso,
            duration_min: front.duration_min,
            has_summary: !body.trim().is_empty(),
        })
    }

    pub fn list(&self) -> Vec<MeetingInfo> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut list: Vec<MeetingInfo> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| self.info(&e.file_name().to_string_lossy()))
            .collect();
        list.sort_by(|a, b| b.start_iso.cmp(&a.start_iso).then(b.id.cmp(&a.id)));
        list
    }

    pub fn search(&self, query: &str) -> Vec<MeetingInfo> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.list();
        }
        self.list()
            .into_iter()
            .filter(|m| {
                [m.title.clone(), self.read_notes(&m.id), self.read_file(&m.id, "transcript.md"), self.read_file(&m.id, "meeting.md")]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub fn rename(&self, id: &str, title: &str) -> Result<String, PennelError> {
        let (mut front, body) = self.frontmatter(id).ok_or(PennelError::NotFound)?;
        let prefix: String = id.chars().take("YYYY-MM-DD HHmm ".len()).collect();
        let base = format!("{prefix}{}", sanitize(title));
        let new_id = std::iter::once(base.clone())
            .chain((2..).map(|n| format!("{base} ({n})")))
            .find(|candidate| candidate == id || !self.dir.join(candidate).exists())
            .expect("unbounded iterator");
        if new_id != id {
            std::fs::rename(self.dir_of(id), self.dir_of(&new_id)).map_err(PennelError::storage)?;
        }
        front.title = sanitize(title);
        self.write_file(&new_id, "meeting.md", &render_meeting(&front, &body))?;
        Ok(new_id)
    }

    pub fn delete(&self, id: &str) -> Result<(), PennelError> {
        std::fs::remove_dir_all(self.dir_of(id)).map_err(PennelError::storage)
    }

    pub fn prune_audio(&self, retention_days: u32, now: SystemTime) -> u32 {
        if retention_days == 0 {
            return 0;
        }
        let max_age = std::time::Duration::from_secs(u64::from(retention_days) * 86_400);
        let mut removed = 0;
        for meeting in self.list() {
            let Ok(entries) = std::fs::read_dir(self.dir_of(&meeting.id)) else { continue };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let old = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|modified| now.duration_since(modified).ok())
                    .is_some_and(|age| age >= max_age);
                if path.extension().is_some_and(|e| e == "m4a") && old && std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }
}

fn render_meeting(front: &Frontmatter, body: &str) -> String {
    let extra: String = front.extra.iter().map(|line| format!("{line}\n")).collect();
    format!(
        "---\ntitle: {}\nstart: {}\nduration_min: {}\ntemplate: {}\n{extra}---\n\n{}\n",
        front.title,
        front.start_iso,
        front.duration_min,
        front.template,
        body.trim()
    )
}

const KNOWN_FIELDS: &[&str] = &["title", "start", "duration_min", "template"];

fn parse_meeting(text: &str) -> Option<(Frontmatter, String)> {
    let rest = text.strip_prefix("---\n")?;
    let (head, body) = rest.split_once("\n---\n")?;
    let field = |key: &str| {
        head.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix(": ").or(v.strip_prefix(":"))))
            .unwrap_or("")
            .trim()
            .to_owned()
    };
    let front = Frontmatter {
        title: field("title"),
        start_iso: field("start"),
        duration_min: field("duration_min").parse().unwrap_or(0),
        template: field("template"),
        extra: head
            .lines()
            .filter(|l| !KNOWN_FIELDS.iter().any(|k| l.split(':').next().is_some_and(|key| key.trim() == *k)))
            .map(str::to_owned)
            .collect(),
    };
    Some((front, body.trim().to_owned()))
}

fn parse_segment(line: &str) -> Option<Segment> {
    let rest = line.strip_prefix('[')?;
    let (stamp, rest) = rest.split_once("] **")?;
    let (label, text) = rest.split_once(":** ")?;
    let (speaker, voice) = match label {
        "Ik" => (Speaker::Me, None),
        "Anderen" => (Speaker::Others, None),
        other => (Speaker::Others, Some(other.strip_prefix("Anderen ")?.parse().ok().filter(|v| VOICES.contains(v))?)),
    };
    let start_ms = parse_timestamp(stamp)?;
    let failed = text.trim() == FAILED_MARK;
    Some(Segment { start_ms, end_ms: start_ms, speaker, text: if failed { String::new() } else { text.to_owned() }, failed, voice })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn start() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 23, 14, 0, 0).unwrap()
    }

    fn seg(start_ms: u64, speaker: Speaker, text: &str) -> Segment {
        Segment { start_ms, end_ms: start_ms + 1000, speaker, text: text.into(), failed: false, voice: None }
    }

    #[test]
    fn transcript_voice_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("Sync", start()).unwrap();
        let mut two = seg(5_000, Speaker::Others, "Klopt");
        two.voice = Some(2);
        store.write_transcript(&id, &[seg(0, Speaker::Others, "Hallo"), two.clone(), seg(9_000, Speaker::Me, "Ok")]).unwrap();
        let read = store.read_transcript(&id);
        assert_eq!(read[0].voice, None);
        assert_eq!(read[1].voice, Some(2));
        assert_eq!(read[1].speaker, Speaker::Others);
        assert!(std::fs::read_to_string(store.dir_of(&id).join("transcript.md")).unwrap().contains("[00:05] **Anderen 2:** Klopt"));
        assert_eq!(parse_segment("[00:01] **Anderen 0:** x"), None);
        assert_eq!(parse_segment("[00:01] **Anderen 9:** x"), None);
        assert_eq!(store.rename_speaker(&id, 0, "x"), Err(PennelError::NotFound));
    }

    #[test]
    fn speaker_names_are_sanitized() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("Sync", start()).unwrap();
        store.rename_speaker(&id, 2, "Jan; De=Smet: CEO").unwrap();
        store.rename_speaker(&id, 1, "Els").unwrap();
        assert_eq!(
            store.speaker_names(&id),
            vec![SpeakerName { voice: 1, name: "Els".into() }, SpeakerName { voice: 2, name: "Jan DeSmet CEO".into() }]
        );
        store.rename_speaker(&id, 1, "  ").unwrap();
        assert_eq!(store.speaker_names(&id).len(), 1);
        assert_eq!(store.info(&id).unwrap().title, "Sync");
    }

    #[test]
    fn labels_use_names() {
        let names = [SpeakerName { voice: 2, name: "Jan".into() }];
        let mut two = seg(0, Speaker::Others, "x");
        two.voice = Some(2);
        let mut three = two.clone();
        three.voice = Some(3);
        assert_eq!(speaker_label(&two, &names), "Jan");
        assert_eq!(speaker_label(&three, &names), "Anderen 3");
        assert_eq!(speaker_label(&seg(0, Speaker::Others, "x"), &names), "Anderen");
        assert_eq!(speaker_label(&seg(0, Speaker::Me, "x"), &names), "Ik");
    }

    #[test]
    fn create_uses_dated_folder_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("Sprint review", start()).unwrap();
        assert_eq!(id, "2026-10-23 1400 Sprint review");
        assert!(dir.path().join(&id).join("meeting.md").exists());
    }

    #[test]
    fn create_sanitizes_and_dedupes_title() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        assert_eq!(store.create("a/b:c", start()).unwrap(), "2026-10-23 1400 abc");
        assert_eq!(store.create("a/b:c", start()).unwrap(), "2026-10-23 1400 abc (2)");
        let long = "x".repeat(100);
        assert_eq!(store.create(&long, start()).unwrap().len(), "2026-10-23 1400 ".len() + 60);
    }

    #[test]
    fn transcript_roundtrips_with_speakers_and_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        let mut failed = seg(65_000, Speaker::Others, "");
        failed.failed = true;
        store.write_transcript(&id, &[seg(5_000, Speaker::Me, "Hallo allemaal"), failed]).unwrap();
        let md = std::fs::read_to_string(dir.path().join(&id).join("transcript.md")).unwrap();
        assert!(md.contains("[00:05] **Ik:** Hallo allemaal"), "{md}");
        assert!(md.contains("[01:05] **Anderen:** _(transcriptie mislukt)_"), "{md}");
        let back = store.read_transcript(&id);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].speaker, Speaker::Me);
        assert_eq!(back[0].text, "Hallo allemaal");
        assert_eq!(back[0].start_ms, 5_000);
        assert!(back[1].failed);
    }

    #[test]
    fn transcript_keeps_multiline_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        store.write_transcript(&id, &[seg(0, Speaker::Me, "eerste zin\ntweede zin")]).unwrap();
        assert_eq!(store.read_transcript(&id)[0].text, "eerste zin tweede zin");
    }

    #[test]
    fn rewriting_keeps_frontmatter_added_by_user() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        let path = dir.path().join(&id).join("meeting.md");
        let edited = std::fs::read_to_string(&path).unwrap().replace("template: \n", "template: \ntags: [q8, klant]\n");
        std::fs::write(&path, edited).unwrap();
        store.write_meeting(&id, 5, "algemeen", "body").unwrap();
        let id = store.rename(&id, "Nieuw").unwrap();
        let md = std::fs::read_to_string(dir.path().join(&id).join("meeting.md")).unwrap();
        assert!(md.contains("tags: [q8, klant]"), "{md}");
        assert!(md.contains("title: Nieuw"), "{md}");
    }

    #[test]
    fn attendees_are_written_to_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        store.set_attendees(&id, &["Sarah Peeters".into(), "Tom".into()]).unwrap();
        store.write_meeting(&id, 3, "algemeen", "body").unwrap();
        let md = std::fs::read_to_string(dir.path().join(&id).join("meeting.md")).unwrap();
        assert_eq!(md.matches("attendees:").count(), 1, "{md}");
        assert!(md.contains("attendees: [Sarah Peeters, Tom]"), "{md}");
    }

    #[test]
    fn transcript_hours_format() {
        assert_eq!(timestamp(3_723_000), "1:02:03");
        assert_eq!(timestamp(59_000), "00:59");
        assert_eq!(parse_timestamp("1:02:03"), Some(3_723_000));
    }

    #[test]
    fn meeting_md_has_frontmatter_and_body() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("Klant X", start()).unwrap();
        store.write_meeting(&id, 42, "klantgesprek", "## Samenvatting\n- ok").unwrap();
        let md = std::fs::read_to_string(dir.path().join(&id).join("meeting.md")).unwrap();
        assert!(md.starts_with("---\ntitle: Klant X\nstart: 2026-10-23T14:00:00"), "{md}");
        assert!(md.contains("duration_min: 42\ntemplate: klantgesprek\n---\n"), "{md}");
        assert_eq!(store.read_meeting(&id).as_deref(), Some("## Samenvatting\n- ok"));
        let info = &store.list()[0];
        assert_eq!(info.title, "Klant X");
        assert_eq!(info.duration_min, 42);
        assert!(info.has_summary);
    }

    #[test]
    fn read_meeting_returns_external_edit() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        store.write_meeting(&id, 1, "algemeen", "origineel").unwrap();
        let path = dir.path().join(&id).join("meeting.md");
        let edited = std::fs::read_to_string(&path).unwrap().replace("origineel", "bewerkt in Obsidian");
        std::fs::write(&path, edited).unwrap();
        assert_eq!(store.read_meeting(&id).as_deref(), Some("bewerkt in Obsidian"));
    }

    #[test]
    fn list_newest_first_and_includes_in_progress() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        store.create("Oud", Local.with_ymd_and_hms(2026, 10, 20, 9, 0, 0).unwrap()).unwrap();
        store.create("Nieuw", start()).unwrap();
        let list = store.list();
        assert_eq!(list.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(), ["Nieuw", "Oud"]);
        assert!(!list[0].has_summary);
    }

    #[test]
    fn search_matches_transcript_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let a = store.create("A", start()).unwrap();
        store.create("B", start()).unwrap();
        store.write_transcript(&a, &[seg(0, Speaker::Others, "De Excel-export is traag")]).unwrap();
        let hits = store.search("excel");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, a);
        assert_eq!(store.search("").len(), 2);
    }

    #[test]
    fn notes_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        assert_eq!(store.read_notes(&id), "");
        store.write_notes(&id, "- export naar Excel").unwrap();
        assert_eq!(store.read_notes(&id), "- export naar Excel");
    }

    #[test]
    fn rename_moves_folder_and_updates_title() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        let new_id = store.rename(&id, "Kickoff Q8").unwrap();
        assert_eq!(new_id, "2026-10-23 1400 Kickoff Q8");
        assert_eq!(store.list()[0].title, "Kickoff Q8");
        assert!(!dir.path().join(&id).exists());
    }

    #[test]
    fn prune_audio_keeps_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path());
        let id = store.create("T", start()).unwrap();
        let audio = dir.path().join(&id).join("ik.m4a");
        std::fs::write(&audio, b"aac").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(40 * 86_400);
        assert_eq!(store.prune_audio(0, later), 0);
        assert_eq!(store.prune_audio(30, later), 1);
        assert!(!audio.exists());
        assert!(dir.path().join(&id).join("meeting.md").exists());
    }

    #[test]
    fn mark_lines_matches_own_notes() {
        let lines = mark_lines("- Export naar Excel\n- Tom vond de oorzaak\n", "- export naar Excel");
        assert_eq!(
            lines,
            vec![
                NoteLine { text: "- Export naar Excel".into(), mine: true },
                NoteLine { text: "- Tom vond de oorzaak".into(), mine: false },
            ]
        );
    }
}
