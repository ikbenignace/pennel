import { setInterfaceLanguage } from "../src/i18n.ts";
setInterfaceLanguage("nl");
import assert from "node:assert/strict";
import test from "node:test";
import { meetingProcessingSummary, frozenRecognitionLabel, processingLabel, retainedMeetingFailure } from "../src/recording-processing.ts";
import { processingSummary } from "../src/processing-state.ts";

const settings = {
  providers: [
    { id: "speech", kind: "local", name: "Lokaal", base_url: "" },
    { id: "text", kind: "custom", name: "Tekstserver", base_url: "https://text.example/v1" },
  ],
  dictation_stt: { provider_id: "speech", model: "whisper" },
  meeting_stt: { provider_id: "speech", model: "whisper" },
  llm: { provider_id: "text", model: "cleanup" },
  mode: "fast", language: "nl",
};
const recognition = { provider_kind: "local", provider_name: "Lokaal", model: "whisper", language: "nl" };

test("live recognition describes the frozen provider, model and language rather than current settings", () => {
  const first = meetingProcessingSummary(processingSummary(settings, "meeting"), recognition, true);
  const changed = { ...settings, language: "en", meeting_stt: { provider_id: "text", model: "changed" }, llm: { provider_id: "speech", model: "later" } };
  const second = meetingProcessingSummary(processingSummary(changed, "meeting"), recognition, true);
  assert.equal(first[0].destination, second[0].destination);
  assert.match(first[0].destination, /Op deze computer.*Lokaal.*whisper.*Nederlands/);
  assert.notEqual(first[1].destination, second[1].destination);
  assert.match(first[1].destination, /alleen bij samenvatten of vragen/);
  assert.match(first[1].destination, /dan gekozen/);
});

test("unknown recording metadata never falls back to a possibly different current speech provider", () => {
  assert.equal(meetingProcessingSummary(processingSummary(settings, "meeting"), undefined, true)[0].destination, "Opnameconfiguratie niet beschikbaar");
  assert.equal(meetingProcessingSummary(processingSummary(settings, "meeting"))[0].destination, "Op deze computer");
  assert.equal(frozenRecognitionLabel({ ...recognition, provider_kind: null, provider_name: "Eigen herkenner", language: null }), "Eigen herkenner (bestemming niet vastgesteld) · whisper · Automatisch");
});

test("processing labels remain literal and custom providers receive no offline claim", () => {
  const custom = frozenRecognitionLabel({ ...recognition, provider_kind: "custom", provider_name: "Mijn server <b>", language: "de" });
  assert.match(custom, /Mijn server <b> \(gekozen provider\)/);
  assert.doesNotMatch(custom, /Op deze computer|offline/);
  assert.match(processingLabel([{ label: "Audio", destination: custom }]), /^Audio → /);
});

test("retained recording failures keep recovery separate from the new-recording form", () => {
  const retained = { busy: false, phase: "error", id: "Meeting", session_id: "session", error: "Bewaren mislukt.", can_retry_persistence: true };
  assert.equal(retainedMeetingFailure(retained), true);
  assert.equal(retainedMeetingFailure({ ...retained, error: null }), true);
  assert.equal(retainedMeetingFailure({ ...retained, busy: true }), false);
  assert.equal(retainedMeetingFailure({ ...retained, id: null }), false);
  assert.equal(retainedMeetingFailure({ ...retained, session_id: null }), false);
  assert.equal(retainedMeetingFailure({ ...retained, phase: "idle", error: null, can_retry_persistence: false }), false);
});
