import assert from 'node:assert/strict';
import test from 'node:test';
import {
  availableModel,
  ModelSelectionLifecycle,
  parseCheckpointIdentity,
  parseModelCatalog,
  validateCheckpointIdentity,
} from './modelCatalog.ts';

const hash = (char: string) => char.repeat(64);
const catalogDocument = () => ({
  formatVersion: 1,
  models: [
    {
      id: 'bio-full', label: 'Bio Full', expectedKind: 'bio-full', availability: 'available',
      checkpointUrl: '/vendor/models/checkpoints/bio-full-a.sfckpt', checkpointSha256: hash('a'),
      trainingPreset: 'quick', trialsRun: 4934, graphNeuronsSha256: hash('b'),
      sensoryMapSha256: hash('c'), outputMapSha256: hash('d'),
    },
    {
      id: 'max-full', label: 'Max Full', expectedKind: 'max-full', availability: 'available',
      checkpointUrl: '/vendor/models/checkpoints/max-full-e.sfckpt', checkpointSha256: hash('e'),
      trainingPreset: 'smoke', trialsRun: 498, graphNeuronsSha256: hash('b'),
      sensoryMapSha256: hash('c'), outputMapSha256: hash('d'),
    },
    {
      id: 'lite', label: 'Lite', expectedKind: 'lite', availability: 'unavailable',
      reason: 'The Full causal prerequisite did not pass.',
    },
  ],
});

function checkpoint(modelKind = 'max-full'): string {
  return JSON.stringify({
    format_version: 1,
    model_kind: modelKind,
    graph_neurons_sha256: hash('b'),
    sensory_map_sha256: hash('c'),
    output_map_sha256: hash('d'),
    preset: 'smoke',
    rng_seed: 42,
    trials_run: 498,
    teacher_top1_accuracy: 0.08,
    deltas: [[1, 2]],
  });
}

test('catalog exposes fixed Bio and Max assets and an honest unavailable Lite entry', () => {
  const catalog = parseModelCatalog(catalogDocument());
  assert.equal(availableModel(catalog, 'bio-full').trainingPreset, 'quick');
  assert.equal(availableModel(catalog, 'max-full').trialsRun, 498);
  assert.throws(() => availableModel(catalog, 'lite'), /Full causal prerequisite did not pass/);
});

test('checkpoint identity comes from content and rejects an incompatible model kind', () => {
  const entry = availableModel(parseModelCatalog(catalogDocument()), 'max-full');
  const identity = parseCheckpointIdentity(checkpoint());
  validateCheckpointIdentity(entry, identity, hash('e'));
  assert.equal(identity.modelKind, 'max-full');
  assert.equal(identity.trainingPreset, 'smoke');
  assert.equal(identity.trialsRun, 498);
  assert.throws(
    () => validateCheckpointIdentity(entry, parseCheckpointIdentity(checkpoint('bio-full')), hash('e')),
    /model kind mismatch: expected max-full, got bio-full/,
  );
});

test('catalog metadata and asset paths cannot silently redirect a fixed model id', () => {
  const wrongKind = catalogDocument();
  wrongKind.models[1].expectedKind = 'bio-full';
  assert.throws(() => parseModelCatalog(wrongKind), /incompatible expected kind/);
  const arbitraryPath = catalogDocument();
  arbitraryPath.models[1].checkpointUrl = '/tmp/user-selected.sfckpt';
  assert.throws(() => parseModelCatalog(arbitraryPath), /outside the model asset catalog/);
});

test('model load lifecycle rejects stale generations and wrong-model replies', () => {
  const lifecycle = new ModelSelectionLifecycle();
  lifecycle.begin({ generation: 4, modelId: 'bio-full' });
  lifecycle.begin({ generation: 5, modelId: 'max-full' });
  assert.equal(lifecycle.finish({ generation: 4, modelId: 'bio-full' }), false);
  assert.equal(lifecycle.finish({ generation: 5, modelId: 'bio-full' }), false);
  assert.equal(lifecycle.finish({ generation: 5, modelId: 'max-full' }), true);
  assert.equal(lifecycle.accepts({ generation: 5, modelId: 'max-full' }), false);
});
