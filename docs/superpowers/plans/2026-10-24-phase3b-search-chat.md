# Fase 3b — zoeken en chatten met meetings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Full-text search over all meetings with snippets, and question-answering over one meeting or all meetings with clickable source citations.

**Architecture:** A SQLite FTS5 index (`index.sqlite` in the app data dir) mirrors the Markdown files in `~/Musli` and is synced by file mtime before every search/ask; the Markdown stays the source of truth. Chat retrieves passages from the index (or the whole meeting when it fits), sends numbered sources to the configured LLM, and returns the answer plus the sources it cited.

**Tech Stack:** Rust (rusqlite bundled, FTS5), UniFFI, SwiftUI.

**Spec:** `docs/superpowers/specs/2026-10-24-phase3-design.md` (row 3b)

## Global Constraints

- Markdown files remain the source of truth; deleting `index.sqlite` must only cost a rebuild.
- FTS5 tokenizer: `unicode61 remove_diacritics 2` (so "één" matches "een").
- Transcript passages: consecutive segments joined as `Speaker: text` lines, max 600 chars per passage, `start_ms` = first segment's start.
- Notes and summary passages: split on blank lines, max 600 chars.
- Single-meeting chat sends the full meeting (summary + notes + transcript) when ≤ 60 000 chars, else summary + top 25 passages.
- All-meetings chat sends the top 12 passages.
- LLM timeout 60 s. Answers in the language of the question; only from sources; cite as `[n]`; say plainly when the sources don't contain the answer.
- No network except the configured LLM provider. UI copy Dutch.

## Review Focus

1. Search query with FTS syntax characters (`"`, `*`, `-`, `:`, `(`) → no SQL/FTS error, treated as plain words. (Task 1 test `query_with_fts_syntax_is_literal`.)
2. Meeting renamed/deleted/trashed outside the app → stale rows disappear on next sync. (Task 1 test `sync_drops_removed_meetings`.)
3. Meeting still being recorded (transcript growing) → re-indexed when mtime changes, never double-indexed. (Task 1 test `sync_reindexes_changed_file_once`.)
4. Answer cites `[7]` when only 3 sources exist, or cites nothing → no crash; out-of-range ignored, no citations → empty source list. (Task 2 test `citations_out_of_range_ignored`.)
5. Question without any searchable word ("wat?") across all meetings → falls back to the 12 most recent passages instead of an empty context. (Task 2 test `no_terms_uses_recent_passages`.)

---

### Task 1: Meeting index with FTS5 search

**Files:**
- Create: `core/src/meeting_index.rs`
- Modify: `core/src/meetings.rs` (expose `pub fn ids(&self) -> Vec<String>` and `pub fn file_mtime(&self, id: &str, name: &str) -> Option<i64>`, reuse existing `read_notes`/`read_transcript`/`read_meeting`), `core/src/engine.rs` (field `index: Mutex<MeetingIndex>`, `search_meetings` via index, new `search_passages`), `core/src/lib.rs`

**Interfaces:**
- Produces: `MeetingIndex::open(path: &Path) -> Result<Self, MusliError>`, `sync(&mut self, store: &MeetingStore) -> Result<(), MusliError>`, `search(&self, query: &str, meeting_id: Option<&str>, limit: u32) -> Result<Vec<Passage>, MusliError>`, `recent(&self, limit: u32) -> Result<Vec<Passage>, MusliError>`.
- Produces (UniFFI record): `Passage { meeting_id: String, title: String, start_iso: String, kind: PassageKind, start_ms: Option<u64>, text: String, snippet: String }`, `enum PassageKind { Notes, Transcript, Summary }`. Snippet uses FTS `snippet()` with `«` `»` around matches, 12 tokens.
- Produces: `MusliEngine::search_passages(&self, query: String) -> Vec<Passage>` (limit 50); `search_meetings` keeps its signature, returns distinct meetings in rank order (empty query → `list()`).

- [ ] **Step 1: Failing tests** (tempdir store with two meetings written via `MeetingStore` API):
  - `finds_word_in_transcript_with_snippet`: transcript segment "We kiezen voor Parakeet als standaard" → `search("parakeet")` returns 1 passage, kind Transcript, `start_ms == Some(segment start)`, snippet contains `«Parakeet»`.
  - `diacritics_insensitive`: notes "één actiepunt" found by `search("een")`.
  - `prefix_match`: "budgetten" found by `search("budget")`.
  - `query_with_fts_syntax_is_literal`: `search("\"budget\" -x (a:b*")` → Ok.
  - `sync_drops_removed_meetings`: delete one meeting dir, sync → its passages gone.
  - `sync_reindexes_changed_file_once`: rewrite transcript with new text, bump mtime (`filetime` not available → sleep is flaky; instead store mtime as nanos via `metadata().modified()` and write twice with different content+length; compare `(mtime, len)` pair), sync twice → old text gone, new text present exactly once.
  - `scoped_search`: `meeting_id = Some(a)` excludes b's hits.
  - `transcript_passages_capped`: 50 segments of 40 chars → every Transcript passage `text.len() <= 600` and passages cover all segments in order.
- [ ] **Step 2:** `cargo test --lib meeting_index` fails.
- [ ] **Step 3:** Implement. Tables: `files(meeting_id, name, mtime, len, PRIMARY KEY(meeting_id, name))` and `CREATE VIRTUAL TABLE passages USING fts5(meeting_id UNINDEXED, name UNINDEXED, start_ms UNINDEXED, text, tokenize='unicode61 remove_diacritics 2')`. Title/start come from `store.info` at query time (cache per call). Query building: split on non-alphanumeric, drop empty, each term → `"term"*`, joined with spaces (implicit AND); no terms → empty result. Order by `bm25(passages)`.
- [ ] **Step 4:** Tests green; engine `search_meetings` tests still green.
- [ ] **Step 5:** Commit `feat(core): full-text meeting index`.

### Task 2: Ask meetings

**Files:**
- Create: `core/src/meeting_chat.rs`
- Modify: `core/src/engine.rs` (`ask_meetings`), `core/src/lib.rs`

**Interfaces:**
- Consumes: `MeetingIndex::{search, recent}`, `Passage`, `llm::chat_complete_within`.
- Produces (UniFFI): `ChatTurn { question: String, answer: String }`; `MeetingAnswer { text: String, sources: Vec<Passage> }`; `MusliEngine::ask_meetings(&self, question: String, history: Vec<ChatTurn>, meeting_id: Option<String>, keys: ApiKeys) -> Result<MeetingAnswer, MusliError>`.
- Produces (pure, tested): `fn search_terms(question: &str) -> String` (drops Dutch/English stop words and words < 3 chars, joins rest with ` OR `-free spaces for `search` — but use OR semantics: run `search` per term set via FTS `OR`), `fn chat_messages(question: &str, history: &[ChatTurn], sources: &[Passage], whole_meeting: Option<&str>) -> Vec<ChatMessage>`, `fn cited(answer: &str, sources: &[Passage]) -> Vec<Passage>`.

- [ ] **Step 1: Failing tests:**
  - `search_terms_drop_stop_words`: `"Wat hebben we beslist over het budget?"` → contains `budget`, `beslist`; not `wat`, `het`, `we`.
  - `chat_prompt_numbers_sources`: two passages → user content contains `[1]` and `[2]` with meeting titles and `mm:ss` time for transcript passages; system content contains "[n]".
  - `history_becomes_alternating_turns`: one `ChatTurn` → messages `system, user, assistant, user`.
  - `citations_out_of_range_ignored`: answer "Ja [2] en [7]" with 3 sources → `[sources[1]]`; "Geen idee" → empty.
  - `no_terms_uses_recent_passages` (engine-level with mockito LLM): question "wat?" with two indexed meetings → request body contains text from the latest meeting.
  - `single_meeting_sends_whole_meeting` (mockito): short meeting → body contains the full transcript text and the summary.
- [ ] **Step 2–4:** fail → implement → green. All-meetings search uses `MATCH 'a OR b OR c'` (add `search_any(&self, terms: &[String], meeting_id, limit)` to the index rather than overloading `search`).
- [ ] **Step 5:** Commit `feat(core): ask questions about meetings`.

### Task 3: Search and chat UI

**Files:**
- Create: `apps/macos/Sources/Musli/Meetings/MeetingChatView.swift`
- Modify: `apps/macos/Sources/Musli/Meetings/LibraryWindow.swift`, `apps/macos/Sources/Musli/Meetings/MeetingViews.swift`

**Interfaces:**
- Consumes: `searchPassages(query:)`, `askMeetings(question:history:meetingId:keys:)`, `state.keys()`.

- [ ] **Step 1:** Sidebar: with a non-empty query, show per meeting row up to two snippet lines (`«…»` rendered bold) from `searchPassages`; run the search off the main thread (`Task.detached`), debounce 250 ms.
- [ ] **Step 2:** Sidebar top: button "Vraag het je meetings" → detail area shows `MeetingChatView(meetingId: nil)`.
- [ ] **Step 3:** `MeetingDetailView` gets a third tab "Vragen" → `MeetingChatView(meetingId: id)`.
- [ ] **Step 4:** `MeetingChatView`: scrolling list of turns (question right-aligned muted bubble, answer left as Markdown text), input `TextField` + Enter, spinner while waiting, error line in `Theme.accent` on failure ("Geen taalmodel ingesteld" for MissingApiKey/NotFound). Sources under each answer as small chips "Titel · 12:34"; clicking posts `.musliSelectMeeting` with the meeting id (and for transcript passages, also `.musliShowTranscriptAt` with `start_ms` so the detail view switches to the Transcript tab and plays from there via the existing play action).
- [ ] **Step 5:** `make app`; commit `feat(macos): search snippets and chat with meetings`.

### Task 4: Verify

- [ ] `make test` green; build app; with a real meeting in `~/Musli` (if present) run one all-meetings question through the configured LLM via an ignored test `real_ask_meetings` printing the answer.
- [ ] Push branch `phase-3b-search-chat` (based on `phase-3a-models`).
