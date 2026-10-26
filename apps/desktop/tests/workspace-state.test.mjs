import assert from "node:assert/strict";
import test from "node:test";
import { WorkspaceNavigation, historyEntryId, historyRoute, legacyWorkspaceRoute, parseWorkspaceRoute, routeKey, samePadRoute, windowForSection, windowMode, workspaceRoute } from "../src/workspace-state.ts";

test("Settings belongs to the main workspace; History and Dictation retain their windows", () => {
  assert.equal(windowForSection("settings"), "workspace");
  assert.equal(windowForSection("history"), "history");
  assert.equal(windowForSection("dictation"), "dictation");
  assert.equal(windowForSection("meetings"), "workspace");
  assert.equal(windowForSection("recording"), "workspace");
  assert.equal(windowMode("settings"), "workspace");
  assert.equal(windowMode("library"), "workspace");
  assert.equal(windowMode(null), "workspace");
  assert.deepEqual(legacyWorkspaceRoute("workspace"), workspaceRoute("meetings"));
  assert.deepEqual(legacyWorkspaceRoute("dictation", "pad"), workspaceRoute("dictation", "pad"));
  assert.deepEqual(new WorkspaceNavigation(workspaceRoute("history")).current, workspaceRoute("history"));
});

test("inline Settings returns to the originating task and remembers its last category", () => {
  const meeting = workspaceRoute("meetings", null, "2026-10-24 Projectoverleg");
  const nav = new WorkspaceNavigation(meeting);
  nav.set(workspaceRoute("settings", "models"));
  nav.set(workspaceRoute("settings", "appearance"));
  assert.deepEqual(nav.settingsBack(), meeting);
  nav.set(nav.settingsBack());
  assert.deepEqual(nav.select("settings"), workspaceRoute("settings", "appearance"));
  assert.deepEqual(new WorkspaceNavigation(workspaceRoute("settings", "models")).select("settings"), workspaceRoute("settings", "models"));
});

test("a recording finished during Settings updates the return destination without leaving Settings", () => {
  const settings = workspaceRoute("settings", "overview");
  const nav = new WorkspaceNavigation(workspaceRoute("recording", null, "pending-meeting"));
  nav.set(settings);
  nav.recordingFinished("saved-meeting");
  assert.deepEqual(nav.current, settings);
  assert.deepEqual(nav.settingsBack(), workspaceRoute("meetings", null, "saved-meeting"));
  const normal = new WorkspaceNavigation(workspaceRoute("meetings", null, "another-meeting"));
  normal.set(settings); normal.recordingFinished("saved-meeting");
  assert.equal(normal.settingsBack().meeting_id, "another-meeting");
  assert.deepEqual(new WorkspaceNavigation(settings).settingsBack(), workspaceRoute("meetings"));
});

test("legacy windows and settings aliases now land in one workspace", () => {
  assert.deepEqual(legacyWorkspaceRoute("settings", "meetings"), workspaceRoute("settings", "meeting-settings"));
  assert.deepEqual(legacyWorkspaceRoute("settings", "local"), workspaceRoute("settings", "local"));
  assert.deepEqual(legacyWorkspaceRoute("history"), workspaceRoute("history"));
  assert.deepEqual(legacyWorkspaceRoute("live", "meeting"), workspaceRoute("recording"));
});

test("new Settings opens at task readiness while old advanced links stay valid", () => {
  assert.deepEqual(legacyWorkspaceRoute("settings"), workspaceRoute("settings", "overview"));
  assert.deepEqual(legacyWorkspaceRoute("settings", "models"), workspaceRoute("settings", "models"));
  assert.deepEqual(parseWorkspaceRoute({ section: "settings", tab: "overview" }), workspaceRoute("settings", "overview"));
  assert.deepEqual(new WorkspaceNavigation().select("settings"), workspaceRoute("settings", "overview"));
  assert.equal(parseWorkspaceRoute({ section: "meetings", meeting_id: "overview" }), null);
});

test("recovery opens a specific History result without putting its id in a meeting path", () => {
  const id = "019c6e27-e55b-73d1-87d8-4e01f1f75043";
  const route = workspaceRoute("history", `entry:${id}`);
  assert.deepEqual(historyRoute(id), route);
  assert.equal(historyEntryId(route.tab), id);
  assert.deepEqual(legacyWorkspaceRoute("history", route.tab), route);
  assert.deepEqual(parseWorkspaceRoute(route), route);
  assert.deepEqual(historyRoute(), workspaceRoute("history"));
  for (const tab of ["entry:", "entry:../history", "entry:<script>", "entry:%3Cscript%3E", "entry:result\n", `entry:${"x".repeat(129)}`, "other"]) {
    assert.equal(historyEntryId(tab), null);
    assert.equal(parseWorkspaceRoute({ section: "history", tab }), null);
    assert.deepEqual(legacyWorkspaceRoute("history", tab), workspaceRoute("history"));
  }
});

test("targeted navigation validates meeting identifiers and timestamp jumps", () => {
  assert.deepEqual(parseWorkspaceRoute({ section: "meetings", meeting_id: "2026-10-25 Productoverleg", jump_ms: 1234 }), workspaceRoute("meetings", null, "2026-10-25 Productoverleg", 1234));
  for (const value of ["../meeting", "a/b", "a\\b", "a:b", "a\n", "..", "settings"]) assert.equal(parseWorkspaceRoute({ section: "meetings", meeting_id: value }), null);
  assert.equal(parseWorkspaceRoute({ section: "other" }), null);
  assert.equal(parseWorkspaceRoute({ section: "settings", tab: "unknown" }), null);
});

test("switching primary destinations preserves the selected meeting and settings category", () => {
  const nav = new WorkspaceNavigation();
  nav.set(workspaceRoute("meetings", null, "Productoverleg", 5000));
  nav.set(workspaceRoute("settings", "models"));
  nav.set(workspaceRoute("history"));
  assert.deepEqual(nav.select("meetings"), workspaceRoute("meetings", null, "Productoverleg"));
  assert.deepEqual(nav.select("settings"), workspaceRoute("settings", "models"));
  assert.notEqual(routeKey(nav.select("meetings")), routeKey(nav.current));
});

test("recording compatibility routes do not erase the remembered meetings route", () => {
  const nav = new WorkspaceNavigation();
  nav.set(workspaceRoute("meetings", null, "Projectoverleg"));
  nav.set(parseWorkspaceRoute({ section: "meetings", tab: "recording" }));
  assert.equal(nav.current.section, "recording");
  assert.equal(nav.select("meetings").meeting_id, "Projectoverleg");
});

test("reselecting the active Pad keeps its session while same-route provider selection can remount", () => {
  assert.equal(samePadRoute(workspaceRoute("dictation"), workspaceRoute("dictation")), true);
  assert.equal(samePadRoute(workspaceRoute("settings", "providers"), workspaceRoute("settings", "providers")), false);
  assert.equal(samePadRoute(workspaceRoute("dictation"), workspaceRoute("history")), false);
  assert.equal(samePadRoute(workspaceRoute("history"), workspaceRoute("dictation")), false);
});

test("the optional Pad has its own route and is remembered across workspace navigation", () => {
  const nav = new WorkspaceNavigation();
  const pad = workspaceRoute("dictation", "pad");
  assert.equal(samePadRoute(nav.current, pad), false);
  nav.set(pad);
  nav.set(workspaceRoute("history"));
  assert.deepEqual(nav.select("dictation"), pad);
  assert.equal(samePadRoute(pad, workspaceRoute("dictation")), false);
});
