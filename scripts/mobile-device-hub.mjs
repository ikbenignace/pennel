#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const directory = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(directory, '../apps/mobile');
const artifacts = path.resolve(directory, '../build/mobile/qa/device-hub');
const require = createRequire(path.join(workspace, 'package.json'));
const cli = path.join(path.dirname(require.resolve('expo-device-hub/package.json')), 'dist/server/cli.mjs');
const { values } = parseArgs({ options: { port: { type: 'string', short: 'p', default: '8879' }, help: { type: 'boolean', short: 'h', default: false } } });
const port = Number(values.port);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Device Hub port must be an integer from 1 to 65535.');
if (!values.help) mkdirSync(artifacts, { recursive: true });
const args = values.help ? ['--help'] : ['--host', '127.0.0.1', '--port', String(port), '--platform', 'ios', '--transport', 'mjpeg', '--hide-boot-device'];
const child = spawn(process.execPath, [cli, ...args], {
  cwd: workspace,
  stdio: 'inherit',
  env: { ...process.env, EXPO_DEVICE_HUB_SCREENSHOT_DIRECTORY: artifacts },
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
child.on('error', (error) => { console.error(error.message); process.exitCode = 1; });
child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
