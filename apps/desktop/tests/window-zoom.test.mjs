import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const config = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
const backend = readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');

test('workspace and dynamically created windows do not enable webview zoom shortcuts', () => {
  assert.notEqual(config.app.windows.find((window) => window.label === 'main').zoomHotkeysEnabled, true);
  assert.doesNotMatch(backend, /\.zoom_hotkeys_enabled\(true\)/);
});
