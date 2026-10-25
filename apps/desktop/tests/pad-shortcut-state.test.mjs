import assert from "node:assert/strict";
import test from "node:test";
import { shortcutAccessHint } from "../src/pad-shortcut-state.ts";

const manualReady = Object.freeze({ configured: true, can_record: true, can_use_shortcut: false, can_paste_externally: false, reason: null, resume_step: "practice" });

test("revoked shortcut rights offer recovery while manual recording remains available", () => {
  const hint = shortcutAccessHint(manualReady, { status: "permission_required", reason: "Geef Pennel toegang tot Toegankelijkheid." });
  assert.deepEqual(hint, { message: "Geef Pennel toegang tot Toegankelijkheid.", recoverable: true });
  assert.equal(manualReady.can_record, true);
});

test("an unavailable shortcut offers a permission/settings check", () => {
  const hint = shortcutAccessHint(manualReady, { status: "unavailable", reason: "De sneltoets kon niet starten." });
  assert.equal(hint.recoverable, true);
  assert.equal(hint.message, "De sneltoets kon niet starten.");
});

test("Wayland exposes desktop shortcut guidance without a wizard recovery loop", () => {
  const reason = "Wayland: stel een desktopsneltoets in voor pennel --toggle-dictation. Je kunt ook de knop gebruiken.";
  const hint = shortcutAccessHint(manualReady, { status: "unsupported", reason });
  assert.deepEqual(hint, { message: reason, recoverable: false });
});

test("available shortcuts need no reminder and missing speech setup is not mislabeled as access failure", () => {
  assert.equal(shortcutAccessHint({ ...manualReady, can_use_shortcut: true }, { status: "available", reason: null }), null);
  assert.equal(shortcutAccessHint({ ...manualReady, can_record: false }, { status: "available", reason: null }), null);
});
