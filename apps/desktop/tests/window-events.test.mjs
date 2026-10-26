import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import test from "node:test";
import { deliveredRouteApplies, workspaceRoute } from "../src/workspace-state.ts";

const read = (dir, extension) => readdirSync(new URL(dir, import.meta.url), { recursive: true })
  .filter((name) => name.endsWith(extension))
  .map((name) => readFileSync(new URL(`${dir}/${name}`, import.meta.url), "utf8"))
  .join("\n");

test("a delivered route only navigates the window that shows it", () => {
  assert.equal(deliveredRouteApplies("workspace", workspaceRoute("dictation")), true);
  assert.equal(deliveredRouteApplies("workspace", workspaceRoute("dictation", "pad")), true);
  assert.equal(deliveredRouteApplies("workspace", workspaceRoute("meetings")), true);
  assert.equal(deliveredRouteApplies("workspace", workspaceRoute("recording")), true);
  // Asking for another window here would deliver the route again and loop show and focus.
  assert.equal(deliveredRouteApplies("workspace", workspaceRoute("history")), true);
  assert.equal(deliveredRouteApplies("dictation", workspaceRoute("dictation")), false);
  assert.equal(deliveredRouteApplies("onboarding", workspaceRoute("dictation")), false);
  assert.equal(deliveredRouteApplies("settings", workspaceRoute("history")), false);
});

test("events the backend sends to one window are only heard by that window", () => {
  const rust = read("../src-tauri/src", ".rs");
  const targeted = new Set([...rust.matchAll(/emit_to\(\s*[^,]+,\s*"([^"]+)"/g)].map((match) => match[1]));
  for (const [, name] of rust.matchAll(/flush_frontend(?:_request)?s\([^"]*"([^"]+)"/g)) targeted.add(name);
  for (const name of ["workspace-navigate", "navigate", "workspace-visibility-changed", "hide-flush-complete", "dictation-session"]) assert.ok(targeted.has(name), name);
  const frontend = read("../src", ".ts");
  for (const name of targeted) {
    const global = new RegExp(`(?<![A-Za-z])listen(?:<[^>]*>)?\\(\\s*"${name}"`);
    assert.doesNotMatch(frontend, global, `${name} is sent to one window but every window listens to it`);
  }
  // The flush events are listened to in a loop over their names.
  assert.match(frontend, /\["prepare-hide", "hide"\]\] as const\) await listenToWindow</);
});
