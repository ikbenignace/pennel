import { setInterfaceLanguage } from "../src/i18n.ts";
setInterfaceLanguage("nl");
import assert from "node:assert/strict";
import test from "node:test";
import { appInfoContentEqual, appearanceIsDark, applicationLabel, hotkeyLabel, languageModelConfigured } from "../src/presentation.ts";

test("the quick theme action reverses the effective System appearance", () => {
  assert.equal(appearanceIsDark("system", true), true);
  assert.equal(appearanceIsDark("system", false), false);
  assert.equal(appearanceIsDark("light", true), false);
  assert.equal(appearanceIsDark("dark", false), true);
});

test("shortcut labels describe the configured trigger rather than the platform default", () => {
  assert.equal(hotkeyLabel("right_option", "macos"), "rechter Option");
  assert.equal(hotkeyLabel("right_command", "macos"), "rechter Command");
  assert.equal(hotkeyLabel("control_super", "windows"), "Ctrl + Windows");
});

test("history uses app names instead of bundle and preview identifiers", () => {
  assert.equal(applicationLabel("com.apple.mail"), "Mail");
  assert.equal(applicationLabel("dev.pennel.Pennel.preview"), "Pennel");
  assert.equal(applicationLabel("com.example.Editor"), "Editor");
  assert.equal(applicationLabel("com.googlecode.iterm2"), "iTerm");
  assert.equal(applicationLabel("com.microsoft.Word"), "Word");
  assert.equal(applicationLabel("com.todesktop.230313mzl4w4u92"), "Cursor");
});

test("summary setup is unavailable without the chosen provider's key or capability", () => {
  const info = { settings: { llm: { provider_id: "groq", model: "model" }, providers: [{ id: "groq", kind: "groq" }] }, kinds: [{ kind: "groq", llm: true, requires_key: true, availability: { status: "available" } }], keys: {} };
  assert.equal(languageModelConfigured(info), false);
  info.keys.groq = true;
  assert.equal(languageModelConfigured(info), true);
  info.kinds[0].availability.status = "unavailable";
  assert.equal(languageModelConfigured(info), false);
});

test("appearance-only refreshes preserve form content despite unordered server maps", () => {
  const previous = { settings: { appearance: "light", dictionary: ["Pennel"] }, keys: { a: true, b: false }, version: "1" };
  const next = { version: "1", keys: { b: false, a: true }, settings: { dictionary: ["Pennel"], appearance: "dark" } };
  assert.equal(appInfoContentEqual(previous, next), true);
  assert.equal(appInfoContentEqual(next, structuredClone(next)), true);
});

test("actual dictionary and credential availability changes still refresh their views", () => {
  const previous = { settings: { appearance: "system", dictionary: ["Pennel"] }, keys: { a: false } };
  assert.equal(appInfoContentEqual(previous, { ...previous, settings: { ...previous.settings, dictionary: ["Pennel", "Nieuw"] } }), false);
  assert.equal(appInfoContentEqual(previous, { ...previous, keys: { a: true } }), false);
});


test("interface-only changes relabel existing views while actual settings changes still refresh", () => {
  const before = { interface_language: "en", system_interface_language: "en", settings: { ui_language: "system", appearance: "system", language: "fr", dictionary: ["Kopiëren"] }, keys: {} };
  const after = { ...before, interface_language: "nl", settings: { ...before.settings, ui_language: "nl" } };
  assert.equal(appInfoContentEqual(before, after), true);
  assert.equal(appInfoContentEqual(before, { ...after, settings: { ...after.settings, language: "en" } }), false);
});
