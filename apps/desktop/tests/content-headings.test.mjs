import assert from "node:assert/strict";
import test from "node:test";
import { contentHeadingTags } from "../src/content-headings.ts";

test("a generated note starts below the meeting's own title", () => {
  assert.deepEqual(contentHeadingTags([2, 2]), ["h2", "h2"]);
  assert.deepEqual(contentHeadingTags([1, 2, 3, 2]), ["h2", "h3", "h4", "h3"]);
});

test("a sparse generated hierarchy does not skip heading ranks", () => {
  assert.deepEqual(contentHeadingTags([1, 4, 6, 4, 1]), ["h2", "h3", "h4", "h3", "h2"]);
});

test("separately marked summary lines retain an accessible section level", () => {
  assert.deepEqual(contentHeadingTags([4]), ["h2"]);
  assert.deepEqual(contentHeadingTags([]), []);
});
