#!/usr/bin/env node
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const args = process.argv.slice(2);
const options = { output: join(root, "build/visual-parity"), native: join(root, "build/visual-parity/native"), desktop: join(root, "build/visual-parity/desktop"), requireAll: false };
for (let index = 0; index < args.length; index++) {
  if (args[index] === "--output") options.output = resolve(args[++index]);
  else if (args[index] === "--native") options.native = resolve(args[++index]);
  else if (args[index] === "--desktop") options.desktop = resolve(args[++index]);
  else if (args[index] === "--require-all") options.requireAll = true;
  else throw new Error(`Unknown argument: ${args[index]}`);
}
const cases = JSON.parse(await readFile(join(root, "scripts/fixtures/visual-cases.json"), "utf8"));
const native = JSON.parse(await readFile(join(options.native, "fixture.json"), "utf8"));
const desktop = JSON.parse(await readFile(join(options.desktop, "manifest.json"), "utf8"));
const key = image => `${image.appearance}/${image.case}`;
const nativeByKey = new Map(native.images.map(image => [key(image), image]));
const desktopByKey = new Map(desktop.images.map(image => [key(image), image]));
const expected = ["light", "dark"].flatMap(appearance => cases.map(item => `${appearance}/${item.name}`));
const matched = expected.filter(key => nativeByKey.has(key) && desktopByKey.has(key));
const missing = expected.filter(key => !nativeByKey.has(key) || !desktopByKey.has(key));
const escape = value => String(value).replace(/[&<>"']/g, character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character]);
const source = (folder, file) => relative(options.output, join(folder, file)).split(sep).map(encodeURIComponent).join("/");
const mismatches = matched.filter(key => {
  const a = nativeByKey.get(key), b = desktopByKey.get(key);
  return a.logical_width !== b.width || a.logical_height !== b.height;
});
const captureFailures = desktop.images.filter(item => item.horizontal_overflow || item.unknown_commands?.length || item.runtime_errors?.length || item.runtime_banner || item.surface_error).map(item => ({ case: item.case, appearance: item.appearance }));
const incomplete = native.complete === false || desktop.complete === false;
await mkdir(options.output, { recursive: true });
const html = `<!doctype html>
<html lang="nl"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Pennel — visuele vergelijking</title>
<style>body{margin:0;padding:24px;font:14px/1.5 system-ui,sans-serif;background:#f4f4f5;color:#202024}h1{font-size:22px;margin:0 0 6px}p{margin:0 0 24px;color:#5f5f66}section{margin:32px 0}h2{font-size:15px;font-weight:600;margin:0 0 10px}.pair{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:16px}figure{margin:0;min-width:0}figcaption{font-size:12px;margin-bottom:6px;color:#5f5f66}img{display:block;width:100%;height:auto;background:#fff}code{font-size:12px}@media(max-width:700px){body{padding:16px}.pair{grid-template-columns:1fr}}</style></head>
<body><h1>Pennel — visuele vergelijking</h1><p>${matched.length} van ${expected.length} fixtureparen beschikbaar.${incomplete ? " De opnamerun is nog niet voltooid." : ""}${captureFailures.length ? ` ${captureFailures.length} opnames bevatten runtime- of layoutfouten.` : ""} Dit vergelijkt productweergaven met synthetische testdata; het bewijst geen hardware- of pakketacceptatie.</p>
${matched.map(key => {
  const a = nativeByKey.get(key), b = desktopByKey.get(key);
  return `<section><h2>${escape(a.case)} · ${a.appearance === "dark" ? "Donker" : "Licht"}</h2><div class="pair"><figure><figcaption>Native macOS · ${a.logical_width} × ${a.logical_height}</figcaption><a href="${source(options.native, a.file)}"><img src="${source(options.native, a.file)}" alt="Native Pennel: ${escape(a.case)}, ${a.appearance}" loading="lazy"></a></figure><figure><figcaption>Gedeeld · ${b.width} × ${b.height}</figcaption><a href="${source(options.desktop, b.file)}"><img src="${source(options.desktop, b.file)}" alt="Gedeeld Pennel: ${escape(b.case)}, ${b.appearance}" loading="lazy"></a></figure></div></section>`;
}).join("\n")}
${missing.length ? `<section><h2>Nog niet vastgelegd</h2><code>${missing.map(escape).join("<br>")}</code></section>` : ""}
</body></html>\n`;
await writeFile(join(options.output, "index.html"), html);
await writeFile(join(options.output, "comparison.json"), JSON.stringify({ matched, missing, dimension_mismatches: mismatches, capture_failures: captureFailures, incomplete }, null, 2) + "\n");
console.log(JSON.stringify({ comparison: join(options.output, "index.html"), matched: matched.length, expected: expected.length, missing: missing.length, dimension_mismatches: mismatches, capture_failures: captureFailures, incomplete }, null, 2));
process.exitCode = mismatches.length || captureFailures.length || incomplete || (options.requireAll && missing.length) ? 1 : 0;
