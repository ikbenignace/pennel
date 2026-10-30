import assert from "node:assert/strict";
import test from "node:test";
import { inferenceConfig, inferenceFingerprint, InferenceVerificationState } from "../src/inference-state.ts";

const config = { provider_id: "custom", protocol: "mercury", model: "decision-model", expected_model: null, upstream: null, max_input_bytes: 16000, timeout_secs: 30 };
const info = { settings: { providers: [{ id: "custom", kind: "custom", name: "My server", base_url: "https://example.com/v1" }], decision: config }, keys: { custom: true }, provider_revisions: { custom: 1 } };
const result = { verified: true, message: "Sample passed", identity: null };

test("old settings do not implicitly configure either inference capability", () => {
  assert.equal(inferenceConfig({ settings: { providers: [] } }, "decision"), null);
  assert.equal(inferenceConfig(info, "reranking"), null);
});

test("inference verification identity includes protocol, pinning, budgets, endpoint and credential revisions", () => {
  const fingerprint = inferenceFingerprint(info, "decision");
  for (const change of [
    { protocol: "perplexity" }, { model: "other" }, { upstream: "other" }, { expected_model: "versioned" }, { max_input_bytes: 12000 }, { timeout_secs: 15 },
  ]) assert.notEqual(inferenceFingerprint({ ...info, settings: { ...info.settings, decision: { ...config, ...change } } }, "decision"), fingerprint);
  assert.notEqual(inferenceFingerprint({ ...info, provider_revisions: { custom: 2 } }, "decision"), fingerprint);
  assert.notEqual(inferenceFingerprint({ ...info, keys: { custom: false } }, "decision"), fingerprint);
  assert.notEqual(inferenceFingerprint({ ...info, settings: { ...info.settings, providers: [{ ...info.settings.providers[0], base_url: "https://other.example/v1" }] } }, "decision"), fingerprint);
});

test("late verification cannot verify a changed configuration", () => {
  const state = new InferenceVerificationState();
  assert.equal(state.begin("old", "before"), true);
  assert.equal(state.begin("duplicate", "before"), false);
  assert.equal(state.finish("old", "after", result), false);
  assert.equal(state.get("after"), null);
  assert.equal(state.running, false);
});

test("cancellation invalidates an old response without losing a newer request", () => {
  const state = new InferenceVerificationState();
  state.begin("old", "before");
  assert.equal(state.invalidate(), "old");
  state.begin("new", "after");
  assert.equal(state.finish("old", "before", result), false);
  assert.equal(state.running, true);
  assert.equal(state.finish("new", "after", result), true);
  assert.equal(state.get("after"), result);
  assert.equal(state.get("changed"), null);
});
