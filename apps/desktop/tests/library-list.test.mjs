import assert from "node:assert/strict";
import test from "node:test";
import { ListTypeahead, listKeyIndex, reconcileChildren } from "../src/library-list.ts";

function list(...children) {
  return {
    children, moved: [], removed: [],
    removeChild(child) { this.removed.push(child); this.children.splice(this.children.indexOf(child), 1); },
    insertBefore(child, before) {
      this.moved.push(child);
      const prior = this.children.indexOf(child);
      if (prior >= 0) this.children.splice(prior, 1);
      this.children.splice(before === null ? this.children.length : this.children.indexOf(before), 0, child);
    },
  };
}

test("unchanged source rows are never detached or moved during refresh", () => {
  const parent = list("today", "a", "b", "earlier", "c");
  reconcileChildren(parent, ["today", "a", "b", "earlier", "c"]);
  assert.deepEqual(parent.moved, []);
  assert.deepEqual(parent.removed, []);
});

test("inserting and removing meetings preserves surviving row identities", () => {
  const a = {}, b = {}, c = {}, next = {};
  const parent = list(a, b, c);
  reconcileChildren(parent, [next, a, c]);
  assert.deepEqual(parent.children, [next, a, c]);
  assert.deepEqual(parent.removed, [b]);
  assert.deepEqual(parent.moved, [next]);
});

test("reordering meeting groups moves only the rows that need to move", () => {
  const parent = list("today", "a", "b", "earlier", "c");
  reconcileChildren(parent, ["today", "b", "a", "earlier", "c"]);
  assert.deepEqual(parent.moved, ["b"]);
  assert.deepEqual(parent.children, ["today", "b", "a", "earlier", "c"]);
});

test("source-list arrows clamp at edges and Home/End reach the entire list", () => {
  assert.equal(listKeyIndex("ArrowUp", 0, 50), 0);
  assert.equal(listKeyIndex("ArrowDown", 49, 50), 49);
  assert.equal(listKeyIndex("ArrowDown", -1, 50), 0);
  assert.equal(listKeyIndex("Home", 30, 50), 0);
  assert.equal(listKeyIndex("End", 30, 50), 49);
  assert.equal(listKeyIndex("ArrowDown", 0, 0), -1);
  assert.equal(listKeyIndex("Enter", 4, 5), -1);
});

test("type-to-select supports prefixes, repeated-letter cycling and accented titles", () => {
  const keys = new ListTypeahead();
  const titles = ["Alpha", "Apple", "Équipe", "Project"];
  assert.equal(keys.find(titles, 3, "a", 1000, "en"), 0);
  assert.equal(keys.find(titles, 0, "a", 1200, "en"), 1);
  assert.equal(keys.find(titles, 1, "e", 2200, "en"), 2);
  assert.equal(keys.find(titles, 2, "q", 2300, "en"), 2);
  assert.equal(keys.find(titles, 2, "p", 4000, "en"), 3);
  assert.equal(keys.find(titles, 3, "z", 5000, "en"), -1);
});
