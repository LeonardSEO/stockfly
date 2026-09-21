import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readdir, rm, writeFile, readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  CdpSession,
  EXPECTED_NO_MODEL_ROUTES,
  checkRoute,
  isExpectedNoModelHttpError,
  isExpectedNoModelLog,
  parseArgs,
  resolveArchive,
} from './smoke-portable.mjs';

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => child.kill('SIGKILL'), 5_000);
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', chunk => { stdout += chunk; });
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.once('error', reject);
    child.once('exit', code => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
  });
}

class FakeSocket {
  constructor(mode) {
    this.mode = mode;
    this.readyState = 0;
    queueMicrotask(() => {
      if (this.mode === 'never-open') return;
      this.readyState = 1;
      this.onopen?.();
    });
  }

  send() {
    if (this.mode === 'close-on-send') queueMicrotask(() => this.finishClose());
  }

  close() {
    if (this.mode !== 'never-close') queueMicrotask(() => this.finishClose());
  }

  finishClose() {
    this.readyState = 3;
    this.onclose?.();
  }
}

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

test('only exact expected no-model network logs are classified as no-model evidence', () => {
  assert.equal(isExpectedNoModelLog({
    source: 'network', level: 'error', text: 'Failed to load resource: 404',
    url: 'http://127.0.0.1:5187/vendor/models/catalog.json',
  }, new Set(), 'http://127.0.0.1:5187'), true);
  assert.equal(isExpectedNoModelLog({
    source: 'network', level: 'error', text: 'Failed to load resource: 404',
    url: 'http://127.0.0.1:5187/missing.js',
  }, new Set(), 'http://127.0.0.1:5187'), false);
  assert.equal(isExpectedNoModelLog({
    source: 'network', level: 'error', text: 'Failed to load resource: 404', networkRequestId: 'catalog-request',
  }, new Set(['catalog-request'])), true);
  assert.equal(isExpectedNoModelLog({ source: 'javascript', level: 'error', text: 'Uncaught Error' }), false);
  assert.equal(isExpectedNoModelLog({ source: 'network', level: 'error', text: 'Failed to load resource: 500' }), false);
});

test('normal no-model browser fixture allows only its exact same-origin 404 routes', async () => {
  const fixture = JSON.parse(await readFile(path.join(repository, 'tests/fixtures/portable-no-model-browser.json'), 'utf8'));
  assert.deepEqual([...EXPECTED_NO_MODEL_ROUTES].sort(), [...fixture.expectedMissingRoutes].sort());
  for (const route of fixture.expectedMissingRoutes) {
    assert.equal(isExpectedNoModelHttpError({ status: 404, url: `http://127.0.0.1:5187${route}` }, 'http://127.0.0.1:5187'), true);
  }
  for (const route of fixture.unexpectedMissingRoutes) {
    assert.equal(isExpectedNoModelHttpError({ status: 404, url: `http://127.0.0.1:5187${route}` }, 'http://127.0.0.1:5187'), false);
  }
  assert.equal(isExpectedNoModelHttpError({ status: 500, url: 'http://127.0.0.1:5187/vendor/graph/manifest.json' }, 'http://127.0.0.1:5187'), false);
  assert.equal(isExpectedNoModelHttpError({ status: 404, url: 'http://example.test/vendor/graph/manifest.json' }, 'http://127.0.0.1:5187'), false);

  const packager = await readFile(path.join(repository, 'scripts/package-local.py'), 'utf8');
  assert.match(packager, /copy2\(ROOT \/ 'apps\/web\/public\/favicon\.svg', app \/ 'public\/favicon\.svg'\)/);
});

test('a stalled or disconnected CDP peer rejects commands and close within the shared deadline', async () => {
  const neverOpens = new CdpSession('ws://never-opens', () => {}, Date.now() + 60, () => new FakeSocket('never-open'));
  await assert.rejects(() => neverOpens.connect(), /Chromium DevTools websocket connection timed out|Timed out waiting for Chromium DevTools websocket connection/);

  const silent = new CdpSession('ws://silent', () => {}, Date.now() + 60, () => new FakeSocket('silent'));
  await silent.connect();
  await assert.rejects(() => silent.send('Runtime.evaluate'), /Timed out waiting for Chromium DevTools command Runtime\.evaluate/);

  const disconnected = new CdpSession('ws://closing', () => {}, Date.now() + 250, () => new FakeSocket('close-on-send'));
  await disconnected.connect();
  await assert.rejects(() => disconnected.send('Page.enable'), /websocket closed/);

  const neverCloses = new CdpSession('ws://never-closes', () => {}, Date.now() + 60, () => new FakeSocket('never-close'));
  await neverCloses.connect();
  await assert.rejects(() => neverCloses.close(), /Timed out waiting for Chromium DevTools websocket close/);
});

test('argument failures still write atomic versioned evidence when an evidence path is known', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stockfly-argument-evidence-'));
  try {
    const evidence = path.join(root, 'evidence.json');
    const result = await run(process.execPath, [
      path.join(repository, 'scripts/smoke-portable.mjs'),
      '--archive', 'unused.zip',
      '--evidence', evidence,
      '--timeout-ms', 'invalid',
    ], { cwd: repository });
    assert.equal(result.code, 1, result.stdout + result.stderr);
    const report = JSON.parse(await readFile(evidence, 'utf8'));
    assert.equal(report.formatVersion, 1);
    assert.equal(report.pass, false);
    assert.match(report.error, /--timeout-ms/);
    assert.deepEqual((await readdir(root)).filter(name => name.endsWith('.tmp')), []);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('HTTP route checks cannot wait beyond their deadline', async () => {
  const server = createServer(() => {});
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const { port } = server.address();
    await assert.rejects(
      () => checkRoute(`http://127.0.0.1:${port}`, '/stalled', 200, () => {}, Date.now() + 60),
      /Timed out waiting for \/stalled response/,
    );
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
});

test('server spawn ENOENT is captured as atomic versioned CLI evidence', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'stockfly-spawn-evidence-'));
  try {
    const fixtureRoot = path.join(root, 'fixture');
    const bundle = path.join(fixtureRoot, 'stockfly');
    await mkdir(bundle, { recursive: true });
    const executable = path.join(bundle, process.platform === 'win32' ? 'stockfly-server.exe' : 'stockfly-server');
    await writeFile(executable, process.platform === 'win32' ? 'not a Windows executable' : '#!/definitely/missing/interpreter\n');
    const archive = path.join(root, 'runtime.zip');
    const python = process.platform === 'win32' ? 'python' : 'python3';
    const zipped = await run(python, ['-m', 'zipfile', '-c', archive, 'stockfly'], { cwd: fixtureRoot });
    assert.equal(zipped.code, 0, zipped.stderr);
    const evidence = path.join(root, 'evidence.json');
    const result = await run(process.execPath, [
      path.join(repository, 'scripts/smoke-portable.mjs'),
      '--archive', archive,
      '--evidence', evidence,
      '--timeout-ms', '1000',
    ], { cwd: repository });
    assert.equal(result.code, 1, result.stdout + result.stderr);
    const report = JSON.parse(await readFile(evidence, 'utf8'));
    assert.equal(report.formatVersion, 1);
    assert.equal(report.pass, false);
    assert.match(report.error, /ENOENT|spawn|not a valid Win32 application|Unknown error/i);
    assert.deepEqual((await readdir(root)).filter(name => name.endsWith('.tmp')), []);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('release workflow runs Windows smoke and uploads its evidence with the portable ZIP', async () => {
  const workflow = await readFile(path.join(repository, '.github/workflows/release.yml'), 'utf8');
  assert.match(workflow, /if: runner\.os == 'Windows'[\s\S]*node scripts\/smoke-portable\.mjs/);
  assert.match(workflow, /dist\/releases\/\*\*\/\$\{\{ matrix\.name \}\}\.zip/);
  assert.match(workflow, /dist\/releases\/\$\{\{ matrix\.name \}\}-smoke\.json/);
  assert.match(workflow, /if: always\(\)/);
});
