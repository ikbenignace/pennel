import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import { visualModels } from "./model-catalog.mjs";

const script = await readFile(new URL("./desktop-platform.js", import.meta.url), "utf8");
const cases = JSON.parse(await readFile(new URL("./visual-cases.json", import.meta.url), "utf8"));
function fixture(name) {
  const item = cases.find(item => item.name === name);
  const context = vm.createContext({ location: { hostname: "127.0.0.1" }, structuredClone, queueMicrotask,
    __PENNEL_FIXTURE_OPTIONS__: { ...item, appearance: "dark" } });
  vm.runInContext(script, context);
  return context;
}

test("every visual scenario has a unique stable identifier and valid entry", () => {
  assert.equal(new Set(cases.map(item => item.name)).size, cases.length);
  for (const item of cases) {
    assert.match(item.name, /^[a-z][a-z0-9-]+$/);
    assert.ok(["workspace", "onboarding", "pill"].includes(item.window));
    if (item.window === "pill") assert.ok(["listening", "working"].includes(item.pill_state));
    else assert.ok(item.section || item.step);
  }
});

test("fixture app information is synthetic and never returns an API key", async () => {
  const context = fixture("onboarding-engine-custom");
  const info = await context.__PENNEL_PLATFORM__.invoke("app_info");
  assert.equal(info.settings.appearance, "dark");
  assert.equal(info.settings.dictation_stt.provider_id, "openai");
  assert.equal(info.settings.mode, "fast");
  assert.equal(info.preview, true);
  assert.ok(Object.values(info.keys).every(value => typeof value === "boolean"));
  await context.__PENNEL_PLATFORM__.invoke("set_key", { account: "openai", key: "synthetic-only" });
  assert.equal(JSON.stringify(await context.__PENNEL_PLATFORM__.invoke("app_info")).includes("synthetic-only"), false);
});

test("pad/practice sessions emit to the fixture listener and never invoke native transport", async () => {
  const context = fixture("dictation-pad");
  const api = context.__PENNEL_PLATFORM__;
  const events = [];
  await api.listen("dictation-session", event => events.push(event.payload));
  const session = await api.invoke("open_dictation_session", { kind: "pad" });
  await api.invoke("toggle_dictation_session", { sessionId: session.session_id });
  await api.invoke("toggle_dictation_session", { sessionId: session.session_id });
  assert.equal(events[0].phase, "listening");
  assert.equal(events[1].phase, "done");
  assert.match(await api.invoke("pad_text"), /Zullen we morgen/);
  assert.equal(context.__PENNEL_FIXTURE__.unknown.length, 0);
});

test("denied and empty states are explicit and unknown commands fail closed", async () => {
  const denied = fixture("onboarding-permission-denied");
  const readiness = await denied.__PENNEL_PLATFORM__.invoke("onboarding_readiness", { target: "practice" });
  assert.equal(readiness.can_record, false);
  const info = await denied.__PENNEL_PLATFORM__.invoke("app_info");
  assert.equal(info.capabilities.global_hotkey.status, "permission_required");
  assert.match(info.capabilities.global_hotkey.reason, /fn in andere apps/);
  const empty = fixture("meetings-empty");
  assert.equal((await empty.__PENNEL_PLATFORM__.invoke("meetings")).length, 0);
  await assert.rejects(() => empty.__PENNEL_PLATFORM__.invoke("unexpected_real_service"), /Onbekend fixturecommando/);
});

test("visual model metadata uses the bundled core catalog rather than invented scores", async () => {
  const catalog = JSON.parse(await readFile(new URL("../../core/src/catalog.json", import.meta.url), "utf8"));
  const source = await readFile(new URL("../../core/src/catalog.rs", import.meta.url), "utf8");
  const models = visualModels(catalog, source);
  assert.ok(models.length > 20);
  assert.equal(models[0].id, "parakeet-tdt-0.6b-v3");
  const whisper = models.find(model => model.id === "whisper-large-v3-turbo");
  assert.equal(whisper.size_bytes, 886381760);
  assert.equal(whisper.speed, 32);
  assert.equal(whisper.accuracy, 67);
  assert.ok(models.every(model => model.speed >= 0 && model.speed <= 100 && (model.accuracy === null || (model.accuracy >= 0 && model.accuracy <= 100))));
});

test("missing global shortcut permission does not block an existing user's manual pad", async () => {
  const context = fixture("dictation-pad-shortcut-denied");
  const info = await context.__PENNEL_PLATFORM__.invoke("app_info");
  const readiness = await context.__PENNEL_PLATFORM__.invoke("onboarding_readiness", { target: "dictation_pad" });
  assert.equal(info.settings.onboarding.status, "skipped");
  assert.equal(info.capabilities.microphone.status, "available");
  assert.equal(readiness.can_record, true);
  assert.equal(readiness.can_use_shortcut, false);
  assert.equal(readiness.can_paste_externally, false);
  await context.__PENNEL_PLATFORM__.invoke("request_permission", { kind: "accessibility" });
  assert.equal((await context.__PENNEL_PLATFORM__.invoke("app_info")).hotkey_error, null);
  assert.equal((await context.__PENNEL_PLATFORM__.invoke("onboarding_readiness", { target: "dictation_pad" })).can_use_shortcut, true);
});

test("the optional pad has an explicit route separate from the direct-dictation home", () => {
  const home = cases.find(item => item.name === "dictation");
  const pad = cases.find(item => item.name === "dictation-pad");
  const deniedPad = cases.find(item => item.name === "dictation-pad-shortcut-denied");
  assert.equal(home.tab, undefined);
  assert.equal(pad.section, home.section);
  assert.equal(pad.tab, "pad");
  assert.equal(deniedPad.tab, "pad");
  assert.equal(deniedPad.shortcut_denied, true);
});

test("pill snapshots provide only synthetic display text with no capture or transport calls", async () => {
  const captions = cases.filter(item => item.window === "pill");
  assert.equal(captions.length, 4);
  for (const item of captions) {
    const context = fixture(item.name);
    const snapshot = await context.__PENNEL_PLATFORM__.invoke("dictation_state");
    assert.equal(snapshot.state, item.pill_state);
    assert.equal(snapshot.preview.session_id, "qa-" + item.name);
    assert.equal(snapshot.preview.text, item.preview_text || "");
    assert.equal(snapshot.preview.revision, item.preview_text ? 7 : 0);
    assert.deepEqual(Array.from(context.__PENNEL_FIXTURE__.calls), ["dictation_state"]);
    assert.equal(context.__PENNEL_FIXTURE__.unknown.length, 0);
  }
  const long = cases.find(item => item.name === "pill-long-preview");
  assert.match(long.preview_text, /<b>blijft letterlijk<\/b>/);
  assert.match(long.preview_text, /日本語/);
  assert.match(long.preview_text, /👨‍👩‍👧‍👦/);
});

test("cloud scenarios are synthetic: signed out by default, signed in only when requested", async () => {
  const signedOut = fixture("settings-account");
  await assert.rejects(() => signedOut.__PENNEL_PLATFORM__.invoke("cloud_account"), (error) => error.code === "not_signed_in");
  assert.equal((await signedOut.__PENNEL_PLATFORM__.invoke("cloud_sync_last")).status.kind, "signed_out");
  assert.equal((await signedOut.__PENNEL_PLATFORM__.invoke("app_info")).settings.cloud_sync, false);
  const synced = fixture("settings-account-synced");
  assert.match((await synced.__PENNEL_PLATFORM__.invoke("cloud_account")).email, /@example\.com$/);
  assert.equal((await synced.__PENNEL_PLATFORM__.invoke("app_info")).settings.cloud_sync, true);
  const conflict = fixture("meeting-sync-conflict");
  assert.equal((await conflict.__PENNEL_PLATFORM__.invoke("meeting_sync_states"))[0].state, "conflict");
  assert.equal((await conflict.__PENNEL_PLATFORM__.invoke("meeting_conflict_files")).length, 1);
  for (const context of [signedOut, synced, conflict]) assert.equal(context.__PENNEL_FIXTURE__.unknown.length, 0);
});
