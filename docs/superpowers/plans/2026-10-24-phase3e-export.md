# Fase 3e — exporteren Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Share a meeting in one click: copy formatted (pastes as rich text in Notion, Slack, Mail, Docs), copy as Markdown, include the transcript optionally, open a prefilled mail, or use the macOS share sheet.

**Architecture:** The core renders an export Markdown document from the meeting files and converts that Markdown to minimal HTML. Swift writes both HTML and plain text to the pasteboard (apps pick the richest), and hands the text to `NSSharingService` for mail and the share picker.

**Tech Stack:** Rust, UniFFI, AppKit `NSPasteboard`/`NSSharingService`.

**Spec:** `docs/superpowers/specs/2026-10-24-phase3-design.md` (row 3e)

## Global Constraints

- Export document: `# Title`, a line `date · duration · attendees` (only known parts), the summary body (or the user's notes when there is no summary), and with transcript: `## Transcript` followed by `**[mm:ss] Ik:** text` lines.
- HTML: headings, paragraphs, `-`/`*` lists, `- [ ]`/`- [x]` as ☐/☑ list items, `**bold**`, `*italic*`, `` `code` ``, links `[t](u)`; everything else escaped. No external CSS.
- Plain-text copy is the Markdown itself.
- Mail subject = meeting title; body = Markdown (plain) because `NSSharingService` mail takes a string.

## Review Focus

1. Meeting without summary and without notes → export still has title + date and, with transcript, the transcript. (Test `export_without_summary_uses_notes_or_nothing`.)
2. HTML-special characters in titles/notes (`<script>`, `&`) → escaped. (Test `html_escapes`.)
3. Unclosed `**` or backticks → rendered literally, no panic. (Test `unbalanced_inline_marks_are_literal`.)
4. Frontmatter never leaks into the export. (Test `frontmatter_not_exported`.)
5. Failed transcript segments are skipped. (Test `failed_segments_skipped`.)

---

### Task 1: Core export
**Files:** create `core/src/export.rs`; modify `core/src/engine.rs` (`export_meeting(id: String, include_transcript: bool) -> Result<MeetingExport, MusliError>` returning `MeetingExport { title: String, markdown: String, html: String }`), `core/src/meetings.rs` (attendees reader if needed), `core/src/lib.rs`.
- [ ] Tests above + `export_has_title_meta_and_summary`, `markdown_to_html_lists_and_headings`. Implement; commit `feat(core): meeting export`.

### Task 2: Share menu
**Files:** create `apps/macos/Sources/Musli/Meetings/MeetingShare.swift`; modify `MeetingViews.swift` footer.
- [ ] Menu "Delen": "Kopieer opgemaakt", "Kopieer als Markdown", "Kopieer met transcript", Divider, "Mail…", "Deel…". Copy shows a 2 s inline confirmation ("Gekopieerd"). Build; commit `feat(macos): share meetings`.
