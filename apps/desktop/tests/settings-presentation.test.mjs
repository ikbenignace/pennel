import assert from "node:assert/strict";
import test from "node:test";
import { setInterfaceLanguage } from "../src/i18n.ts";
import { localModelLanguageName, recordActionOptions, templateDisplayName } from "../src/settings-presentation.ts";

test("builtin template labels follow the interface while customized names stay literal", () => {
  const template = { id: "algemeen", name: "Algemeen", builtin: true, customized: false };
  setInterfaceLanguage("nl");
  const label = templateDisplayName(template);
  assert.equal(String(label), "Algemeen");
  setInterfaceLanguage("en");
  assert.equal(String(label), "General");
  assert.equal(templateDisplayName({ ...template, customized: true, name: "Mijn eigen inhoud {name}" }), "Mijn eigen inhoud {name}");
  assert.deepEqual(template, { id: "algemeen", name: "Algemeen", builtin: true, customized: false });
});

test("recording options retain stable values and language names follow the interface", () => {
  setInterfaceLanguage("nl");
  const options = recordActionOptions();
  assert.deepEqual(options.map(([value, label]) => [value, String(label)]), [["ask", "Vragen"], ["always", "Altijd"], ["never", "Nooit"]]);
  assert.equal(localModelLanguageName("de"), "Duits");
  setInterfaceLanguage("en");
  assert.deepEqual(options.map(([value, label]) => [value, String(label)]), [["ask", "Ask"], ["always", "Always"], ["never", "Never"]]);
  assert.equal(localModelLanguageName("de"), "German");
});
