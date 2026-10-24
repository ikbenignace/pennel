import assert from "node:assert/strict";
import test from "node:test";
import { FrontendBarrier, canShowFinished } from "../src/session-events.ts";

test("an old recording completion cannot unlock a newer quit barrier", () => {
  const barrier = new FrontendBarrier();
  barrier.begin(10);
  barrier.begin(11);
  assert.equal(barrier.complete(10), false);
  assert.equal(barrier.isCurrent(11), true);
  assert.equal(barrier.locked, true);
  assert.equal(barrier.complete(11), true);
  assert.equal(barrier.locked, false);
});

test("a delayed refresh checks the barrier again before unlocking", async () => {
  const barrier = new FrontendBarrier();
  let release;
  const waiting = new Promise((resolve) => { release = resolve; });
  barrier.begin(10);
  const resuming = barrier.resume(10, async () => { await waiting; });
  barrier.begin(11);
  release();
  assert.equal(await resuming, false);
  assert.equal(barrier.isCurrent(11), true);
  assert.equal(barrier.locked, true);
});

test("a stale resume never starts a refresh", async () => {
  const barrier = new FrontendBarrier();
  barrier.begin(11);
  let refreshed = false;
  assert.equal(await barrier.resume(10, async () => { refreshed = true; }), false);
  assert.equal(refreshed, false);
});

test("an old meeting cannot replace the next recording or its finished view", () => {
  assert.equal(canShowFinished({ busy: true, session_id: "session-b" }, "session-a", "session-b"), false);
  assert.equal(canShowFinished({ busy: false, session_id: null }, "session-a", "session-b"), false);
});

test("a completed meeting is shown only after the matching session is idle", () => {
  assert.equal(canShowFinished({ busy: true, session_id: "session-a" }, "session-a", "session-a"), false);
  assert.equal(canShowFinished({ busy: false, session_id: null }, "session-a", "session-a"), true);
  assert.equal(canShowFinished({ busy: false, session_id: null }, "session-a", null), true);
});

test("an old completion cannot replace a newer session error state", () => {
  assert.equal(canShowFinished({ busy: false, session_id: "session-b" }, "session-a", "session-a"), false);
});
