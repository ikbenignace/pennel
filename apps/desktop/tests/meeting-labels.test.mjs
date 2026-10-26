import assert from "node:assert/strict";
import test from "node:test";
import { setInterfaceLanguage } from "../src/i18n.ts";
import { dayGroup, meetingDate, sourceSpeakerLabel, speakerLabel, speakerUiLabel, templateLabel, templateUiLabel } from "../src/meeting-labels.ts";

const others = { speaker: "others", voice: 2 };

test("interface speaker names switch while source prefixes remain stable", () => {
  const label = speakerUiLabel(others);
  setInterfaceLanguage("nl");
  assert.equal(speakerLabel(others), "Anderen 2");
  assert.equal(String(label), "Anderen 2");
  assert.equal(sourceSpeakerLabel(others), "Anderen 2");
  setInterfaceLanguage("en");
  assert.equal(speakerLabel(others), "Speaker 2");
  assert.equal(String(label), "Speaker 2");
  assert.equal(sourceSpeakerLabel(others), "Anderen 2");
  assert.equal(speakerLabel({ speaker: "me", voice: null }), "Me");
  assert.equal(sourceSpeakerLabel({ speaker: "me", voice: null }), "Ik");
});

test("remembered speaker names remain verbatim in both interfaces", () => {
  const detail = { speakers: [{ voice: 2, name: "Samenvatting {number}" }] };
  for (const language of ["nl", "en"]) {
    setInterfaceLanguage(language);
    assert.equal(speakerLabel(others, detail), "Samenvatting {number}");
    assert.equal(String(speakerUiLabel(others, detail)), "Samenvatting {number}");
    assert.equal(sourceSpeakerLabel(others, detail), "Samenvatting {number}");
  }
});

test("only uncustomized built-in template names are translated", () => {
  const original = { id: "algemeen", name: "Algemeen", builtin: true, customized: false };
  const label = templateUiLabel(original);
  setInterfaceLanguage("nl");
  assert.equal(templateLabel(original), "Algemeen");
  assert.equal(String(label), "Algemeen");
  setInterfaceLanguage("en");
  assert.equal(templateLabel(original), "General");
  assert.equal(String(label), "General");
  assert.equal(templateLabel({ ...original, customized: true }), "Algemeen");
  assert.equal(String(templateUiLabel({ ...original, customized: true })), "Algemeen");
  assert.equal(templateLabel({ ...original, builtin: false }), "Algemeen");
});

test("meeting dates and relative groups use the interface locale", () => {
  const now = new Date("2026-10-26T12:00:00Z");
  setInterfaceLanguage("nl");
  assert.equal(dayGroup("2026-10-26T12:00:00Z", now), "Vandaag");
  assert.equal(dayGroup("2026-10-23T12:00:00Z", now), "Deze week");
  assert.equal(dayGroup("2026-10-01T12:00:00Z", now), "Eerder");
  const dutch = meetingDate("2026-10-26T12:00:00Z");
  setInterfaceLanguage("en");
  assert.equal(dayGroup("2026-10-26T12:00:00Z", now), "Today");
  assert.equal(dayGroup("2026-10-23T12:00:00Z", now), "This week");
  assert.equal(dayGroup("2026-10-01T12:00:00Z", now), "Earlier");
  assert.notEqual(meetingDate("2026-10-26T12:00:00Z"), dutch);
  assert.equal(meetingDate("not a date"), "not a date");
});
