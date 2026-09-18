#!/usr/bin/env node
// Prepare locally by default. Only explicit --publish invokes gh and uploads assets.
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readdir, readFile, copyFile, writeFile, realpath, stat } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { validateReleaseManifest } from './validate-release-manifest.mjs';
const { values } = parseArgs({ options: { tag: { type: 'string' }, input: { type: 'string' }, out: { type: 'string' }, publish: { type: 'boolean', default: false } } });
if (!values.tag || !/^[A-Za-z0-9._-]+$/.test(values.tag) || !values.input || !values.out) throw Error('Usage: publish-release.mjs --tag TAG --input PORTABLE_ROOT --out EMPTY_OUTPUT [--publish]');
const input = await realpath(values.input);
const out = path.resolve(values.out);
if (out === input || out.startsWith(`${input}${path.sep}`)) throw Error('Output must be outside input');
await mkdir(out, { recursive: true });
if ((await readdir(out)).length) throw Error('Output must be empty; never overwrite a release staging directory');
const hash = async file => { const digest = createHash('sha256'); for await (const chunk of createReadStream(file)) digest.update(chunk); return digest.digest('hex'); };
const catalog = JSON.parse(await readFile(path.join(input, 'data/browser-models/catalog.json'), 'utf8'));
const models = catalog.models.filter(model => model.availability === 'available');
if (models.length !== 2 || !['bio-full', 'max-full'].every(kind => models.some(model => model.id === kind))) throw Error('Expected Bio and Max Full experimental checkpoints');
const graphHash = models[0].graphNeuronsSha256;
if (models.some(model => model.graphNeuronsSha256 !== graphHash)) throw Error('Models use different graphs');
const files = [];
async function walk(relative) {
  const directory = path.join(input, relative);
  if (await realpath(directory) !== directory) throw Error(`Symlinked asset directory: ${relative}`);
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const item = `${relative}/${entry.name}`;
    if (entry.isSymbolicLink()) throw Error(`Symlinks are not release assets: ${item}`);
    if (entry.isDirectory()) { await walk(item); continue; }
    if (!entry.isFile()) throw Error(`Not a regular file: ${item}`);
    const source = path.join(input, item);
    const identity = await hash(source);
    const model = models.find(model => item.endsWith(`/${path.basename(model.checkpointUrl)}`));
    if (item.endsWith('.sfckpt') && !model) throw Error(`Uncatalogued checkpoint: ${item}`);
    if (model && identity !== model.checkpointSha256) throw Error(`Checkpoint hash mismatch: ${item}`);
    const asset = `${identity}-${entry.name}`;
    await copyFile(source, path.join(out, asset));
    files.push({ path: item, asset, size: (await stat(source)).size, sha256: identity,
      modelKind: model?.expectedKind ?? 'shared', graphNeuronsSha256: graphHash, trainingPreset: model?.trainingPreset ?? 'not-applicable' });
  }
}
// Explicit allowlist: raw data and unrelated local files cannot enter the release.
for (const directory of ['data/compiled/malecns-v1', 'data/browser', 'data/chess-maps', 'data/browser-models', 'data/release-evidence']) await walk(directory);
if (files.find(file => file.path === 'data/compiled/malecns-v1/neurons.bin')?.sha256 !== graphHash) throw Error('Graph does not match the model catalog');
const manifest = {
  formatVersion: 1, tag: values.tag, releaseStatus: 'experimental', causalAuditStatus: 'failed', liteStatus: 'blocked',
  integrityNotice: 'SHA-256 verifies content integrity, not publisher identity or a cryptographic signature.',
  attribution: { maleCns: 'Janelia Research Campus / FlyEM MaleCNS v1.0, CC-BY. See THIRD_PARTY_NOTICES.md and https://www.janelia.org/project-team/flyem',
    stockfish: 'Stockfish.js v19.0.0 Lite single-thread, GPLv3. Portable bundle preserves Copying.txt, AUTHORS, NOTICE.txt, sources.json and corresponding upstream source archive.' },
  models: models.map(model => ({ kind: model.expectedKind, trainingPreset: model.trainingPreset, trialsRun: model.trialsRun, checkpointSha256: model.checkpointSha256, graphNeuronsSha256: graphHash, causalAuditStatus: 'failed' })),
  files: files.sort((a, b) => a.path.localeCompare(b.path)),
};
const schema = JSON.parse(await readFile(new URL('./release-manifest.schema.json', import.meta.url), 'utf8'));
validateReleaseManifest(manifest, schema);
await writeFile(path.join(out, 'release-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Prepared ${files.length} files, ${files.reduce((sum, file) => sum + file.size, 0)} bytes. ${path.join(out, 'release-manifest.json')}`);
console.log('EXPERIMENTAL: Bio and Max failed their causal gates; Lite blocked. Nothing uploaded unless --publish was explicitly supplied.');
if (values.publish) {
  const assets = (await readdir(out)).map(name => path.join(out, name));
  const result = spawnSync('gh', ['release', 'create', values.tag, '--repo', 'LeonardSEO/stockfly', '--prerelease', '--title', `StockFly ${values.tag} — experimental`, '--notes', 'Experimental runtime and pretrained artifacts. Both Full causal audits failed; Lite is blocked. Full denotes graph coverage, not validated playing strength. See packaged source, licenses and release evidence.', ...assets], { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
