import assert from "node:assert/strict";
import test from "node:test";
import { VisibilityPoll } from "../src/window-lifecycle.ts";

function clock() {
  const timers = new Map();
  let next = 0;
  return {
    timers,
    set: (work) => { timers.set(++next, work); return next; },
    clear: (id) => { timers.delete(id); },
    tick: () => { for (const work of timers.values()) work(); },
  };
}

test("a hidden workspace does not poll, and reopening refreshes once without duplicate timers", async () => {
  const timers = clock();
  let requests = 0;
  const poll = new VisibilityPoll(1000, async () => { requests += 1; }, assert.fail, timers);
  poll.setVisible(false);
  timers.tick();
  assert.equal(requests, 0);
  poll.setVisible(true);
  poll.setVisible(true);
  await Promise.resolve();
  assert.equal(requests, 1);
  assert.equal(timers.timers.size, 1);
  poll.setVisible(false);
  assert.equal(timers.timers.size, 0);
  poll.setVisible(true);
  await Promise.resolve();
  assert.equal(requests, 2);
  assert.equal(timers.timers.size, 1);
  poll.dispose();
  assert.equal(timers.timers.size, 0);
});

test("reopening during an in-flight request refreshes immediately after that request", async () => {
  const timers = clock();
  let release;
  let requests = 0;
  const waiting = new Promise((resolve) => { release = resolve; });
  const poll = new VisibilityPoll(1000, async () => { requests += 1; if (requests === 1) await waiting; }, assert.fail, timers);
  poll.setVisible(true);
  poll.setVisible(false);
  poll.setVisible(true);
  timers.tick();
  assert.equal(requests, 1);
  release();
  await waiting;
  await Promise.resolve();
  assert.equal(requests, 2);
  poll.dispose();
});

test("hiding or disposing while a request runs prevents a queued refresh", async () => {
  for (const dispose of [false, true]) {
    const timers = clock();
    let release;
    let requests = 0;
    const waiting = new Promise((resolve) => { release = resolve; });
    const poll = new VisibilityPoll(1000, async () => { requests += 1; await waiting; }, assert.fail, timers);
    poll.setVisible(true);
    timers.tick();
    if (dispose) poll.dispose(); else poll.setVisible(false);
    release();
    await waiting;
    await Promise.resolve();
    assert.equal(requests, 1);
    assert.equal(timers.timers.size, 0);
  }
});
