import assert from "node:assert/strict";
import test from "node:test";
import { retranscriptionControl, retranscriptionPercent } from "../src/meeting-retranscription.ts";

const recognition = { provider_kind: "local", provider_name: "Lokaal", model: "parakeet-tdt-0.6b-v3", language: "nl" };
const ready = { available: true, reason: null, recognition };
const running = { running: true, recognition, progress: { phase: "transcribing", speaker: "others", completed: 15_000, total: 60_000 }, error: null, warning: null };

test("completed local recordings can be reprocessed, but missing audio and setup retain their reason", () => {
  assert.deepEqual(retranscriptionControl(ready, null, false, false), { disabled: false, reason: null });
  assert.deepEqual(retranscriptionControl({ ...ready, available: false, reason: "Download het model" }, null, false, false), { disabled: true, reason: "Download het model" });
  assert.equal(retranscriptionControl(undefined, null, false, false).disabled, true);
});

test("an active recording or processing job blocks duplicate and conflicting actions", () => {
  assert.equal(retranscriptionControl(ready, running, false, false).disabled, true);
  assert.equal(retranscriptionControl(ready, null, true, false).disabled, true);
  assert.equal(retranscriptionControl(ready, null, false, true).disabled, true);
  assert.equal(retranscriptionControl(ready, { ...running, running: false }, false, false).disabled, false);
});

test("progress handles decoding and bounds late or malformed counters", () => {
  assert.equal(retranscriptionPercent(running.progress), 25);
  assert.equal(retranscriptionPercent({ ...running.progress, completed: 70_000 }), 100);
  assert.equal(retranscriptionPercent({ ...running.progress, completed: -1 }), 0);
  assert.equal(retranscriptionPercent({ ...running.progress, total: 0 }), null);
});
