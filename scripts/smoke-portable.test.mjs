import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile, readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  checkRoute,
  isExpectedNoModelLog,
  parseArgs,
  resolveArchive,
} from './smoke-portable.mjs';

test('portable smoke options require archive and machine-readable evidence paths', () => {
  assert.deepEqual(parseArgs(['--archive', 'dist/releases', '--evidence', 'smoke.json']), {
    archive: 'dist/releases',
    evidence: 'smoke.json',
    timeoutMs: 45_000,
  });
  assert.throws(() => parseArgs(['--archive', 'runtime.zip']), /--evidence is required/);
  assert.throws(() => parseArgs(['--wat']), /Unknown or incomplete option/);
});

test('archive directory resolution rejects ambiguous workflow outputs', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stockfly-smoke-test-'));
  try {
    await mkdir(path.join(root, 'nested'));
    await writeFile(path.join(root, 'nested', 'stockfly-windows-x64.zip'), 'zip');
    assert.equal(await resolveArchive(root), path.join(root, 'nested', 'stockfly-windows-x64.zip'));
    await writeFile(path.join(root, 'other.zip'), 'zip');
    await assert.rejects(() => resolveArchive(root), /Expected exactly one ZIP/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('route check records stable machine-readable response evidence', async () => {
  const server = createServer((request, response) => {
    response.writeHead(200, {
      'content-type': 'application/json',
      'cross-origin-opener-policy': 'same-origin',
      'cross-origin-embedder-policy': 'require-corp',
    });
    response.end('{"status":"ok"}');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const { port } = server.address();
    const result = await checkRoute(`http://127.0.0.1:${port}`, '/health', 200, body => {
      assert.equal(JSON.parse(body).status, 'ok');
    });
    assert.equal(result.route, '/health');
    assert.equal(result.status, 200);
    assert.equal(result.headers['content-type'], 'application/json');
    assert.equal(result.bytes, 15);
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
});

test('only the expected absent-catalog network log is classified as no-model evidence', () => {
  assert.equal(isExpectedNoModelLog({
    source: 'network', level: 'error', text: 'Failed to load resource: 404',
    url: 'http://127.0.0.1:8765/vendor/models/catalog.json',
  }), true);
  assert.equal(isExpectedNoModelLog({
    source: 'network', level: 'error', text: 'Failed to load resource: 404',
    url: 'http://127.0.0.1:8765/missing.js',
  }), false);
  assert.equal(isExpectedNoModelLog({
    source: 'network', level: 'error', text: 'Failed to load resource: 404', networkRequestId: 'catalog-request',
  }, new Set(['catalog-request'])), true);
  assert.equal(isExpectedNoModelLog({ source: 'javascript', level: 'error', text: 'Uncaught Error' }), false);
  assert.equal(isExpectedNoModelLog({ source: 'network', level: 'error', text: 'Failed to load resource: 500' }), false);
});

test('release workflow runs Windows smoke and uploads its evidence with the portable ZIP', async () => {
  const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const workflow = await readFile(path.join(repository, '.github/workflows/release.yml'), 'utf8');
  assert.match(workflow, /if: runner\.os == 'Windows'[\s\S]*node scripts\/smoke-portable\.mjs/);
  assert.match(workflow, /dist\/releases\/\*\*\/\$\{\{ matrix\.name \}\}\.zip/);
  assert.match(workflow, /dist\/releases\/\$\{\{ matrix\.name \}\}-smoke\.json/);
  assert.match(workflow, /if: always\(\)/);
});
