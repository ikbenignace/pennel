import assert from "node:assert/strict";
import test from "node:test";
import { HistoryFocusMemory } from "../src/history-focus-state.ts";

const available = new Set(["first", "second"]);
const options = { available, returning: true, unobstructed: true, requested: undefined, previous: null };

test("returning to History remembers the entry and action without changing stored content", () => {
  const memory = new HistoryFocusMemory();
  memory.remember("first", "copy");
  assert.deepEqual(memory.plan(options), { target: { id: "first", action: "copy" }, fallback: null, scroll: "nearest" });
  const snapshot = memory.selection(available);
  snapshot.id = "second";
  assert.equal(memory.selection(available).id, "first");
});

test("an explicit recovery target overrides memory and does not reuse another entry's action", () => {
  const memory = new HistoryFocusMemory();
  memory.remember("first", "delete");
  assert.deepEqual(memory.plan({ ...options, requested: "second" }), { target: { id: "second" }, fallback: null, scroll: "center" });
  assert.deepEqual(memory.selection(available), { id: "second" });
  assert.deepEqual(memory.plan({ ...options, requested: "expired" }), { target: null, fallback: "notice", scroll: "center" });
});

test("refreshes restore only lost row focus and never take focus from another control", () => {
  const memory = new HistoryFocusMemory();
  memory.remember("first", "copy");
  const none = { target: null, fallback: null, scroll: null };
  assert.deepEqual(memory.plan({ ...options, returning: false }), none);
  assert.deepEqual(memory.plan({ ...options, returning: false, previous: { id: "first", action: "copy" }, unobstructed: false }), none);
  assert.deepEqual(memory.plan({ ...options, unobstructed: false }), none);
  assert.deepEqual(memory.plan({ ...options, returning: false, previous: { id: "first", action: "copy" } }), { target: { id: "first", action: "copy" }, fallback: null, scroll: null });
});

test("removed entries are forgotten and deleting a focused row has a safe heading fallback", () => {
  const memory = new HistoryFocusMemory();
  memory.remember("expired", "delete");
  assert.equal(memory.selection(available), null);
  assert.deepEqual(memory.plan(options), { target: null, fallback: null, scroll: null });
  assert.deepEqual(memory.plan({ ...options, returning: false, previous: { id: "expired", action: "delete" } }), { target: null, fallback: "heading", scroll: null });
});
