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

test("prepare hide waits for saved drafts before closing the window-owned session", async () => {
  const barrier = new FrontendBarrier();
  let release;
  const waiting = new Promise((resolve) => { release = resolve; });
  const actions = [];
  const preparing = barrier.prepare(20, async () => { await waiting; actions.push("saved"); }, async () => { actions.push("closed"); });
  assert.equal(barrier.locked, true);
  assert.deepEqual(actions, []);
  release();
  await preparing;
  assert.deepEqual(actions, ["saved", "closed"]);
  assert.equal(barrier.locked, true);
  assert.equal(barrier.complete(20), true);
});

test("failed draft persistence leaves the owned session available for recovery", async () => {
  const barrier = new FrontendBarrier();
  let closed = false;
  await assert.rejects(barrier.prepare(21, async () => { throw new Error("Disk full"); }, async () => { closed = true; }), /Disk full/);
  assert.equal(closed, false);
  assert.equal(barrier.locked, true);
  barrier.complete(21);
  assert.equal(barrier.locked, false);
});

test("a timed-out hide cannot close a session after the window has reopened", async () => {
  const barrier = new FrontendBarrier();
  let release;
  const waiting = new Promise((resolve) => { release = resolve; });
  let closed = false;
  const preparing = barrier.prepare(22, async () => { await waiting; }, async () => { closed = true; });
  barrier.complete(22);
  release();
  assert.equal(await preparing, false);
  assert.equal(closed, false);
});

test("a native close cannot replace or complete an active quit or update barrier", async () => {
  for (const kind of ["quit", "update"]) {
    const barrier = new FrontendBarrier();
    barrier.begin(30, kind);
    let saved = false;
    let closed = false;
    const accepted = await barrier.prepare(31, async () => { saved = true; }, async () => { closed = true; }, "hide");
    assert.equal(accepted, false);
    assert.equal(saved, false);
    assert.equal(closed, false);
    assert.equal(barrier.kind, kind);
    assert.equal(barrier.isCurrent(30), true);
    assert.equal(barrier.complete(31, "hide"), false);
    assert.equal(barrier.complete(30, "hide"), false);
    assert.equal(barrier.locked, true);
    assert.equal(barrier.complete(30, kind), true);
    assert.equal(barrier.kind, null);
  }
});

test("a quit can supersede a pending hide while its later completion remains harmless", async () => {
  const barrier = new FrontendBarrier();
  let release;
  const saving = new Promise((resolve) => { release = resolve; });
  let closed = false;
  const hiding = barrier.prepare(40, async () => { await saving; }, async () => { closed = true; }, "hide");
  barrier.begin(41, "quit");
  release();
  assert.equal(await hiding, false);
  assert.equal(closed, false);
  assert.equal(barrier.complete(40, "hide"), false);
  assert.equal(barrier.kind, "quit");
  assert.equal(barrier.locked, true);
});
