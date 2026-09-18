#!/usr/bin/env node
// Source-checkout convenience wrapper. Portable bundles use their native binary directly.
import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = fileURLToPath(new URL('../..', import.meta.url));
const binary = path.join(root, 'target', 'release', `stockfly-server${process.platform === 'win32' ? '.exe' : ''}`);
const args = ['fetch-models', '--root', root, ...process.argv.slice(2)];
const result = existsSync(binary)
  ? spawnSync(binary, args, { stdio: 'inherit' })
  : spawnSync('cargo', ['run', '--release', '-p', 'stockfly-server', '--', ...args], { cwd: root, stdio: 'inherit' });
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
