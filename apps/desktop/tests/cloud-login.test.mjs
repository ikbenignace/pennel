import assert from "node:assert/strict";
import test from "node:test";
import { accountScreenFor, afterSignOut, copyStatusText, formatUsage, initialAccountUi, pollLogin, recoveryNeedsConfirmation } from "../src/cloud-login.ts";

function clock(start = 0) {
  const state = { time: start, sleeps: [] };
  return {
    state,
    now: () => state.time,
    sleep: async (ms) => { state.sleeps.push(ms); state.time += ms; },
  };
}

function scripted(...results) {
  let calls = 0;
  const poll = async () => results[Math.min(calls++, results.length - 1)];
  return { poll, calls: () => calls };
}

test("login completes after two pending polls", async () => {
  const time = clock();
  const script = scripted({ kind: "pending" }, { kind: "pending" }, { kind: "done", email: "a@b.c" });
  const outcome = await pollLogin(script.poll, { intervalMs: 5000, expiresAtMs: 900000, now: time.now, sleep: time.sleep });
  assert.equal(outcome, "done");
  assert.equal(script.calls(), 3);
  assert.deepEqual(time.state.sleeps, [5000, 5000, 5000]);
});

test("slow_down adds five seconds to every later wait", async () => {
  const time = clock();
  const script = scripted({ kind: "slow_down" }, { kind: "pending" }, { kind: "done", email: "a@b.c" });
  await pollLogin(script.poll, { intervalMs: 5000, expiresAtMs: 900000, now: time.now, sleep: time.sleep });
  assert.deepEqual(time.state.sleeps, [5000, 10000, 10000]);
});

test("login expires at the deadline without polling again", async () => {
  const time = clock();
  const script = scripted({ kind: "pending" });
  const outcome = await pollLogin(script.poll, { intervalMs: 5000, expiresAtMs: 12000, now: time.now, sleep: time.sleep });
  assert.equal(outcome, "expired");
  assert.equal(script.calls(), 2);
  assert.equal(time.state.time, 15000);
});

test("an expired answer from the server ends the login", async () => {
  const time = clock();
  const outcome = await pollLogin(scripted({ kind: "expired" }).poll, { intervalMs: 5000, expiresAtMs: 900000, now: time.now, sleep: time.sleep });
  assert.equal(outcome, "expired");
});

test("cancelling stops polling", async () => {
  const time = clock();
  let cancelled = false;
  const script = scripted({ kind: "pending" });
  const outcome = await pollLogin(async () => { cancelled = true; return script.poll(); }, {
    intervalMs: 5000, expiresAtMs: 900000, now: time.now, sleep: time.sleep, cancelled: () => cancelled,
  });
  assert.equal(outcome, "cancelled");
  assert.equal(script.calls(), 1);
});

test("usage is formatted in Dutch", () => {
  assert.deepEqual(formatUsage({
    audio_seconds: 12000, tokens: 400000, storage_bytes: 12e6,
    limit_audio_seconds: 72000, limit_tokens: 2e6, limit_storage_bytes: 1e9,
  }), { audio: "3 u 20 min van 20 u", tokens: "0,4 mln van 2 mln tokens", storage: "12 MB van 1 GB" });
});

test("usage handles zero and sub-hour values", () => {
  const usage = formatUsage({
    audio_seconds: 0, tokens: 0, storage_bytes: 0,
    limit_audio_seconds: 72000, limit_tokens: 2e6, limit_storage_bytes: 1e9,
  });
  assert.equal(usage.audio, "0 min van 20 u");
  assert.equal(usage.tokens, "0 mln van 2 mln tokens");
  assert.equal(usage.storage, "0 MB van 1 GB");
});

test("recovery code confirmation is only asked while explicitly unconfirmed", () => {
  const base = { signedIn: true, syncOn: true };
  assert.equal(recoveryNeedsConfirmation({ ...base, confirmed: false }), true);
  assert.equal(recoveryNeedsConfirmation({ ...base, confirmed: true }), false);
  assert.equal(recoveryNeedsConfirmation({ ...base, confirmed: null }), false);
  assert.equal(recoveryNeedsConfirmation({ signedIn: false, syncOn: true, confirmed: false }), false);
  assert.equal(recoveryNeedsConfirmation({ signedIn: true, syncOn: false, confirmed: false }), false);
});

test("logging out or deleting the account clears every local trace", () => {
  const dirty = {
    ...initialAccountUi(), shownCode: "ABCD", askCode: true, lostCode: true, codeDraft: "x", notice: "fout",
    syncing: true, offerCloud: true, login: { kind: "expired" },
  };
  assert.notDeepEqual(dirty, initialAccountUi());
  assert.deepEqual(afterSignOut(dirty), initialAccountUi());
});

test("not_signed_in leads to the signed-out screen, other errors do not", () => {
  assert.equal(accountScreenFor("not_signed_in"), "signed_out");
  assert.equal(accountScreenFor("error"), "signed_in");
  assert.equal(accountScreenFor(null), "signed_in");
});

test("a failed copy shows the error instead of the success text", () => {
  assert.equal(copyStatusText(null), "Gekopieerd");
  assert.equal(copyStatusText("Kopiëren is mislukt."), "Kopiëren is mislukt.");
});
