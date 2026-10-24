# Fase 3c — eigen samenvattingssjablonen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Users create, edit and delete their own summary templates (instruction + sections) next to the four built-ins.

**Architecture:** A template is a Markdown file `{meetings_dir}/.templates/{id}.md` with `name:` frontmatter; its body text is the instruction and its `## ` headings are the sections the summary must contain. Built-ins stay in code; a file with a built-in id overrides it. The core parses templates into a `TemplateSpec` that `summary_messages` uses.

**Tech Stack:** Rust, UniFFI, SwiftUI.

**Spec:** `docs/superpowers/specs/2026-10-24-phase3-design.md` (row 3c)

## Global Constraints

- Default sections when a template has no `## ` headings: `Samenvatting`, `Beslissingen`, `Actiepunten` (as today).
- `Actiepunten`, when present, keeps the `- [ ] Person — action` rule; "Samenvatting" keeps the notes-verbatim rule when the user took notes. When a custom template lacks "Samenvatting", the notes rule applies to its first section.
- Template ids: slug of the name (lowercase, `a-z0-9-`, max 40), unique (`-2`, `-3`…).
- Files are plain Markdown so they can be edited in any editor; the app re-reads them on every list/summarize.

## Review Focus

1. Template file without frontmatter or with an empty name → still listed, name = file stem. (Task 1 test `file_without_frontmatter_uses_stem`.)
2. Meeting summarized with a template that was deleted since → falls back to "algemeen" instead of failing. (Task 1 test `missing_template_falls_back`.)
3. Name with `/`, `:` or only emoji → valid, unique id. (Task 1 test `slug_is_safe_and_unique`.)
4. Deleting a built-in id that has no override file → `NotFound`, built-in stays. (Task 1 test `builtin_cannot_be_deleted`.)
5. Default template in settings points to a deleted custom template → picker shows "Algemeen". (Task 2: settings normalization `normalized_template`.)

---

### Task 1: Template store and prompt

**Files:**
- Create: `core/src/templates.rs`
- Modify: `core/src/summary.rs` (`summary_messages(spec: &TemplateSpec, notes, transcript)`; remove `TEMPLATES`/`meeting_templates`/`template_instruction`, keep the four built-ins as data in `templates.rs`), `core/src/engine.rs`, `core/src/lib.rs`

**Interfaces:**
- Produces (UniFFI record): `MeetingTemplate { id: String, name: String, builtin: bool, customized: bool }`.
- Produces: `TemplateSpec { name: String, instruction: String, sections: Vec<String> }`, `TemplateStore::new(dir: &Path)`, `list() -> Vec<MeetingTemplate>` (built-ins in fixed order, then custom by name), `spec(id) -> TemplateSpec` (fallback "algemeen"), `source(id) -> Option<String>` (file text, or rendered built-in), `save(id: Option<&str>, name: &str, body: &str) -> Result<String, MusliError>` (returns id), `delete(id) -> Result<(), MusliError>`.
- Produces (engine): `meeting_templates() -> Vec<MeetingTemplate>`, `template_source(id) -> Option<String>`, `save_template(id: Option<String>, name: String, body: String) -> Result<String, MusliError>`, `delete_template(id) -> Result<(), MusliError>`.

- [ ] **Step 1: Failing tests:** `builtins_listed_first` (ids `algemeen, een-op-een, klantgesprek, standup` then customs); `custom_sections_drive_prompt` (body "Wat ging goed en wat beter kan.\n\n## Goed\n## Beter\n## Actiepunten" → system prompt contains `## Goed\n## Beter\n## Actiepunten` and the instruction, not `## Beslissingen`); `default_sections_without_headings`; `override_builtin` (save with id `standup` → list shows standup `customized: true`, spec uses file); `file_without_frontmatter_uses_stem`; `missing_template_falls_back`; `slug_is_safe_and_unique` (`"Retro / Q4: 🚀"` → `retro-q4`, second save → `retro-q4-2`; `"🚀"` → `sjabloon`); `builtin_cannot_be_deleted`; `notes_rule_targets_first_section_without_samenvatting`.
- [ ] **Step 2–4:** fail → implement → green; existing summary tests adapted to `TemplateSpec`.
- [ ] **Step 5:** Commit `feat(core): custom summary templates as markdown files`.

### Task 2: Templates UI

**Files:**
- Create: `apps/macos/Sources/Musli/Windows/TemplateEditor.swift`
- Modify: `apps/macos/Sources/Musli/Windows/MeetingsPane.swift`, `apps/macos/Sources/Musli/Meetings/MeetingViews.swift` (picker uses `engine.meetingTemplates()`), `core/src/settings.rs` (no change needed if Swift falls back: picker shows `algemeen` when the stored id is missing)

- [ ] **Step 1:** Meetings pane: section "Sjablonen" listing templates (name, "eigen"/"aangepast" tag), buttons "Bewerk", "Verwijder" (custom/customized only, confirmation), "Nieuw sjabloon".
- [ ] **Step 2:** `TemplateEditor` sheet: name field, monospaced `TextEditor` with the source (body without frontmatter), help text "Schrijf wat de samenvatting moet doen. Elke regel die met ## begint wordt een vaste sectie.", live list of detected sections, Save/Cancel. Built-in "Bewerk" saves an override with the same id.
- [ ] **Step 3:** Default-template and meeting pickers list `engine.meetingTemplates()`; unknown stored id shows as "Algemeen".
- [ ] **Step 4:** `make app`; commit `feat(macos): edit summary templates`.
