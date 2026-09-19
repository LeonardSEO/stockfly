import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { validateReleaseManifest } from './validate-release-manifest.mjs';
const schema = JSON.parse(await readFile(new URL('./release-manifest.schema.json', import.meta.url), 'utf8'));
const publisher = new URL('./publish-release.mjs', import.meta.url);
const sha = value => createHash('sha256').update(value).digest('hex');
async function fixture(t, invalidPreset = false) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'stockfly-publish-schema-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const input = path.join(root, 'input');
  for (const directory of ['compiled/malecns-v1', 'browser', 'chess-maps', 'browser-models/checkpoints', 'release-evidence']) await mkdir(path.join(input, 'data', directory), { recursive: true });
  await writeFile(path.join(input, 'data/compiled/malecns-v1/neurons.bin'), 'graph fixture');
  const models = [];
  for (const kind of ['bio-full', 'max-full']) {
    const filename = `${kind}.sfckpt`;
    await writeFile(path.join(input, 'data/browser-models/checkpoints', filename), kind);
    models.push({ id: kind, availability: 'available', expectedKind: kind, checkpointUrl: `/vendor/models/checkpoints/${filename}`, checkpointSha256: sha(kind), graphNeuronsSha256: sha('graph fixture'), trainingPreset: invalidPreset ? 42 : 'quick', trialsRun: 1 });
  }
  await writeFile(path.join(input, 'data/browser-models/catalog.json'), JSON.stringify({ models }));
  const out = path.join(root, 'out');
  const result = spawnSync(process.execPath, [fileURLToPath(publisher), '--tag', 'fixture', '--input', input, '--out', out], { encoding: 'utf8' });
  return { result, out };
}
test('publisher validates a generated fixture manifest against the checked-in schema', async t => {
  const { result, out } = await fixture(t);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(await readFile(path.join(out, 'release-manifest.json'), 'utf8'));
  assert.doesNotThrow(() => validateReleaseManifest(manifest, schema));
  assert.equal(Object.hasOwn(manifest, 'liteStatus'), false);
  for (const [change, expected] of [
    [value => { delete value.attribution; }, /required attribution/],
    [value => { value.files = []; }, /minItems/],
    [value => { value.releaseStatus = 'validated'; }, /const/],
    [value => { value.files[0].sha256 = 'bad'; }, /pattern/],
    [value => { value.files[0].modelKind = 'lite'; }, /enum/],
    [value => { value.files[0].size = -1; }, /minimum/],
    [value => { value.files[0].size = 0.5; }, /type integer/],
    [value => { value.files[0].trainingPreset = ''; }, /minLength/],
  ]) {
    const invalid = structuredClone(manifest); change(invalid);
    assert.throws(() => validateReleaseManifest(invalid, schema), expected);
  }
  const drifted = structuredClone(schema);
  drifted.properties.files.items.properties.sha256.maxLength = 64;
  assert.throws(() => validateReleaseManifest(manifest, drifted), /Unsupported release schema keyword: maxLength/);
});
test('publisher rejects schema-invalid catalog metadata before writing or publishing a manifest', async t => {
  const { result, out } = await fixture(t, true);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /trainingPreset: violates type string/);
  await assert.rejects(readFile(path.join(out, 'release-manifest.json')), { code: 'ENOENT' });
});
