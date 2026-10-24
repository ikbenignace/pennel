import assert from "node:assert/strict";
import test from "node:test";
import { LatestRequest, SerialQueue, persistAll } from "../src/async.ts";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("overlapping settings changes read the last acknowledged settings", async () => {
  const queue = new SerialQueue();
  const writing = deferred();
  const release = deferred();
  let settings = { language: "nl", auto_detect: true };
  const updates = [];

  function save(change) {
    return queue.run(async () => {
      const next = structuredClone(settings);
      change(next);
      updates.push(next);
      if (updates.length === 1) {
        writing.resolve();
        await release.promise;
      }
      settings = next;
      return next;
    });
  }

  const first = save((next) => { next.language = "fr"; });
  const second = save((next) => { next.auto_detect = false; });
  await writing.promise;
  assert.equal(updates.length, 1);
  release.resolve();
  await Promise.all([first, second]);
  assert.deepEqual(settings, { language: "fr", auto_detect: false });
});

test("a failed save does not block later changes or commit failed state", async () => {
  const queue = new SerialQueue();
  let settings = { language: "nl", auto_detect: true };
  const first = queue.run(async () => { throw new Error("storage unavailable"); });
  const rejected = assert.rejects(first, /storage unavailable/);
  const second = queue.run(async () => {
    settings = { ...settings, auto_detect: false };
  });
  await Promise.all([rejected, second]);
  assert.deepEqual(settings, { language: "nl", auto_detect: false });
});

test("a late search result cannot replace a newer search", async () => {
  const requests = new LatestRequest();
  const old = deferred();
  const recent = deferred();
  let visible;
  const first = requests.run(() => old.promise, (value) => { visible = value; });
  const second = requests.run(() => recent.promise, (value) => { visible = value; });
  recent.resolve(["new result"]);
  assert.equal(await second, true);
  old.resolve(["old result"]);
  assert.equal(await first, false);
  assert.deepEqual(visible, ["new result"]);
});

test("a stale failure cannot replace the current search state", async () => {
  const requests = new LatestRequest();
  const old = deferred();
  let visible;
  const first = requests.run(
    () => old.promise,
    (value) => { visible = value; },
    () => { visible = "error"; },
  );
  await requests.run(async () => "current", (value) => { visible = value; });
  old.reject(new Error("old request failed"));
  assert.equal(await first, false);
  assert.equal(visible, "current");
});

test("closing a view invalidates its in-flight result", async () => {
  const requests = new LatestRequest();
  const waiting = deferred();
  let accepted = false;
  const pending = requests.run(() => waiting.promise, () => { accepted = true; });
  requests.cancel();
  waiting.resolve("result");
  assert.equal(await pending, false);
  assert.equal(accepted, false);
});

test("the current request reports failures through its own error handler", async () => {
  const requests = new LatestRequest();
  const failure = new Error("not connected");
  let received;
  assert.equal(await requests.run(
    async () => { throw failure; },
    () => assert.fail("failed request was accepted"),
    (error) => { received = error; },
  ), true);
  assert.equal(received, failure);
});

test("draining waits for the queued save before reporting it finished", async () => {
  const queue = new SerialQueue();
  const waiting = deferred();
  let persisted = false;
  queue.run(async () => { await waiting.promise; persisted = true; });
  let drained = false;
  const drain = queue.drain().then(() => { drained = true; });
  await Promise.resolve();
  assert.equal(drained, false);
  waiting.resolve();
  await drain;
  assert.equal(persisted, true);
  assert.equal(drained, true);
});

test("one failed persistence task does not skip another pending draft", async () => {
  const waiting = deferred();
  let otherStarted = false;
  let otherSaved = false;
  const saving = persistAll([
    async () => { throw new Error("settings disk full"); },
    async () => { otherStarted = true; await waiting.promise; otherSaved = true; },
  ]);
  const rejected = assert.rejects(saving, /settings disk full/);
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(otherStarted, true);
  assert.equal(otherSaved, false);
  waiting.resolve();
  await rejected;
  assert.equal(otherSaved, true);
});

test("successful independent persistence waits for every task", async () => {
  const saved = [];
  await persistAll([async () => { saved.push("notes"); }, async () => { saved.push("title"); }]);
  assert.deepEqual(saved.sort(), ["notes", "title"]);
});
