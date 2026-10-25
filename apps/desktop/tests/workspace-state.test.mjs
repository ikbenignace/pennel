import assert from "node:assert/strict";
import test from "node:test";
import { WorkspaceNavigation, legacyWorkspaceRoute, parseWorkspaceRoute, routeKey, samePadRoute, workspaceRoute } from "../src/workspace-state.ts";

test("legacy windows and settings aliases now land in one workspace", () => {
  assert.deepEqual(legacyWorkspaceRoute("settings", "meetings"), workspaceRoute("settings", "meeting-settings"));
  assert.deepEqual(legacyWorkspaceRoute("settings", "local"), workspaceRoute("settings", "local"));
  assert.deepEqual(legacyWorkspaceRoute("history"), workspaceRoute("history"));
  assert.deepEqual(legacyWorkspaceRoute("live", "meeting"), workspaceRoute("recording"));
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
