import assert from "node:assert/strict";
import test from "node:test";
import { NativeMenuSession, physicalMenuPosition } from "../src/native-menu-state.ts";

function deferred() {
  let resolve;
  const promise = new Promise(yes => { resolve = yes; });
  return { promise, resolve };
}

function menu() {
  return { shown: 0, closed: 0, async popup() { this.shown++; }, async close() { this.closed++; } };
}

test("native popup handles remain alive after popup resolves until selection or disposal", async () => {
  const resource = menu();
  const session = new NativeMenuSession(() => true, assert.fail);
  assert.equal(await session.show(async () => resource), true);
  assert.equal(resource.shown, 1);
  assert.equal(resource.closed, 0);
  let selected = 0;
  session.choose(() => { selected++; });
  session.choose(() => { selected++; });
  assert.equal(selected, 1);
  assert.equal(resource.closed, 1);
  session.close();
  assert.equal(resource.closed, 1);
});

test("a popup whose owner leaves while it is being created never appears", async () => {
  const created = deferred();
  const resource = menu();
  const session = new NativeMenuSession(() => true, assert.fail);
  const showing = session.show(() => created.promise);
  session.close();
  created.resolve(resource);
  assert.equal(await showing, false);
  assert.equal(resource.shown, 0);
  assert.equal(resource.closed, 1);
});

test("a detached or disabled owner cannot invoke a delayed native action", async () => {
  let available = true;
  const resource = menu();
  const session = new NativeMenuSession(() => available, assert.fail);
  await session.show(async () => resource);
  available = false;
  session.choose(() => assert.fail("stale action"));
  assert.equal(resource.closed, 1);
});

test("an owner that becomes unavailable while native popup is opening releases its menu", async () => {
  let available = true;
  const shown = deferred();
  const resource = menu();
  resource.popup = () => shown.promise;
  const session = new NativeMenuSession(() => available, assert.fail);
  const showing = session.show(async () => resource);
  await Promise.resolve();
  available = false;
  shown.resolve();
  assert.equal(await showing, false);
  assert.equal(resource.closed, 1);
});

test("popup failures release resources and propagate for the renderer fallback", async () => {
  const resource = menu();
  resource.popup = async () => { throw new Error("native popup unavailable"); };
  const session = new NativeMenuSession(() => true, assert.fail);
  await assert.rejects(session.show(async () => resource), /native popup unavailable/);
  assert.equal(resource.closed, 1);
});

test("async and synchronous menu action failures reach the owner", async () => {
  for (const run of [() => { throw new Error("save failed"); }, async () => { throw new Error("save failed"); }]) {
    const errors = [];
    const session = new NativeMenuSession(() => true, error => errors.push(error.message));
    await session.show(async () => menu());
    session.choose(run);
    await Promise.resolve();
    assert.deepEqual(errors, ["save failed"]);
  }
});

test("popup coordinates account for Retina scale and webview zoom exactly once", () => {
  const point = { x: 120, y: 48 };
  assert.deepEqual(physicalMenuPosition(point, 1000, 1000), { x: 120, y: 48 });
  assert.deepEqual(physicalMenuPosition(point, 1000, 2000), { x: 240, y: 96 });
  assert.deepEqual(physicalMenuPosition(point, 800, 2400), { x: 360, y: 144 });
  assert.deepEqual(physicalMenuPosition({ x: -2, y: 5.2 }, 0, 0), { x: 0, y: 5 });
});
