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
async function fixture(t, change = () => {}) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'stockfly-publish-schema-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const input = path.join(root, 'input');
  const write = async (name, value) => {
    const target = path.join(input, name);
    await mkdir(path.dirname(target), { recursive: true });
    const bytes = typeof value === 'string' ? value : JSON.stringify(value);
    await writeFile(target, bytes);
    return sha(bytes);
  };
  for (const directory of ['browser', 'browser-models/checkpoints']) await mkdir(path.join(input, 'data', directory), { recursive: true });
  const graphHash = await write('data/compiled/malecns-v1/neurons.bin', 'graph fixture');
  const sensory = await write('data/chess-maps/sensory-map.json', 'sensory fixture');
  const output = await write('data/chess-maps/output-map.json', 'output fixture');
  const reports = {};
  for (const name of ['standard-causal-audit', 'standard-playing-strength', 'fullgraph-parity', 'standard-training-sweep', 'windows-runtime']) {
    reports[name] = JSON.parse(await readFile(new URL(`../../docs/results/2026-09-19-${name}.json`, import.meta.url), 'utf8'));
  }
  const causal = reports['standard-causal-audit'];
  const ladder = reports['standard-playing-strength'];
  const parity = reports['fullgraph-parity'];
  const training = reports['standard-training-sweep'];
  const models = [];
  for (const kind of ['bio-full', 'max-full']) {
    const filename = `${kind}.sfckpt`;
    await write(`data/browser-models/checkpoints/${filename}`, kind);
    const candidate = training.candidates.find(item => item.kind === kind && item.seed === (kind === 'bio-full' ? 45 : 43));
    candidate.sha256 = sha(kind);
    models.push({ id: kind, availability: 'available', expectedKind: kind, checkpointUrl: `/vendor/models/checkpoints/${filename}`, checkpointSha256: sha(kind), graphNeuronsSha256: graphHash, sensoryMapSha256: sensory, outputMapSha256: output, trainingPreset: 'standard', trialsRun: candidate.trialsRun });
    const audit = causal.reports.find(report => report.model_kind === kind);
    const match = ladder.models[kind === 'bio-full' ? 'bio' : 'max'];
    const numerical = parity.final.models.find(report => report.model_kind === kind);
    for (const identity of [audit.inputs, match.identity.model.hashes, numerical.identity]) {
      Object.assign(identity, { model_sha256: sha(kind), graph_files: { 'neurons.bin': graphHash }, sensory_map_sha256: sensory, output_map_sha256: output });
    }
    audit.raw_report_sha256 = await write(`data/release-evidence/causal/${path.basename(audit.raw_report)}`, `${kind} causal`);
    match.raw_report_sha256 = await write(`data/release-evidence/ladder/${path.basename(match.raw_report)}`, `${kind} ladder`);
  }
  for (const report of causal.superseded_evidence.reports) report.raw_report_sha256 = await write(`data/release-evidence/causal-superseded/${path.basename(report.raw_report)}`, report.model_kind);
  causal.superseded_evidence.tracked_summary_sha256 = await write('data/release-evidence/causal/superseded-summary.json', 'superseded fixture');
  for (const phase of ['baseline', 'diagnostic', 'final']) parity[phase].raw_report.sha256 = await write(`data/release-evidence/parity/${path.basename(parity[phase].raw_report.path)}`, phase);
  await change({ models, reports, input });
  for (const [name, report] of Object.entries(reports)) await write(`data/release-evidence/2026-09-19-${name}.json`, report);
  await write('data/browser-models/catalog.json', { models });
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
  assert.deepEqual(manifest.models.map(model => model.selectionSeed), [45, 43]);
  assert.equal(manifest.evidence.playingStrength.totals.losses, 200);
  assert.equal(manifest.evidence.playingStrength.finitePointElo, false);
  assert.equal(manifest.evidence.nativeParity.cases, 24);
  assert.equal(manifest.evidence.windowsRuntime.status, 'passed');
  for (const [change, expected] of [
    [value => { delete value.attribution; }, /required attribution/],
    [value => { value.files = []; }, /minItems/],
    [value => { value.releaseStatus = 'validated'; }, /const/],
    [value => { value.files[0].sha256 = 'bad'; }, /pattern/],
    [value => { value.files[0].modelKind = 'lite'; }, /enum/],
    [value => { value.files[0].size = -1; }, /minimum/],
    [value => { value.files[0].size = 0.5; }, /type integer/],
    [value => { value.files[0].trainingPreset = ''; }, /minLength/],
    [value => { value.models.push(value.models[0]); }, /maxItems/],
    [value => { delete value.evidence; }, /required evidence/],
    [value => { value.evidence.playingStrength.finitePointElo = true; }, /const/],
  ]) {
    const invalid = structuredClone(manifest); change(invalid);
    assert.throws(() => validateReleaseManifest(invalid, schema), expected);
  }
  const drifted = structuredClone(schema);
  drifted.properties.files.items.properties.sha256.maxLength = 64;
  assert.throws(() => validateReleaseManifest(manifest, drifted), /Unsupported release schema keyword: maxLength/);
});
test('publisher rejects schema-invalid catalog metadata before writing or publishing a manifest', async t => {
  const { result, out } = await fixture(t, ({ models }) => { models[0].trainingPreset = 42; });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Missing matching Standard evidence/);
  await assert.rejects(readFile(path.join(out, 'release-manifest.json')), { code: 'ENOENT' });
});

for (const [name, change, expected] of [
  ['checkpoint from different evidence', ({ reports }) => { reports['standard-causal-audit'].reports[0].inputs.model_sha256 = '0'.repeat(64); }, /Evidence checkpoint mismatch/],
  ['changed raw report digest', ({ reports }) => { reports['standard-causal-audit'].reports[0].raw_report_sha256 = '0'.repeat(64); }, /Evidence hash mismatch/],
  ['failed Windows smoke', ({ reports }) => { reports['windows-runtime'].pass = false; }, /passing Windows/],
  ['extra catalog identity', ({ models }) => { models.push({ id: 'lite', availability: 'unavailable' }); }, /Expected exactly/],
]) {
  test(`publisher rejects ${name}`, async t => {
    const { result, out } = await fixture(t, change);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, expected);
    await assert.rejects(readFile(path.join(out, 'release-manifest.json')), { code: 'ENOENT' });
  });
}
