import assert from "node:assert/strict";
import test from "node:test";
import { initialEngine, manualPracticeAllowed, onboardingProgressChange, onboardingSteps, resumeStep, stepIndex } from "../src/onboarding-state.ts";

const settings = { providers: [{ id: "local", kind: "local" }], dictation_stt: { provider_id: "local", model: "whisper" } };

test("unknown microphone permission permits a real manual test without global shortcut rights", () => {
  assert.equal(manualPracticeAllowed(null, [{ kind: "microphone", status: "unknown" }, { kind: "accessibility", status: "denied" }], settings), true);
  assert.equal(manualPracticeAllowed({ configured: true, can_record: true, can_use_shortcut: false }, [], settings), true);
});

test("denied microphone and unavailable chosen recognizer do not falsely pass onboarding", () => {
  assert.equal(manualPracticeAllowed(null, [{ kind: "microphone", status: "denied" }], settings), false);
  assert.equal(manualPracticeAllowed({ configured: false, can_record: false }, [], settings), false);
});

test("Apple speech requires its own authorization, unlike local speech", () => {
  const apple = { providers: [{ id: "apple", kind: "apple_speech" }], dictation_stt: { provider_id: "apple", model: "dictation" } };
  assert.equal(manualPracticeAllowed(null, [{ kind: "microphone", status: "granted" }, { kind: "speech", status: "denied" }], apple), false);
  assert.equal(manualPracticeAllowed(null, [{ kind: "microphone", status: "granted" }, { kind: "speech", status: "granted" }], apple), true);
});

test("unknown saved wizard steps fall back to Welcome", () => {
  assert.equal(stepIndex("practice"), 3);
  assert.equal(stepIndex("old-step"), 0);
});

test("a successful initial practice snapshot completes once without an identical persistence loop", () => {
  const pending = { version: 1, status: "pending", resume_step: "practice" };
  const completed = onboardingProgressChange(pending, "practice", "completed");
  assert.deepEqual(completed, { version: 1, status: "completed", resume_step: "practice" });
  assert.equal(onboardingProgressChange(completed, "practice", "completed"), null);
  assert.equal(onboardingProgressChange(completed, "practice"), null);
});

test("replaying or skipping a completed wizard never downgrades completion", () => {
  const completed = { version: 1, status: "completed", resume_step: "practice" };
  assert.deepEqual(onboardingProgressChange(completed, "engine", "skipped"), { version: 1, status: "completed", resume_step: "engine" });
});

test("model chooser keeps a valid selection and defaults to the recommended model", async () => {
  const { defaultModelId, modelOptionLabel } = await import("../src/onboarding-state.ts");
  const models = [{ id: "a", name: "Klein", size_bytes: 150e6, recommended: false }, { id: "b", name: "Groot", size_bytes: 1.2e9, recommended: true }];
  assert.equal(defaultModelId(models, "a"), "a");
  assert.equal(defaultModelId(models, "zzz"), "b");
  assert.equal(defaultModelId([], "x"), "x");
  const size = (bytes) => `${bytes / 1e6} MB`;
  assert.equal(modelOptionLabel(models[1], size), "Groot · 1200 MB · Aanbevolen");
  assert.equal(modelOptionLabel(models[0], size), "Klein · 150 MB");
});

test("learning your voice is the optional last step, after the practice", () => {
  assert.deepEqual(onboardingSteps.slice(-2), ["practice", "voice"]);
  assert.equal(stepIndex("voice"), 4);
});

test("resume returns to the voice step once the practice is done", () => {
  assert.equal(resumeStep("practice", "voice"), "voice");
  assert.equal(resumeStep("practice", "practice"), "practice");
  assert.equal(resumeStep("engine", "voice"), "engine");
  assert.equal(resumeStep("permissions", undefined), "permissions");
});

test("finishing or skipping the voice step completes the onboarding", () => {
  const pending = { version: 1, status: "pending", resume_step: "voice" };
  assert.deepEqual(onboardingProgressChange(pending, "voice", "completed"), { version: 1, status: "completed", resume_step: "voice" });
  assert.deepEqual(onboardingProgressChange({ ...pending, resume_step: "practice" }, "voice"), pending);
});

test("a processor too old for local models starts onboarding on a provider", () => {
  assert.equal(initialEngine(undefined, true), "local");
  assert.equal(initialEngine("local", true), "local");
  assert.equal(initialEngine(undefined, false), "custom");
  assert.equal(initialEngine("local", false), "custom");
  assert.equal(initialEngine("apple_speech", false), "apple");
  assert.equal(initialEngine("open_ai", true), "custom");
});
