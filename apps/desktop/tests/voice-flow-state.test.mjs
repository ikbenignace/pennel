import assert from "node:assert/strict";
import test from "node:test";
import { recordingEntry, summaryControl } from "../src/voice-flow-state.ts";

const idle = { busy: false, phase: "idle", id: null, session_id: null, error: null, persistence_error: null, terminal_error: null, can_retry_persistence: false };

test("the persistent recording action starts a new session after an ordinary finished meeting", () => {
  assert.deepEqual(recordingEntry(idle), { kind: "new", meetingId: null });
  assert.deepEqual(recordingEntry({ ...idle, id: "previous", session_id: "old" }), { kind: "new", meetingId: null });
});

test("active, starting and finalizing recordings always return to their existing session", () => {
  assert.deepEqual(recordingEntry({ ...idle, busy: true, phase: "starting" }), { kind: "active", meetingId: null });
  assert.deepEqual(recordingEntry({ ...idle, busy: true, phase: "recording", id: "current" }), { kind: "active", meetingId: "current" });
  assert.deepEqual(recordingEntry({ ...idle, busy: true, phase: "finalizing", id: "current" }), { kind: "finishing", meetingId: "current" });
  assert.deepEqual(recordingEntry({ ...idle, busy: true, phase: "stopping", id: "current" }), { kind: "finishing", meetingId: "current" });
});

test("retained save failures expose recovery instead of presenting a second recording", () => {
  assert.deepEqual(recordingEntry({ ...idle, phase: "error", id: "retained", session_id: "failed", persistence_error: "Disk unavailable", can_retry_persistence: true }), { kind: "recovery", meetingId: "retained" });
  assert.deepEqual(recordingEntry({ ...idle, phase: "error", error: "Start failed" }), { kind: "new", meetingId: null });
});

test("missing text setup remains actionable before and after recording", () => {
  assert.deepEqual(summaryControl(false, false, false), { disabled: true, reason: "setup", canSetUp: true });
  assert.deepEqual(summaryControl(false, true, false), { disabled: true, reason: "setup", canSetUp: true });
  assert.deepEqual(summaryControl(true, true, false), { disabled: true, reason: "recording", canSetUp: false });
});

test("text setup changes enable summary generation without replacing the meeting view", () => {
  assert.equal(summaryControl(false, false, false).disabled, true);
  assert.deepEqual(summaryControl(true, false, false), { disabled: false, reason: null, canSetUp: false });
  assert.deepEqual(summaryControl(true, false, true), { disabled: true, reason: "working", canSetUp: false });
  assert.deepEqual(summaryControl(false, false, true), { disabled: true, reason: "working", canSetUp: false });
});
