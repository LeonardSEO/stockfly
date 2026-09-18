import test from 'node:test';
import assert from 'node:assert/strict';
import type { ActivationFrame } from '../brain/activation.ts';
import { FrameStreamTracker } from '../engine/frameStream.ts';
import { compareRecordedActivation } from '../engine/traceVerification.ts';
import { exportMoveTrace, importMoveTrace, MOVE_TRACE_VERSION, type MoveTrace } from './MoveTrace.ts';
import { TraceTimeline } from './TraceTimeline.ts';
import { modelTruthBadge } from '../engine/modelTruth.ts';

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
  format: 'stockfly-move-trace', version: MOVE_TRACE_VERSION, traceId: '2:7',
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

async function jsonForBinary(json: string, binary: Uint8Array): Promise<string> {
  const parsed = JSON.parse(json);
  const digest = await crypto.subtle.digest('SHA-256', binary.slice().buffer);
  parsed.activationBinary.sha256 = [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('');
  return JSON.stringify(parsed);
}

test('versioned JSON and binary activation payload round-trip move, hashes and both encodings', async () => {
  const exported = await exportMoveTrace(trace);
  const restored = await importMoveTrace(exported.json, exported.binary);
  assert.equal(restored.finalDecision.selectedMove, 'a2a4');
  assert.deepEqual(restored.provenance, trace.provenance);
  assert.deepEqual(restored.frames[0].decision, trace.frames[0].decision);
  assert.deepEqual([...((restored.frames[0].neuronRates as { values: Uint8Array }).values)], [0, 13, 255]);
  assert.deepEqual([...restored.frames[1].neuronRates as Float32Array], [0, 1.25, 9.5]);
});

test('binary import rejects corruption and mismatched frame data', async () => {
  const exported = await exportMoveTrace(trace);
  const badMagic = exported.binary.slice(); badMagic[0] = 0;
  await assert.rejects(async () => importMoveTrace(await jsonForBinary(exported.json, badMagic), badMagic), /magic/);
  const truncated = exported.binary.slice(0, -1);
  await assert.rejects(async () => importMoveTrace(await jsonForBinary(exported.json, truncated), truncated), /truncated/);
  const trailing = new Uint8Array(exported.binary.length + 1); trailing.set(exported.binary);
  await assert.rejects(async () => importMoveTrace(await jsonForBinary(exported.json, trailing), trailing), /trailing/);
  const invalidMetadata = JSON.parse(exported.json); invalidMetadata.frames[0].topNeurons = null;
  await assert.rejects(() => importMoveTrace(JSON.stringify(invalidMetadata), exported.binary), /topNeurons/);
  const mismatchedDecision = JSON.parse(exported.json); mismatchedDecision.finalDecision.selectedMove = 'h2h4';
  await assert.rejects(() => importMoveTrace(JSON.stringify(mismatchedDecision), exported.binary), /final recorded move/);
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

test('import rejects forged model kinds and badges before they reach the decision panel', async () => {
  const exported = await exportMoveTrace(trace);
  const forgedBadge = JSON.parse(exported.json);
  forgedBadge.provenance.modelBadge = 'BIO FULL · definitely stronger';
  await assert.rejects(() => importMoveTrace(JSON.stringify(forgedBadge), exported.binary), /modelBadge does not match/);
  const unknownKind = JSON.parse(exported.json);
  unknownKind.provenance.modelKind = 'super-full';
  unknownKind.provenance.modelBadge = 'BIO FULL · complete MaleCNS';
  await assert.rejects(() => importMoveTrace(JSON.stringify(unknownKind), exported.binary), /modelKind is unsupported/);
});

test('import rejects mixed backends, non-monotonic steps and a same-shape swapped binary', async () => {
  const exported = await exportMoveTrace(trace);
  const mixedBackend = JSON.parse(exported.json);
  mixedBackend.frames[0].backend = 'browser-webgpu';
  await assert.rejects(() => importMoveTrace(JSON.stringify(mixedBackend), exported.binary), /backend does not match/);
  const duplicateStep = JSON.parse(exported.json);
  duplicateStep.frames[1].step = duplicateStep.frames[0].step;
  await assert.rejects(() => importMoveTrace(JSON.stringify(duplicateStep), exported.binary), /strictly increasing/);

  const other: MoveTrace = {
    ...trace,
    traceId: 'other',
    frames: trace.frames.map((item, index) => ({
      ...item,
      neuronRates: index === 0
        ? { values: new Uint8Array([255, 13, 0]), maxRate: 20 }
        : new Float32Array([9.5, 1.25, 0]),
    })),
  };
  const otherExport = await exportMoveTrace(other);
  await assert.rejects(() => importMoveTrace(exported.json, otherExport.binary), /SHA-256 does not match/);
});

test('backend fallback restart discards samples from the abandoned attempt', () => {
  const tracker = new FrameStreamTracker();
  let retained: string[] = [];
  for (const [step, backend] of [[1, 'browser-webgpu'], [2, 'browser-webgpu'], [1, 'cpu-wasm'], [2, 'cpu-wasm']] as const) {
    const observed = tracker.observe({ step, backend });
    if (observed.restarted) retained = [];
    retained.push(`${observed.attempt}:${backend}:${step}`);
  }
  assert.deepEqual(retained, ['1:cpu-wasm:1', '1:cpu-wasm:2']);
});

test('full-precision activation failure is not masked by a quantized frame tolerance', () => {
  const full = compareRecordedActivation(new Float32Array([0]), new Float32Array([0.001]));
  const quantized = compareRecordedActivation({ values: new Uint8Array([0]), maxRate: 20 }, new Float32Array([0.02]));
  assert.equal(full.matches, false);
  assert.ok(full.maxDifference < quantized.tolerance);
  assert.equal(quantized.matches, true);
  assert.equal([full, quantized].every(result => result.matches), false);
});
