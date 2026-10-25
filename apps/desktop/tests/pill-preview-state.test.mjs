import assert from "node:assert/strict";
import test from "node:test";
import { PillPreviewState, previewTail } from "../src/pill-preview-state.ts";
import { PILL_METRICS } from "../src/pill-metrics.generated.ts";

const update = (session_id, revision, text = "") => ({ session_id, revision, text });

test("only a session begin admits later preview updates", () => {
  const state = new PillPreviewState(PILL_METRICS.previewLimit);
  assert.equal(state.receive(update("old", 2, "Late text")), false);
  assert.equal(state.receive(update("current", 0)), true);
  assert.equal(state.receive(update("current", 1, "Voorlopig")), true);
  assert.equal(state.snapshot.text, "Voorlopig");
});

test("corrections replace text without replaying old revisions or sessions", () => {
  const state = new PillPreviewState(PILL_METRICS.previewLimit);
  state.receive(update("first", 0));
  state.receive(update("first", 1, "Morgen om twee"));
  state.receive(update("first", 2, "Morgen om tien"));
  assert.equal(state.receive(update("first", 1, "Morgen om twee")), false);
  assert.equal(state.receive(update("first", 0)), false);
  assert.equal(state.snapshot.text, "Morgen om tien");
  state.receive(update("next", 0));
  assert.equal(state.snapshot.text, "");
  assert.equal(state.receive(update("first", 3, "Oud dictaat")), false);
});

test("hiding clears ownership so late text cannot reappear", () => {
  const state = new PillPreviewState(PILL_METRICS.previewLimit);
  state.receive(update("first", 0));
  state.receive(update("first", 1, "Een eerste dictaat"));
  state.receive(null);
  assert.equal(state.receive(update("first", 2, "Laat resultaat")), false);
  assert.equal(state.snapshot, null);
});

test("boot snapshots recover the current revision before newer live events", () => {
  const state = new PillPreviewState(PILL_METRICS.previewLimit);
  state.restore(update("active", 7, "Tijdens het verwerken"));
  assert.equal(state.receive(update("active", 6, "Eerdere woorden")), false);
  assert.equal(state.receive(update("active", 8, "Het volledige resultaat")), true);
  assert.equal(state.snapshot.text, "Het volledige resultaat");
});

test("preview keeps the recent Unicode tail and treats markup as plain text", () => {
  const family = "👨‍👩‍👧‍👦";
  assert.equal(previewTail("  <b>Dit is tekst</b>\n\tgeen opmaak  ", PILL_METRICS.previewLimit), "<b>Dit is tekst</b> geen opmaak");
  assert.equal(previewTail("x".repeat(PILL_METRICS.previewLimit) + family, PILL_METRICS.previewLimit), `…${"x".repeat(PILL_METRICS.previewLimit - 1)}${family}`);
});

test("invalid revisions never replace a valid preview", () => {
  const state = new PillPreviewState(PILL_METRICS.previewLimit);
  state.receive(update("active", 0));
  for (const revision of [-1, 1.5, Number.NaN, Number.MAX_SAFE_INTEGER + 1]) {
    assert.equal(state.receive(update("active", revision, "Invalid")), false);
  }
  assert.equal(state.snapshot.text, "");
});
