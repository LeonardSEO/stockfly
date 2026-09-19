import test from 'node:test';
import assert from 'node:assert/strict';
import { activationColor, intensity, quantize, rateAt, topNeurons, summarizeRegions } from './activation.ts';
import { parseAnnotations, parseBodyIds, parseSomas } from './geometry.ts';

test('activation colors use the full quiet-to-active spectrum and clamp rates', () => {
  assert.deepEqual(activationColor(0), [0.08, 0.17, 0.28]);
  assert.deepEqual(activationColor(-1), activationColor(0));
  assert.deepEqual(activationColor(25), activationColor(20));
  assert.deepEqual(activationColor(6), [0.08, 0.69, 0.82]);
  assert.deepEqual(activationColor(20), [1, 0.24, 0.43]);
  for (const rate of [0, 1, 6, 10, 14, 20]) {
    activationColor(rate).forEach(channel => assert.ok(channel >= 0 && channel <= 1));
  }
  assert.equal(intensity(NaN), 0);
});
test('quantized samples preserve actual rates within half a bin and exact top values', () => {
  const rates = new Float32Array([0, 0.5, 5, 12.678, 20]);
  const sample = quantize(rates);
  rates.forEach((rate, i) => assert.ok(Math.abs(rateAt(sample, i) - rate) <= 20 / 510));
  assert.deepEqual(topNeurons(rates, 2), [{ denseIndex: 4, rate: 20 }, { denseIndex: 3, rate: rates[3] }]);
});
test('real binary layout preserves sparse dense indexes and uint64 body IDs', () => {
  const ids = new ArrayBuffer(16); const idView = new DataView(ids);
  idView.setBigUint64(0, 9007199254740993n, true); idView.setBigUint64(8, 18446744073709551615n, true);
  assert.deepEqual(parseBodyIds(ids), ['9007199254740993', '18446744073709551615']);
  const points = new ArrayBuffer(16); const view = new DataView(points);
  view.setUint32(0, 1, true); [1.25, -3.5, 500].forEach((value, axis) => view.setFloat32(4 + 4 * axis, value, true));
  const parsed = parseSomas(points, 2);
  assert.deepEqual([...parsed.denseIndices], [1]);
  assert.deepEqual([...parsed.positions], [1.25, -3.5, 500]);
  assert.throws(() => parseSomas(points, 1), /index/);
  assert.throws(() => parseSomas(new ArrayBuffer(3), 2), /length/);
  view.setFloat32(4, Infinity, true); assert.throws(() => parseSomas(points, 2), /coordinate/);
});
test('annotations must align with graph and keep superclass distinct from soma neuromere', () => {
  const doc = { formatVersion: 1, fields: ['type', 'class', 'superclass', 'somaNeuromere'], graphNeuronsSha256: 'abc', neurons: [['DN1', 'descending', 'descending_neuron', 'T1']] };
  assert.deepEqual(parseAnnotations(doc, 1, 'abc')[0], ['DN1', 'descending', 'descending_neuron', 'T1']);
  assert.throws(() => parseAnnotations(doc, 1, 'def'), /match/);
  assert.throws(() => parseAnnotations(doc, 2, 'abc'), /match/);
});

test('neuromere summaries use exact full rates including neurons without soma geometry', () => {
  assert.deepEqual(summarizeRegions(new Float32Array([1, 3, 4]), ['T1', 'T1', '']), [
    { region: 'T1', rate: 2, neuronCount: 2 },
    { region: 'Unannotated', rate: 4, neuronCount: 1 },
  ]);
  assert.throws(() => summarizeRegions(new Float32Array([1]), []), /match/);
});
