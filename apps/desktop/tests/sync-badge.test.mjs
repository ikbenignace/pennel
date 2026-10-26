import { setInterfaceLanguage } from "../src/i18n.ts";
setInterfaceLanguage("nl");
import assert from "node:assert/strict";
import test from "node:test";
import { badgeFor, resolveOpenMeeting, syncActive } from "../src/sync-badge.ts";

test("sync off shows no badge whatever the state", () => {
  for (const state of ["synced", "pending", "conflict", undefined]) assert.equal(badgeFor(state, false), null);
});

test("each sync state has its own label", () => {
  assert.equal(badgeFor("synced", true).label, "Gesynchroniseerd");
  assert.equal(badgeFor("pending", true).label, "Wacht op sync");
  assert.equal(badgeFor("conflict", true).label, "Conflictversie");
  const icons = new Set(["synced", "pending", "conflict"].map((state) => badgeFor(state, true).icon));
  assert.equal(icons.size, 3);
});

test("a meeting without a known state shows no badge", () => {
  assert.equal(badgeFor(undefined, true), null);
});

const meetings = [
  { id: "2026-10-25 0930 Weekly", title: "Weekly", start_iso: "2026-10-25T09:30:00" },
  { id: "2026-10-24 0900 Plan", title: "Plan", start_iso: "2026-10-24T09:00:00" },
];

test("an open meeting that still exists keeps its id", () => {
  assert.equal(resolveOpenMeeting("2026-10-24 0900 Plan", meetings, null), "2026-10-24 0900 Plan");
});

test("a renamed meeting is found again by its start time", () => {
  const renamed = [{ id: "2026-10-25 0930 Nieuw", title: "Nieuw", start_iso: "2026-10-25T09:30:00" }, meetings[1]];
  const remembered = { title: "Weekly", start_iso: "2026-10-25T09:30:00" };
  assert.equal(resolveOpenMeeting("2026-10-25 0930 Weekly", renamed, remembered), "2026-10-25 0930 Nieuw");
});

test("meetings that share a start time are told apart by title", () => {
  const same = [
    { id: "a", title: "Een", start_iso: "2026-10-25T09:30:00" },
    { id: "b", title: "Twee", start_iso: "2026-10-25T09:30:00" },
  ];
  assert.equal(resolveOpenMeeting("gone", same, { title: "Twee", start_iso: "2026-10-25T09:30:00" }), "b");
  assert.equal(resolveOpenMeeting("gone", same, { title: "Drie", start_iso: "2026-10-25T09:30:00" }), undefined);
});

test("a vanished meeting without a match falls back to the list", () => {
  assert.equal(resolveOpenMeeting("gone", meetings, null), undefined);
  assert.equal(resolveOpenMeeting("gone", meetings, { title: "X", start_iso: "2020-01-01T00:00:00" }), undefined);
});

test("sync counts as active only while the setting is on and the report is not off or signed out", () => {
  assert.equal(syncActive(true, "synced"), true);
  assert.equal(syncActive(true, "needs_recovery_code"), true);
  assert.equal(syncActive(true, "signed_out"), false);
  assert.equal(syncActive(true, "off"), false);
  assert.equal(syncActive(false, "synced"), false);
  assert.equal(syncActive(true, undefined), true);
});
