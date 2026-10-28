#!/usr/bin/env node
import { spawn, execFile } from "node:child_process";
import { createServer } from "node:http";
import { createHash } from "node:crypto";
import { access, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execute = promisify(execFile);
const directory = dirname(fileURLToPath(import.meta.url));
const runtimeRoot = resolve(directory, "../..");
const options = { source: runtimeRoot, output: join(runtimeRoot, "build/native-performance/candidate"), label: "candidate" };
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index].replace(/^--/, "");
  if (!(key in options) || !process.argv[index + 1]) throw new Error("Usage: run.mjs [--source PATH] [--output PATH] [--label NAME]");
  options[key] = process.argv[index + 1];
}
if (process.platform !== "darwin") throw new Error("The owned WKWebView runner requires macOS.");
options.source = resolve(options.source);
options.output = resolve(options.output);
if (options.output === options.source || !relative(options.source, options.output).startsWith(`..${sep}`) && !relative(options.source, options.output).startsWith(`build${sep}`)) {
  throw new Error("Output must be outside the source checkout or under its ignored build directory.");
}
const appRoot = join(options.source, "apps/desktop");
try { await access(join(appRoot, "node_modules")); }
catch { await symlink(join(runtimeRoot, "apps/desktop/node_modules"), join(appRoot, "node_modules"), "dir"); }
await mkdir(options.output, { recursive: true });
const temporary = await mkdtemp(join(tmpdir(), "pennel-webkit-benchmark-"));
let server;
let child;
let sampling;
try {
  const { build } = await import(pathToFileURL(join(runtimeRoot, "apps/desktop/node_modules/vite/dist/node/index.js")).href);
  const fixture = await readFile(join(options.source, "scripts/fixtures/desktop-platform.js"), "utf8");
  const benchmark = await readFile(join(directory, "benchmark.js"), "utf8");
  const { visualModels, visualCallApps } = await import(pathToFileURL(join(options.source, "scripts/fixtures/model-catalog.mjs")).href);
  const models = visualModels(JSON.parse(await readFile(join(options.source, "core/src/catalog.json"), "utf8")), await readFile(join(options.source, "core/src/catalog.rs"), "utf8"));
  const appIds = visualCallApps(await readFile(join(options.source, "core/src/rules.rs"), "utf8"));
  const frontend = join(options.output, "frontend");
  await build({
    configFile: join(appRoot, "vite.config.ts"), root: appRoot, logLevel: "warn",
    cacheDir: join(temporary, "vite"),
    // This build is only loaded by the owned helper; shipped release builds keep this false.
    define: { "import.meta.env.DEV": "true" },
    build: { outDir: frontend, emptyOutDir: true },
    plugins: [{ name: "owned-native-performance-fixtures", transformIndexHtml: {
      order: "pre",
      handler() {
        const settings = JSON.stringify({ locale: "en", appearance: "light", window: "workspace", configured_text_provider: true, speakers: "ready", local_models: models, recording_app_ids: appIds });
        const setup = `const scenario = new URL(location.href).searchParams.get('scenario') || 'library'; globalThis.__PENNEL_FIXTURE_OPTIONS__ = { ...${settings}, name: 'performance-' + scenario, section: scenario === 'recording' ? 'recording' : 'meetings' };`;
        return [{ tag: "script", children: `(() => { ${setup} })();\n${fixture}\n${benchmark}`, injectTo: "head-prepend" }];
      },
    } }],
  });
  server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://127.0.0.1");
      const file = resolve(frontend, `.${decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname)}`);
      if (!file.startsWith(frontend + sep)) { response.writeHead(403).end(); return; }
      const contentType = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".ttf": "font/ttf", ".woff2": "font/woff2" }[extname(file)] || "application/octet-stream";
      response.writeHead(200, { "Content-Type": contentType, "Cache-Control": "no-store" });
      response.end(await readFile(file));
    } catch { response.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const bundle = join(options.output, "PennelWebKitBenchmark.app");
  const executable = join(bundle, "Contents/MacOS/PennelWebKitBenchmark");
  await mkdir(dirname(executable), { recursive: true });
  await execute("xcrun", ["swiftc", "-O", "-swift-version", "5", join(directory, "Runner.swift"), "-o", executable], { maxBuffer: 1024 * 1024 });
  await writeFile(join(bundle, "Contents/Info.plist"), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.pennel.webkit-benchmark</string>
<key>CFBundleExecutable</key><string>PennelWebKitBenchmark</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSUIElement</key><true/>
<key>NSAppTransportSecurity</key><dict><key>NSAllowsLocalNetworking</key><true/></dict>
</dict></plist>`);
  await execute("codesign", ["--force", "--sign", "-", bundle]);
  const packageHash = (await execute("shasum", ["-a", "256", executable])).stdout.trim().split(/\s/)[0];
  const frontendHash = createHash("sha256");
  async function hashDirectory(path) {
    for (const entry of (await readdir(path, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) {
      const file = join(path, entry.name);
      if (entry.isDirectory()) await hashDirectory(file);
      else { frontendHash.update(relative(frontend, file)); frontendHash.update(await readFile(file)); }
    }
  }
  await hashDirectory(frontend);
  let revision;
  try { revision = (await execute("git", ["-C", options.source, "rev-parse", "HEAD"])).stdout.trim(); }
  catch { revision = options.label; }
  const receipt = { label: options.label, source: options.source, source_revision: revision, recorded_at: new Date().toISOString(),
    evidence_kind: "Optimized frontend in owned packaged WKWebView helper with synthetic platform responses; not the Tauri product package",
    hardware: { model: (await execute("sysctl", ["-n", "hw.model"])).stdout.trim(), memory_bytes: Number((await execute("sysctl", ["-n", "hw.memsize"])).stdout.trim()) },
    helper_sha256: packageHash, frontend_sha256: frontendHash.digest("hex"), fixture: { meetings: 1000, transcript_segments: 2400, transcript_duration_seconds: 7200 },
    limitations: ["No Rust commands, audio/model/provider processing, native Tauri menus or actual keyboard/trackpad events are exercised.",
      "Fixture access is enabled only in this temporary optimized frontend build.",
      "CPU/RSS samples cover only the owned host PID. WebContent/GPU services cannot be reliably attributed here and are excluded.",
      "Frame gaps use requestAnimationFrame during scripted scrolling; they are not Instruments frame-present measurements.",
      "Fixed optional-command delays reproduce independent-loading behavior and are synthetic, not measured backend latency."], scenarios: [] };
  for (const scenario of ["library", "recording"]) {
    const outputFile = join(options.output, `${scenario}.json`);
    await rm(outputFile, { force: true });
    const samples = [];
    const section = scenario === "recording" ? "recording" : "meetings";
    const url = `${base}/index.html?scenario=${scenario}&window=workspace&section=${section}&meeting_id=${encodeURIComponent("2026-10-24 0900 Projectoverleg")}`;
    child = spawn(executable, [url, outputFile], { stdio: ["ignore", "pipe", "pipe"] });
    let logs = "";
    let phase = "active";
    child.stdout.on("data", data => {
      logs += data;
      const phases = String(data).matchAll(/PENNEL_BENCHMARK_PHASE ([a-z-]+)/g);
      for (const match of phases) phase = match[1];
    });
    child.stderr.on("data", data => { logs += data; });
    const sampledPid = child.pid;
    sampling = setInterval(async () => {
      try {
        const rss = Number((await execute("ps", ["-p", String(sampledPid), "-o", "rss="])).stdout.trim());
        if (Number.isFinite(rss)) samples.push({ phase, rss_kib: rss });
      } catch {}
    }, 500);
    const code = await new Promise(resolve => child.once("exit", resolve));
    clearInterval(sampling); sampling = null;
    await writeFile(join(options.output, `${scenario}.log`), logs);
    if (code !== 0) throw new Error(`WKWebView helper failed: ${scenario}, exit ${code}`);
    const result = JSON.parse(await readFile(outputFile, "utf8"));
    const resourceSummary = observations => ({ count: observations.length,
      peak_rss_kib: observations.length ? Math.max(...observations.map(sample => sample.rss_kib)) : null });
    result.host_process_samples = { ...resourceSummary(samples),
      idle: resourceSummary(samples.filter(sample => sample.phase.endsWith("-idle"))),
      excludes_webcontent_and_gpu: true };
    await writeFile(outputFile, JSON.stringify(result, null, 2) + "\n");
    receipt.scenarios.push({ scenario, file: `${scenario}.json`, passed: !result.error && !result.errors?.length && !result.unknown_commands?.length });
    console.log(JSON.stringify({ scenario, error: result.error ?? null, measurements: result.measurements ?? null }));
  }
  await writeFile(join(options.output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
  if (receipt.scenarios.some(scenario => !scenario.passed)) process.exitCode = 1;
} finally {
  if (sampling) clearInterval(sampling);
  if (child && child.exitCode === null) child.kill("SIGTERM");
  if (server) await new Promise(resolve => server.close(resolve));
  await rm(temporary, { recursive: true, force: true });
}
