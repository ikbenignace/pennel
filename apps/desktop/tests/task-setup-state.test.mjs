import assert from "node:assert/strict";
import test from "node:test";
import { TaskSetupScope, taskConfiguration } from "../src/task-setup-state.ts";

function info() {
  return {
    settings: {
      providers: [{ id: "local", kind: "local", name: "Local", base_url: "" }, { id: "text", kind: "groq", name: "Groq", base_url: "https://api.groq.com/openai/v1" }],
      dictation_stt: { provider_id: "local", model: "speech" },
      meeting_stt: { provider_id: "local", model: "speech" },
      llm: { provider_id: "text", model: "text-model" },
      mode: "fast",
    },
    keys: { text: false },
    kinds: [
      { kind: "local", stt: true, llm: false, requires_key: false, is_cli_agent: false, availability: { status: "available", reason: null } },
      { kind: "groq", stt: true, llm: true, requires_key: true, is_cli_agent: false, availability: { status: "available", reason: null } },
    ],
  };
}

test("task readiness keeps speech independent of optional text processing", () => {
  const current = info();
  assert.equal(taskConfiguration(current, "dictation_stt", ["speech"]).state, "ready");
  assert.equal(taskConfiguration(current, "meeting_stt", ["speech"]).state, "ready");
  assert.equal(taskConfiguration(current, "llm", ["speech"]).state, "key");
  current.keys.text = true;
  assert.equal(taskConfiguration(current, "llm", ["speech"]).state, "ready");
});

test("a selected local model must actually be downloaded, and an unsupported task stays unavailable", () => {
  const current = info();
  assert.equal(taskConfiguration(current, "dictation_stt", []).state, "download");
  current.settings.llm = { provider_id: "local", model: "speech" };
  assert.equal(taskConfiguration(current, "llm", ["speech"]).state, "unsupported");
  current.kinds[0].availability = { status: "unsupported", reason: "Unavailable here" };
  assert.equal(taskConfiguration(current, "meeting_stt", ["speech"]).reason, "Unavailable here");
});

test("setup scope ignores stale async views and keeps saves pending until settled", async () => {
  const scope = new TaskSetupScope();
  const first = scope.next();
  assert.equal(scope.current(first), true);
  const second = scope.next();
  assert.equal(scope.current(first), false);
  assert.equal(scope.current(second), true);
  let finish;
  const saving = scope.track(new Promise(resolve => { finish = resolve; }));
  let settled = false;
  void scope.settle().then(() => { settled = true; });
  await Promise.resolve();
  assert.equal(settled, false);
  finish();
  await saving;
  await scope.settle();
  assert.equal(settled, true);
  scope.dispose();
  assert.equal(scope.current(second), false);
});

test("failed saves are observable and a successful retry clears the failure", async () => {
  const scope = new TaskSetupScope();
  await assert.rejects(scope.track(Promise.reject(new Error("Cannot save"))), /Cannot save/);
  assert.match(String(scope.failure), /Cannot save/);
  await scope.track(Promise.resolve());
  assert.equal(scope.failure, null);
});

test("a flushed settings retry clears only its own failure, never a failed credential replacement", async () => {
  const scope = new TaskSetupScope();
  await assert.rejects(scope.track(Promise.reject(new Error("Settings unavailable")), "settings"));
  scope.settingsSaved();
  assert.equal(scope.failure, null);
  await assert.rejects(scope.track(Promise.reject(new Error("Credential save failed")), "mutation"));
  await assert.rejects(scope.track(Promise.reject(new Error("Settings unavailable")), "settings"));
  scope.settingsSaved();
  assert.match(String(scope.failure), /Credential save failed/);
  await scope.track(Promise.resolve(), "mutation");
  assert.equal(scope.failure, null);
});
