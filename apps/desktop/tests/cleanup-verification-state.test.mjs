import assert from "node:assert/strict";
import test from "node:test";
import { setInterfaceLanguage } from "../src/i18n.ts";
import { cleanupVerificationControl, cleanupVerificationCurrent, cleanupVerificationDestination, cleanupVerificationStage } from "../src/cleanup-verification-state.ts";

const status = { enabled: false, available: false, reason_code: "unqualified", provider_id: "decision", model: "selected-model", endpoint: "https://example.com/v1/decisions" };

test("configuring a decision connection never enables cleanup verification", () => {
  const settings = { mode: "clean", decision: { provider_id: "decision" } };
  assert.deepEqual(cleanupVerificationControl(settings, status), { enabled: false, disabled: true });
  assert.equal(cleanupVerificationStage(settings, status), null);
});

test("a saved enabled preference can always be disabled while qualification is unavailable", () => {
  setInterfaceLanguage("en");
  const settings = { mode: "clean", cleanup_verification_enabled: true };
  assert.deepEqual(cleanupVerificationControl(settings, { ...status, enabled: true }), { enabled: true, disabled: false });
  assert.match(cleanupVerificationStage(settings, { ...status, enabled: true }).destination, /original text/i);
  assert.equal(cleanupVerificationStage({ ...settings, mode: "fast" }, { ...status, enabled: true }), null);
});

test("active verification uses the frozen endpoint after settings select another server", () => {
  setInterfaceLanguage("en");
  const settings = { mode: "clean", cleanup_verification_enabled: false, decision: { provider_id: "replacement" } };
  const frozen = { ...status, enabled: true, available: true };
  assert.equal(cleanupVerificationStage(settings, frozen, true).destination, "example.com");
  assert.equal(cleanupVerificationStage(settings, { ...frozen, enabled: false }, true), null);
});

test("destinations hide URL secrets and explain device-local processing in English and Dutch", () => {
  for (const language of ["en", "nl"]) {
    setInterfaceLanguage(language);
    assert.equal(cleanupVerificationDestination({ ...status, endpoint: "https://name:secret@example.com/v1?key=secret" }), "example.com");
    const local = cleanupVerificationDestination({ ...status, endpoint: "http://127.0.0.1:8080/v1/decisions" });
    assert.match(local, language === "en" ? /this computer/ : /deze computer/);
    assert.match(local, /127\.0\.0\.1:8080/);
    assert.doesNotMatch(local, /secret/);
  }
});

test("a remote domain beginning with 127 is never described as processing on this computer", () => {
  for (const language of ["en", "nl"]) {
    setInterfaceLanguage(language);
    for (const endpoint of ["https://127.evil.invalid/v1", "https://128.0.0.1/v1"]) {
      const destination = cleanupVerificationDestination({ ...status, endpoint });
      assert.equal(destination, new URL(endpoint).host);
      assert.doesNotMatch(destination, /this computer|deze computer/);
    }
    for (const endpoint of ["http://127.40.50.60:8080/v1", "http://[::1]:8080/v1", "http://localhost:8080/v1"]) {
      const destination = cleanupVerificationDestination({ ...status, endpoint });
      assert.match(destination, language === "en" ? /this computer/ : /deze computer/);
      assert.match(destination, /8080/);
    }
  }
});

test("current session copy explains invalidation and Fast bypass without exposing template placeholders", () => {
  for (const language of ["en", "nl"]) {
    setInterfaceLanguage(language);
    const invalidated = cleanupVerificationCurrent({ ...status, enabled: true, reason_code: "configuration_changed" });
    assert.match(invalidated, language === "en" ? /connection changed/ : /verbinding veranderde/);
    assert.doesNotMatch(invalidated, /\{[^}]+\}/);
    const fast = { ...status, enabled: true, reason_code: "disabled" };
    assert.equal(cleanupVerificationStage({ mode: "clean" }, fast, true), null);
    assert.match(cleanupVerificationCurrent(fast), language === "en" ? /no text sent/ : /geen tekst verstuurd/);
  }
});
