import assert from "node:assert/strict";
import test from "node:test";
import { AccountActionEpoch } from "../src/account-actions.ts";

test("logout and identity transitions invalidate in-flight secret operations", async () => {
  const guard = new AccountActionEpoch();
  guard.observe("first@example.test");
  let finish;
  const response = new Promise(resolve => { finish = resolve; });
  const current = guard.begin();
  let shown = null;
  const operation = response.then(code => { if (current()) shown = code; });
  guard.invalidate();
  guard.observe(null);
  guard.observe("second@example.test");
  finish("old-secret");
  await operation;
  assert.equal(shown, null);
});

test("a newer recovery operation wins over an older response on the same account", () => {
  const guard = new AccountActionEpoch();
  guard.observe("first@example.test");
  const first = guard.begin();
  const second = guard.begin();
  assert.equal(first(), false);
  assert.equal(second(), true);
  assert.equal(guard.observe("first@example.test"), false);
  assert.equal(second(), true);
  assert.equal(guard.observe("second@example.test"), true);
  assert.equal(second(), false);
});

test("confirmation dialogs retain identity ownership without blocking unrelated same-account work", () => {
  const guard = new AccountActionEpoch();
  guard.observe("first@example.test");
  const ownsIdentity = guard.captureIdentity();
  guard.begin();
  assert.equal(ownsIdentity(), true);
  guard.observe(null);
  assert.equal(ownsIdentity(), false);
});
