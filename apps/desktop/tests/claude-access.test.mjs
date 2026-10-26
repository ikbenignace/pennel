import assert from "node:assert/strict";
import test, { beforeEach } from "node:test";
import { setInterfaceLanguage } from "../src/i18n.ts";

beforeEach(() => setInterfaceLanguage("nl"));
import { claudeInstructions, claudeStatusText } from "../src/claude-access.ts";
import { relativeTime } from "../src/cloud-login.ts";

const NOW = 10 * 60 * 60 * 1000;
const ago = (then) => relativeTime(then, NOW);

test("no status line while switched off", () => {
  assert.equal(claudeStatusText({ kind: "off" }, false, ago), null);
  assert.equal(claudeStatusText({ kind: "connected", last_request_ms: null }, false, ago), null);
});

test("switched on shows the connection state", () => {
  assert.equal(claudeStatusText({ kind: "off" }, true, ago), "Verbinden met Pennel Cloud…");
  assert.equal(claudeStatusText({ kind: "connecting" }, true, ago), "Verbinden met Pennel Cloud…");
  assert.equal(claudeStatusText({ kind: "connected", last_request_ms: null }, true, ago), "Verbonden. Claude kan je meetings op deze computer doorzoeken.");
  assert.equal(claudeStatusText({ kind: "connected", last_request_ms: NOW - 5 * 60000 }, true, ago), "Verbonden · laatste vraag van Claude 5 min geleden");
  assert.equal(claudeStatusText({ kind: "error", message: "network error: x" }, true, ago), "Geen verbinding met Pennel Cloud. Pennel probeert het zo opnieuw.");
});

test("instructions use the server's MCP URL", () => {
  assert.deepEqual(claudeInstructions("https://api.pennel.app/mcp"), {
    connectorUrl: "https://api.pennel.app/mcp",
    command: "claude mcp add --transport http pennel https://api.pennel.app/mcp",
  });
});


test("Claude status is localized without changing connector instructions", () => {
  const url = "https://example.invalid/mcp";
  const instructions = claudeInstructions(url);
  setInterfaceLanguage("en");
  assert.equal(claudeStatusText({ kind: "connecting" }, true, ago), "Connecting to Pennel Cloud…");
  assert.equal(claudeStatusText({ kind: "connected", last_request_ms: NOW - 120000 }, true, ago), "Connected · last question from Claude 2 min ago");
  setInterfaceLanguage("nl");
  assert.deepEqual(claudeInstructions(url), instructions);
});
