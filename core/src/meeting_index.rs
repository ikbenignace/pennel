use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, params};

use crate::error::MusliError;
use crate::meetings::{MeetingInfo, MeetingStore, Segment};

const PASSAGE_CHARS: usize = 600;
const FILES: [(&str, PassageKind); 3] =
    [("notes.md", PassageKind::Notes), ("transcript.md", PassageKind::Transcript), ("meeting.md", PassageKind::Summary)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PassageKind {
    Notes,
    Transcript,
    Summary,
}

impl PassageKind {
    fn from_file(name: &str) -> Option<Self> {
        FILES.iter().find(|(file, _)| *file == name).map(|(_, kind)| *kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Passage {
    pub meeting_id: String,
    pub title: String,
    pub start_iso: String,
    pub kind: PassageKind,
    pub start_ms: Option<u64>,
    pub text: String,
    pub snippet: String,
}

/// Full-text mirror of the Markdown meetings; the files stay the source of truth.
pub struct MeetingIndex {
    conn: Connection,
}

fn db(e: rusqlite::Error) -> MusliError {
    MusliError::Storage { message: e.to_string() }
}

/// Words only, each as a quoted prefix term, so user input can never be read as FTS syntax.
pub(crate) fn fts_terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.to_lowercase()))
        .collect()
}

fn pack(pieces: impl IntoIterator<Item = (Option<u64>, String)>) -> Vec<(Option<u64>, String)> {
    let mut passages: Vec<(Option<u64>, String)> = Vec::new();
    for (start, piece) in pieces {
        match passages.last_mut() {
            Some((_, text)) if text.len() + piece.len() < PASSAGE_CHARS => {
                text.push('\n');
                text.push_str(&piece);
            }
            _ => passages.push((start, piece.chars().take(PASSAGE_CHARS).collect())),
        }
    }
    passages
}

fn transcript_passages(segments: &[Segment]) -> Vec<(Option<u64>, String)> {
    pack(
        segments
            .iter()
            .filter(|s| !s.failed && !s.text.trim().is_empty())
            .map(|s| (Some(s.start_ms), format!("{}: {}", s.speaker.label(), s.text.trim()))),
    )
}

fn text_passages(text: &str) -> Vec<(Option<u64>, String)> {
    pack(text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()).map(|p| (None, p.to_owned())))
}

impl MeetingIndex {
    pub fn open(path: &Path) -> Result<Self, MusliError> {
        let conn = Connection::open(path).map_err(db)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                meeting_id TEXT NOT NULL, name TEXT NOT NULL, mtime INTEGER NOT NULL, len INTEGER NOT NULL,
                PRIMARY KEY (meeting_id, name));
             CREATE VIRTUAL TABLE IF NOT EXISTS passages USING fts5(
                meeting_id UNINDEXED, name UNINDEXED, start_ms UNINDEXED, text,
                tokenize = 'unicode61 remove_diacritics 2');",
        )
        .map_err(db)?;
        Ok(Self { conn })
    }

    pub fn sync(&mut self, store: &MeetingStore) -> Result<(), MusliError> {
        let meetings: Vec<String> = store.list().into_iter().map(|m| m.id).collect();
        let tx = self.conn.transaction().map_err(db)?;
        let known: Vec<(String, String, i64, i64)> = {
            let mut stmt = tx.prepare("SELECT meeting_id, name, mtime, len FROM files").map_err(db)?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map_err(db)?
                .collect::<Result<_, _>>()
                .map_err(db)?
        };
        let known: HashMap<(String, String), (i64, i64)> =
            known.into_iter().map(|(id, name, mtime, len)| ((id, name), (mtime, len))).collect();

        for (id, name) in known.keys() {
            if !meetings.contains(id) || store.file_stamp(id, name).is_none() {
                tx.execute("DELETE FROM passages WHERE meeting_id = ?1 AND name = ?2", params![id, name]).map_err(db)?;
                tx.execute("DELETE FROM files WHERE meeting_id = ?1 AND name = ?2", params![id, name]).map_err(db)?;
            }
        }
        for id in &meetings {
            for (name, kind) in FILES {
                let Some((mtime, len)) = store.file_stamp(id, name) else { continue };
                if known.get(&(id.clone(), name.to_owned())) == Some(&(mtime, len as i64)) {
                    continue;
                }
                let passages = match kind {
                    PassageKind::Transcript => transcript_passages(&store.read_transcript(id)),
                    PassageKind::Notes => text_passages(&store.read_notes(id)),
                    PassageKind::Summary => text_passages(&store.read_meeting(id).unwrap_or_default()),
                };
                tx.execute("DELETE FROM passages WHERE meeting_id = ?1 AND name = ?2", params![id, name]).map_err(db)?;
                for (start, text) in passages {
                    tx.execute(
                        "INSERT INTO passages (meeting_id, name, start_ms, text) VALUES (?1, ?2, ?3, ?4)",
                        params![id, name, start.map(|ms| ms as i64), text],
                    )
                    .map_err(db)?;
                }
                tx.execute(
                    "INSERT OR REPLACE INTO files (meeting_id, name, mtime, len) VALUES (?1, ?2, ?3, ?4)",
                    params![id, name, mtime, len as i64],
                )
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)
    }

    /// Passages containing every word of `query` (prefix match), best first.
    pub fn search(
        &self,
        store: &MeetingStore,
        query: &str,
        meeting_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Passage>, MusliError> {
        self.matching(store, &fts_terms(query).join(" "), meeting_id, limit)
    }

    /// Passages containing any of `words`, best first.
    pub fn search_any(
        &self,
        store: &MeetingStore,
        words: &[String],
        meeting_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Passage>, MusliError> {
        let terms: Vec<String> = words.iter().flat_map(|w| fts_terms(w)).collect();
        self.matching(store, &terms.join(" OR "), meeting_id, limit)
    }

    fn matching(
        &self,
        store: &MeetingStore,
        expression: &str,
        meeting_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Passage>, MusliError> {
        if expression.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare(
                "SELECT meeting_id, name, start_ms, text, snippet(passages, 3, '«', '»', '…', 12)
                 FROM passages WHERE passages MATCH ?1 AND (?2 IS NULL OR meeting_id = ?2)
                 ORDER BY bm25(passages) LIMIT ?3",
            )
            .map_err(db)?;
        let rows = stmt.query_map(params![expression, meeting_id, limit], row).map_err(db)?;
        resolve(store, rows.collect::<Result<_, _>>().map_err(db)?)
    }

    /// Passages from the most recent meetings, newest first, for questions without searchable words.
    pub fn recent(&self, store: &MeetingStore, meeting_id: Option<&str>, limit: u32) -> Result<Vec<Passage>, MusliError> {
        let order: Vec<String> = match meeting_id {
            Some(id) => vec![id.to_owned()],
            None => store.list().into_iter().map(|m| m.id).collect(),
        };
        let mut out = Vec::new();
        let mut stmt = self
            .conn
            .prepare("SELECT meeting_id, name, start_ms, text, '' FROM passages WHERE meeting_id = ?1 ORDER BY rowid")
            .map_err(db)?;
        for id in order {
            let rows: Vec<RawRow> = stmt.query_map(params![id], row).map_err(db)?.collect::<Result<_, _>>().map_err(db)?;
            out.extend(resolve(store, rows)?);
            if out.len() >= limit as usize {
                break;
            }
        }
        out.truncate(limit as usize);
        Ok(out)
    }
}

type RawRow = (String, String, Option<i64>, String, String);

fn row(r: &rusqlite::Row) -> rusqlite::Result<RawRow> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

fn resolve(store: &MeetingStore, rows: Vec<RawRow>) -> Result<Vec<Passage>, MusliError> {
    let mut infos: HashMap<String, Option<MeetingInfo>> = HashMap::new();
    Ok(rows
        .into_iter()
        .filter_map(|(id, name, start, text, snippet)| {
            let info = infos.entry(id.clone()).or_insert_with(|| store.info(&id)).clone()?;
            Some(Passage {
                meeting_id: id,
                title: info.title,
                start_iso: info.start_iso,
                kind: PassageKind::from_file(&name)?,
                start_ms: start.map(|ms| ms as u64),
                snippet: if snippet.is_empty() { text.chars().take(160).collect() } else { snippet },
                text,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::Speaker;
    use chrono::TimeZone;

    struct Fixture {
        _dir: tempfile::TempDir,
        store: MeetingStore,
        index: MeetingIndex,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(&dir.path().join("meetings"));
        std::fs::create_dir_all(dir.path().join("meetings")).unwrap();
        let index = MeetingIndex::open(&dir.path().join("index.sqlite")).unwrap();
        Fixture { _dir: dir, store, index }
    }

    fn segment(start_ms: u64, text: &str) -> Segment {
        Segment { start_ms, end_ms: start_ms + 1000, speaker: Speaker::Others, text: text.into(), failed: false }
    }

    fn meeting(store: &MeetingStore, title: &str, hour: u32) -> String {
        store.create(title, chrono::Local.with_ymd_and_hms(2026, 10, 20, hour, 0, 0).unwrap()).unwrap()
    }

    #[test]
    fn finds_word_in_transcript_with_snippet() {
        let mut f = fixture();
        let id = meeting(&f.store, "Sync", 9);
        f.store.write_transcript(&id, &[segment(65_000, "We kiezen voor Parakeet als standaard")]).unwrap();
        f.index.sync(&f.store).unwrap();
        let hits = f.index.search(&f.store, "parakeet", None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, PassageKind::Transcript);
        assert_eq!(hits[0].start_ms, Some(65_000));
        assert_eq!(hits[0].title, "Sync");
        assert!(hits[0].snippet.contains("«Parakeet»"), "{}", hits[0].snippet);
    }

    #[test]
    fn diacritics_insensitive_and_prefix() {
        let mut f = fixture();
        let id = meeting(&f.store, "Plan", 9);
        f.store.write_notes(&id, "één actiepunt\n\nbudgetten herzien").unwrap();
        f.index.sync(&f.store).unwrap();
        assert_eq!(f.index.search(&f.store, "een", None, 10).unwrap().len(), 1);
        assert_eq!(f.index.search(&f.store, "budget", None, 10).unwrap().len(), 1);
    }

    #[test]
    fn query_with_fts_syntax_is_literal() {
        let mut f = fixture();
        meeting(&f.store, "Plan", 9);
        f.index.sync(&f.store).unwrap();
        assert!(f.index.search(&f.store, "\"budget\" -x (a:b* NEAR", None, 10).is_ok());
        assert!(f.index.search(&f.store, "  ?! ", None, 10).unwrap().is_empty());
        assert!(f.index.search_any(&f.store, &["or".into(), "and\"".into()], None, 10).is_ok());
    }

    #[test]
    fn sync_drops_removed_meetings() {
        let mut f = fixture();
        let a = meeting(&f.store, "A", 9);
        let b = meeting(&f.store, "B", 10);
        f.store.write_notes(&a, "kiwi").unwrap();
        f.store.write_notes(&b, "kiwi").unwrap();
        f.index.sync(&f.store).unwrap();
        assert_eq!(f.index.search(&f.store, "kiwi", None, 10).unwrap().len(), 2);
        f.store.delete(&b).unwrap();
        f.index.sync(&f.store).unwrap();
        let hits = f.index.search(&f.store, "kiwi", None, 10).unwrap();
        assert_eq!(hits.iter().map(|h| h.meeting_id.as_str()).collect::<Vec<_>>(), [a.as_str()]);
    }

    #[test]
    fn sync_reindexes_changed_file_once() {
        let mut f = fixture();
        let id = meeting(&f.store, "A", 9);
        f.store.write_transcript(&id, &[segment(0, "eerste versie")]).unwrap();
        f.index.sync(&f.store).unwrap();
        f.store.write_transcript(&id, &[segment(0, "tweede versie met meer tekst")]).unwrap();
        f.index.sync(&f.store).unwrap();
        f.index.sync(&f.store).unwrap();
        assert!(f.index.search(&f.store, "eerste", None, 10).unwrap().is_empty());
        assert_eq!(f.index.search(&f.store, "tweede", None, 10).unwrap().len(), 1);
    }

    #[test]
    fn scoped_search() {
        let mut f = fixture();
        let a = meeting(&f.store, "A", 9);
        let b = meeting(&f.store, "B", 10);
        f.store.write_notes(&a, "mango").unwrap();
        f.store.write_notes(&b, "mango").unwrap();
        f.index.sync(&f.store).unwrap();
        let hits = f.index.search(&f.store, "mango", Some(&a), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].meeting_id, a);
    }

    #[test]
    fn transcript_passages_capped() {
        let segments: Vec<Segment> = (0..50).map(|i| segment(i * 1000, &format!("zin nummer {i:02} {}", "x".repeat(20)))).collect();
        let passages = transcript_passages(&segments);
        assert!(passages.len() > 1);
        assert!(passages.iter().all(|(_, text)| text.len() <= PASSAGE_CHARS));
        let joined: String = passages.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n");
        assert!(joined.find("nummer 00").unwrap() < joined.find("nummer 49").unwrap());
        assert_eq!(passages[0].0, Some(0));
    }

    #[test]
    fn recent_returns_newest_meeting_first() {
        let mut f = fixture();
        let old = meeting(&f.store, "Oud", 8);
        let new = meeting(&f.store, "Nieuw", 11);
        f.store.write_notes(&old, "oude notitie").unwrap();
        f.store.write_notes(&new, "nieuwe notitie").unwrap();
        f.index.sync(&f.store).unwrap();
        let recent = f.index.recent(&f.store, None, 1).unwrap();
        assert_eq!(recent[0].meeting_id, new);
    }
}
