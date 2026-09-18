import test from 'node:test';
import assert from 'node:assert/strict';
import type { ActivationFrame } from '../brain/activation.ts';
import { exportMoveTrace, importMoveTrace, type MoveTrace } from './MoveTrace.ts';
import { TraceTimeline } from './TraceTimeline.ts';
import { modelTruthBadge } from '../engine/protocol.ts';

const hash = (digit: string) => digit.repeat(64);
const decision = (selectedMove: string, seed: number) => ({
  selectedMove,
  fromRates: Array.from({ length: 64 }, (_, index) => seed + index / 100),
  toRates: Array.from({ length: 64 }, (_, index) => seed + index / 50),
  promotionRates: [seed, seed + 1, seed + 2, seed + 3],
  legalScores: [{ move: selectedMove, score: seed + 2.5 }, { move: 'a7a6', score: seed + 1 }],
});
const frame = (step: number, selectedMove: string, seed: number, neuronRates: ActivationFrame['neuronRates']): ActivationFrame => ({
  step, tMs: step, elapsedMs: step * 4, backend: 'cpu-wasm', neuronRates,
  topNeurons: [{ denseIndex: 1, rate: seed }],
  regionRates: [{ region: 'T1', rate: seed, neuronCount: 2 }],
  decision: decision(selectedMove, seed),
});
const trace: MoveTrace = {
  format: 'stockfly-move-trace', version: 1, traceId: '2:7',
  provenance: {
    inputFen: '8/8/8/8/8/8/k6K/8 b - - 0 1', backend: 'cpu-wasm', adapter: '',
    modelKind: 'bio-full', modelBadge: 'BIO FULL · complete MaleCNS', checkpointSha256: hash('a'),
    graphManifestSha256: hash('b'), graphNeuronsSha256: hash('c'), sensoryMapSha256: hash('d'), outputMapSha256: hash('e'),
  },
  frames: [
    frame(1, 'a2a3', 1, { values: new Uint8Array([0, 13, 255]), maxRate: 20 }),
    frame(2, 'a2a4', 2, new Float32Array([0, 1.25, 9.5])),
  ],
  finalDecision: { ...decision('a2a4', 2), settleSteps: 2 },
};

test('versioned JSON and binary activation payload round-trip move, hashes and both encodings', () => {
  const exported = exportMoveTrace(trace);
  const restored = importMoveTrace(exported.json, exported.binary);
  assert.equal(restored.finalDecision.selectedMove, 'a2a4');
  assert.deepEqual(restored.provenance, trace.provenance);
  assert.deepEqual(restored.frames[0].decision, trace.frames[0].decision);
  assert.deepEqual([...((restored.frames[0].neuronRates as { values: Uint8Array }).values)], [0, 13, 255]);
  assert.deepEqual([...restored.frames[1].neuronRates as Float32Array], [0, 1.25, 9.5]);
});

test('binary import rejects corruption and mismatched frame data', () => {
  const exported = exportMoveTrace(trace);
  const badMagic = exported.binary.slice(); badMagic[0] = 0;
  assert.throws(() => importMoveTrace(exported.json, badMagic), /magic/);
  assert.throws(() => importMoveTrace(exported.json, exported.binary.slice(0, -1)), /truncated/);
  const trailing = new Uint8Array(exported.binary.length + 1); trailing.set(exported.binary);
  assert.throws(() => importMoveTrace(exported.json, trailing), /trailing/);
  const invalidMetadata = JSON.parse(exported.json); invalidMetadata.frames[0].topNeurons = null;
  assert.throws(() => importMoveTrace(JSON.stringify(invalidMetadata), exported.binary), /topNeurons/);
  const mismatchedDecision = JSON.parse(exported.json); mismatchedDecision.finalDecision.selectedMove = 'h2h4';
  assert.throws(() => importMoveTrace(JSON.stringify(mismatchedDecision), exported.binary), /final recorded move/);
});

test('timeline scrubbing returns each recorded step readout instead of repeating the final decision', () => {
  const observed: string[] = [];
  const timeline = new TraceTimeline(trace, current => observed.push(`${current.step}:${current.decision.selectedMove}:${current.decision.fromRates[0]}`));
  assert.equal(timeline.seek(0).decision.selectedMove, 'a2a3');
  assert.equal(timeline.seek(1).decision.selectedMove, 'a2a4');
  assert.deepEqual(observed, ['1:a2a3:1', '2:a2a4:2']);
  assert.throws(() => timeline.seek(2), /range/);
});

test('model truth badges distinguish complete, pruned and untrained models', () => {
  assert.equal(modelTruthBadge('bio-full'), 'BIO FULL · complete MaleCNS');
  assert.equal(modelTruthBadge('max-full'), 'MAX FULL · complete MaleCNS');
  assert.equal(modelTruthBadge('lite'), 'LITE · pruned MaleCNS subset');
  assert.equal(modelTruthBadge('untrained baseline'), 'UNTRAINED · complete MaleCNS baseline');
  assert.throws(() => modelTruthBadge('mystery'), /Unsupported/);
});
