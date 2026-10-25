import assert from "node:assert/strict";
import test from "node:test";
import { sourceRange, sourceBlocks, transcriptSourceBlocks } from "../src/source-navigation.ts";
import { WorkspaceNavigation, parseWorkspaceRoute, workspaceRoute } from "../src/workspace-state.ts";

test("passages match full source text with whitespace normalization and original selection offsets", () => {
  const text = "  # Besluit\n\n- Het café blijft open.\n";
  const range = sourceRange(text, "# Besluit\n- Het cafe\u0301 blijft open.");
  assert.deepEqual(range, { start: 2, end: text.lastIndexOf(".") + 1 });
  assert.equal(sourceRange(text, "Het café sluit."), null);
});

test("ambiguous repeated source text is never assigned to an unrelated occurrence", () => {
  assert.equal(sourceRange("Een besluit.\nEen besluit.", "Een besluit."), null);
  assert.equal(sourceRange("Een besluit.", ""), null);
  assert.equal(sourceRange("De planning wijzigt.", "plan"), null);
  assert.deepEqual(sourceBlocks(["# Besluiten", "- We bellen morgen.", "# Acties"], "# Besluiten\n- We bellen morgen."), { first: 0, last: 1 });
});

test("transcript citations verify packed speaker-labelled text at the cited timestamp", () => {
  const segments = [
    { start_ms: 0, end_ms: 4000, text: "Noor: De planning is klaar." },
    { start_ms: 4000, end_ms: 8000, text: "Ik: Ik stuur het voorstel." },
    { start_ms: 8000, end_ms: 12000, text: "Noor: Dank je." },
  ];
  assert.deepEqual(transcriptSourceBlocks(segments, { start_ms: 0, text: "Noor: De planning is klaar.\nIk: Ik stuur het voorstel." }), { first: 0, last: 1 });
  assert.equal(transcriptSourceBlocks(segments, { start_ms: 0, text: "Noor: Dank je." }), null);
  assert.equal(transcriptSourceBlocks(segments, { start_ms: 0, text: "Noor: De planning was gisteren klaar." }), null);
  assert.equal(transcriptSourceBlocks(segments, { start_ms: 50_000, text: "Noor: Dank je." }), null);
});

test("source routes validate targets, keep their text literal and clear one-shot jumps when remembered", () => {
  const target = { kind: "notes", start_ms: null, text: "<b>Letterlijke notitie</b>" };
  const route = workspaceRoute("meetings", null, "Projectoverleg", null, target);
  assert.deepEqual(parseWorkspaceRoute(route), route);
  assert.equal(route.source_target.text, target.text);
  const nav = new WorkspaceNavigation();
  nav.set(route);
  nav.set(workspaceRoute("history"));
  assert.deepEqual(nav.select("meetings"), workspaceRoute("meetings", null, "Projectoverleg"));
  for (const invalid of [{ kind: "other", text: "x", start_ms: null }, { kind: "notes", text: "", start_ms: null }, { kind: "transcript", text: "x", start_ms: -1 }]) {
    assert.equal(parseWorkspaceRoute({ ...route, source_target: invalid }), null);
  }
  assert.equal(parseWorkspaceRoute({ ...route, section: "history" }), null);
});
