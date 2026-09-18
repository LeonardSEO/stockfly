import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, readdir, rm, symlink, stat } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
const binary = process.env.STOCKFLY_SERVER ?? path.resolve(`target/debug/stockfly-server${process.platform === 'win32' ? '.exe' : ''}`);
const digest = data => createHash('sha256').update(data).digest('hex');
const entry = (name, data) => ({ path: `data/checkpoints/${name}`, asset: name, size: Buffer.byteLength(data), sha256: digest(data), modelKind: 'bio-full', graphNeuronsSha256: 'a'.repeat(64), trainingPreset: 'quick' });
const manifest = files => ({ formatVersion: 1, tag: 'fixture', releaseStatus: 'experimental', causalAuditStatus: 'failed', liteStatus: 'blocked', attribution: { maleCns: 'MaleCNS CC-BY', stockfish: 'Stockfish GPLv3' }, files });
async function fixture(t, doc, bodies) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'stockfly-install-'));
  const requests = [];
  const server = http.createServer((request, response) => {
    requests.push(request.url);
    if (request.url === '/release-manifest.json') response.end(JSON.stringify(doc));
    else if (bodies[request.url.slice(1)] !== undefined) response.end(bodies[request.url.slice(1)]);
    else { response.statusCode = 404; response.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => { await new Promise(resolve => server.close(resolve)); await rm(root, { recursive: true, force: true }); });
  const run = () => new Promise((resolve, reject) => {
    const child = spawn(binary, ['fetch-models', '--root', root, '--tag', 'fixture', '--fixture-base', `http://127.0.0.1:${server.address().port}`]);
    let output = ''; child.stdout.on('data', data => output += data); child.stderr.on('data', data => output += data);
    child.on('error', reject); child.on('close', code => resolve({ code, output }));
  });
  return { root, requests, run };
}
test('installs verified files, reports size, and preserves matching files without redownload', async t => {
  const f = await fixture(t, manifest([entry('bio.sfckpt', 'correct')]), { 'bio.sfckpt': 'correct' });
  assert.equal((await f.run()).code, 0);
  const destination = path.join(f.root, 'data/checkpoints/bio.sfckpt');
  const before = await stat(destination);
  const result = await f.run();
  assert.equal(result.code, 0, result.output);
  assert.match(result.output, /7 bytes/);
  assert.equal(await readFile(destination, 'utf8'), 'correct');
  assert.equal((await stat(destination)).mtimeMs, before.mtimeMs);
  assert.equal(f.requests.filter(url => url === '/bio.sfckpt').length, 1);
});
for (const corruption of ['corrupt', 'correct but too large']) {
  test(`rejects ${corruption}; all prior files and catalog survive, staging is removed`, async t => {
    const files = [entry('first.sfckpt', 'new'), entry('second.sfckpt', 'correct')];
    const f = await fixture(t, manifest(files), { 'first.sfckpt': 'new', 'second.sfckpt': corruption });
    await mkdir(path.join(f.root, 'data/checkpoints'), { recursive: true });
    await writeFile(path.join(f.root, files[0].path), 'old first');
    await writeFile(path.join(f.root, files[1].path), 'old second');
    const result = await f.run();
    assert.notEqual(result.code, 0);
    assert.match(result.output, /mismatch/);
    assert.equal(await readFile(path.join(f.root, files[0].path), 'utf8'), 'old first');
    assert.equal(await readFile(path.join(f.root, files[1].path), 'utf8'), 'old second');
    assert.deepEqual(await readdir(f.root), ['data']);
  });
}
for (const unsafe of ['data/checkpoints/../../outside', '/tmp/file', 'data/raw/private.csv', 'data/checkpoints/..\\outside', 'data/checkpoints/C:stream', 'data/checkpoints/%2e%2e/file']) {
  test(`rejects unsafe manifest destination ${unsafe} before any asset request`, async t => {
    const file = { ...entry('bio.sfckpt', 'correct'), path: unsafe };
    const f = await fixture(t, manifest([file]), { 'bio.sfckpt': 'correct' });
    assert.notEqual((await f.run()).code, 0);
    assert.deepEqual(f.requests, ['/release-manifest.json']);
  });
}
test('rejects destination symlink without modifying its target', { skip: process.platform === 'win32' }, async t => {
  const f = await fixture(t, manifest([entry('bio.sfckpt', 'correct')]), { 'bio.sfckpt': 'correct' });
  await mkdir(path.join(f.root, 'outside'));
  await mkdir(path.join(f.root, 'data'));
  await symlink(path.join(f.root, 'outside'), path.join(f.root, 'data/checkpoints'));
  assert.notEqual((await f.run()).code, 0);
  assert.deepEqual(await readdir(path.join(f.root, 'outside')), []);
});
test('rejects a passing scientific claim and a mismatched requested tag', async t => {
  for (const extra of [{ causalAuditStatus: 'passed' }, { tag: 'different' }]) {
    const f = await fixture(t, { ...manifest([entry('bio.sfckpt', 'correct')]), ...extra }, { 'bio.sfckpt': 'correct' });
    assert.notEqual((await f.run()).code, 0);
    assert.deepEqual(f.requests, ['/release-manifest.json']);
  }
});
