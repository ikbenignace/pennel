import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const native = new URL("../src-tauri/", import.meta.url);
const config = JSON.parse(readFileSync(new URL("tauri.conf.json", native), "utf8"));
const plist = readFileSync(new URL("Info.plist", native), "utf8");
const permissionKeys = ["NSMicrophoneUsageDescription", "NSAudioCaptureUsageDescription", "NSSpeechRecognitionUsageDescription", "NSCalendarsFullAccessUsageDescription"];

function permissionStrings(language) {
  const source = `resources/${language}.lproj/InfoPlist.strings`;
  assert.equal(config.bundle.resources[source], `${language}.lproj/InfoPlist.strings`);
  return Object.fromEntries([...readFileSync(new URL(source, native), "utf8").matchAll(/^"([^"]+)"\s*=\s*"([^"]+)";$/gm)].map(([, key, value]) => [key, value]));
}

test("native permission bundles have English fallback and complete Dutch resources", () => {
  assert.match(plist, /<key>CFBundleDevelopmentRegion<\/key>\s*<string>en<\/string>/);
  const english = permissionStrings("en");
  const dutch = permissionStrings("nl");
  assert.deepEqual(Object.keys(english).sort(), permissionKeys.toSorted());
  assert.deepEqual(Object.keys(dutch).sort(), permissionKeys.toSorted());
  for (const key of permissionKeys) {
    const base = new RegExp(`<key>${key}</key>\\s*<string>([^<]+)</string>`).exec(plist)?.[1];
    assert.equal(english[key], base);
    assert.notEqual(dutch[key], english[key]);
  }
});
