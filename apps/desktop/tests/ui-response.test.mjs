import test from "node:test";
import assert from "node:assert/strict";
import { getInterfaceLanguage, setInterfaceLanguage } from "../src/i18n.ts";
import { localizeUiResponse } from "../src/ui-response.ts";

test("metadata follows the response locale without changing runtime state or speech configuration", () => {
  const settings = { ui_language: "nl", language: "nl", mode: "fast", dictionary: ["Kopiëren"], providers: [{ name: "Mijn provider" }] };
  setInterfaceLanguage("en");
  const response = localizeUiResponse("save_settings", { interface_language: "nl", settings, kinds: [], capabilities: {}, hotkey_error: "Choose a speech model." });
  assert.equal(response.hotkey_error, "Kies een spraakmodel.");
  assert.equal(getInterfaceLanguage(), "en");
  assert.deepEqual(response.settings, settings);
  localizeUiResponse("save_settings", { interface_language: "en", system_interface_language: "en", settings: { ...settings, ui_language: "system" }, kinds: [], capabilities: {} });
  assert.equal(getInterfaceLanguage(), "en");
  setInterfaceLanguage("en");
});

test("only known UI message fields translate; identical user words and machine states do not", () => {
  setInterfaceLanguage("en");
  const content = "Kies een spraakmodel.";
  const status = { error: content, notes: content, title: content, transcript: [{ text: content }], phase: "error", session_id: content };
  const result = localizeUiResponse("meeting_status", status);
  assert.notEqual(result.error, content);
  assert.equal(result.notes, content);
  assert.equal(result.title, content);
  assert.equal(result.session_id, content);
  assert.equal(result.phase, "error");
  assert.deepEqual(result.transcript, status.transcript);
  assert.deepEqual(status.error, content);
  assert.deepEqual(localizeUiResponse("history", [{ text: content, raw: content }]), [{ text: content, raw: content }]);
  const arbitrary = { settings: { ui_language: "nl" }, kinds: [], hotkey_error: content };
  assert.deepEqual(localizeUiResponse("user_document", arbitrary), arbitrary);
  assert.equal(getInterfaceLanguage(), "en");
  assert.deepEqual(localizeUiResponse("download-done", { error: "cancelled" }), { error: "cancelled" });
});
