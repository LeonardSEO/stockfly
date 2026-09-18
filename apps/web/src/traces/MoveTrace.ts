import type { ActivationFrame, DecisionReadout } from '../brain/activation';

export const MOVE_TRACE_FORMAT = 'stockfly-move-trace';
export const MOVE_TRACE_VERSION = 1;
const BINARY_MAGIC = 'SFTB';
const BINARY_VERSION = 1;
const BINARY_HEADER_BYTES = 16;
const FRAME_HEADER_BYTES = 12;

export interface TraceProvenance {
  inputFen: string;
  backend: string;
  adapter: string;
  fallbackReason?: string;
  modelKind: string;
  modelBadge: string;
  checkpointSha256: string | null;
  graphManifestSha256: string;
  graphNeuronsSha256: string;
  sensoryMapSha256: string;
  outputMapSha256: string;
}

export interface FinalDecision extends DecisionReadout {
  settleSteps: number;
}

export interface MoveTrace {
  format: typeof MOVE_TRACE_FORMAT;
  version: typeof MOVE_TRACE_VERSION;
  traceId: string;
  provenance: TraceProvenance;
  frames: ActivationFrame[];
  finalDecision: FinalDecision;
}

interface JsonFrame extends Omit<ActivationFrame, 'neuronRates'> {
  activationEncoding: 'u8' | 'f32';
  maxRate?: number;
}

interface MoveTraceJson extends Omit<MoveTrace, 'frames'> {
  activationBinary: {
    formatVersion: typeof BINARY_VERSION;
    neuronCount: number;
    frameCount: number;
  };
  frames: JsonFrame[];
}

export interface ExportedMoveTrace {
  json: string;
  binary: Uint8Array;
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(`Invalid move trace: ${message}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function finiteNumber(value: unknown, label: string): asserts value is number {
  assert(typeof value === 'number' && Number.isFinite(value), `${label} must be finite`);
}

function numberArray(value: unknown, length: number, label: string): asserts value is number[] {
  assert(Array.isArray(value) && value.length === length, `${label} must contain ${length} values`);
  value.forEach((item, index) => finiteNumber(item, `${label}[${index}]`));
}

function validateDecision(value: unknown, label: string): asserts value is DecisionReadout {
  assert(isRecord(value), `${label} must be an object`);
  assert(typeof value.selectedMove === 'string' && /^[a-h][1-8][a-h][1-8][qrbn]?$/.test(value.selectedMove), `${label}.selectedMove is invalid`);
  numberArray(value.fromRates, 64, `${label}.fromRates`);
  numberArray(value.toRates, 64, `${label}.toRates`);
  numberArray(value.promotionRates, 4, `${label}.promotionRates`);
  assert(Array.isArray(value.legalScores), `${label}.legalScores must be an array`);
  value.legalScores.forEach((entry, index) => {
    assert(isRecord(entry), `${label}.legalScores[${index}] must be an object`);
    assert(typeof entry.move === 'string' && /^[a-h][1-8][a-h][1-8][qrbn]?$/.test(entry.move), `${label}.legalScores[${index}].move is invalid`);
    finiteNumber(entry.score, `${label}.legalScores[${index}].score`);
  });
}

function validateHash(value: unknown, label: string): asserts value is string {
  assert(typeof value === 'string' && /^[a-f0-9]{64}$/.test(value), `${label} must be a SHA-256 hash`);
}

function validateTraceJson(value: unknown): asserts value is MoveTraceJson {
  assert(isRecord(value), 'JSON root must be an object');
  assert(value.format === MOVE_TRACE_FORMAT, `format must be ${MOVE_TRACE_FORMAT}`);
  assert(value.version === MOVE_TRACE_VERSION, `unsupported JSON version ${String(value.version)}`);
  assert(typeof value.traceId === 'string' && value.traceId.length > 0, 'traceId is required');
  assert(isRecord(value.provenance), 'provenance is required');
  const provenance = value.provenance;
  for (const field of ['inputFen', 'backend', 'modelKind', 'modelBadge'] as const) {
    assert(typeof provenance[field] === 'string' && provenance[field].length > 0, `provenance.${field} is required`);
  }
  assert(typeof provenance.adapter === 'string', 'provenance.adapter must be a string');
  assert(provenance.checkpointSha256 === null || typeof provenance.checkpointSha256 === 'string', 'provenance.checkpointSha256 is invalid');
  if (provenance.checkpointSha256 !== null) validateHash(provenance.checkpointSha256, 'provenance.checkpointSha256');
  validateHash(provenance.graphManifestSha256, 'provenance.graphManifestSha256');
  validateHash(provenance.graphNeuronsSha256, 'provenance.graphNeuronsSha256');
  validateHash(provenance.sensoryMapSha256, 'provenance.sensoryMapSha256');
  validateHash(provenance.outputMapSha256, 'provenance.outputMapSha256');
  assert(isRecord(value.activationBinary), 'activationBinary is required');
  assert(value.activationBinary.formatVersion === BINARY_VERSION, `unsupported binary version ${String(value.activationBinary.formatVersion)}`);
  assert(Number.isInteger(value.activationBinary.neuronCount) && (value.activationBinary.neuronCount as number) > 0, 'neuronCount must be positive');
  assert(Number.isInteger(value.activationBinary.frameCount) && (value.activationBinary.frameCount as number) > 0, 'frameCount must be positive');
  assert(Array.isArray(value.frames) && value.frames.length === value.activationBinary.frameCount, 'frame count does not match metadata');
  value.frames.forEach((frame, index) => {
    assert(isRecord(frame), `frames[${index}] must be an object`);
    for (const field of ['step', 'tMs', 'elapsedMs'] as const) finiteNumber(frame[field], `frames[${index}].${field}`);
    assert(Number.isInteger(frame.step) && (frame.step as number) > 0, `frames[${index}].step must be a positive integer`);
    assert((frame.tMs as number) >= 0 && (frame.elapsedMs as number) >= 0, `frames[${index}] times must not be negative`);
    assert(typeof frame.backend === 'string' && frame.backend.length > 0, `frames[${index}].backend is required`);
    assert(frame.activationEncoding === 'u8' || frame.activationEncoding === 'f32', `frames[${index}].activationEncoding is invalid`);
    if (frame.activationEncoding === 'u8') {
      finiteNumber(frame.maxRate, `frames[${index}].maxRate`);
      assert((frame.maxRate as number) > 0, `frames[${index}].maxRate must be positive`);
    }
    assert(Array.isArray(frame.topNeurons), `frames[${index}].topNeurons must be an array`);
    frame.topNeurons.forEach((entry, entryIndex) => {
      assert(isRecord(entry), `frames[${index}].topNeurons[${entryIndex}] must be an object`);
      assert(Number.isInteger(entry.denseIndex) && (entry.denseIndex as number) >= 0, `frames[${index}].topNeurons[${entryIndex}].denseIndex is invalid`);
      finiteNumber(entry.rate, `frames[${index}].topNeurons[${entryIndex}].rate`);
    });
    assert(Array.isArray(frame.regionRates), `frames[${index}].regionRates must be an array`);
    frame.regionRates.forEach((entry, entryIndex) => {
      assert(isRecord(entry), `frames[${index}].regionRates[${entryIndex}] must be an object`);
      assert(typeof entry.region === 'string' && entry.region.length > 0, `frames[${index}].regionRates[${entryIndex}].region is required`);
      finiteNumber(entry.rate, `frames[${index}].regionRates[${entryIndex}].rate`);
      assert(Number.isInteger(entry.neuronCount) && (entry.neuronCount as number) > 0, `frames[${index}].regionRates[${entryIndex}].neuronCount is invalid`);
    });
    validateDecision(frame.decision, `frames[${index}].decision`);
  });
  validateDecision(value.finalDecision, 'finalDecision');
  const settleSteps = (value.finalDecision as unknown as Record<string, unknown>).settleSteps;
  finiteNumber(settleSteps, 'finalDecision.settleSteps');
  assert(Number.isInteger(settleSteps) && settleSteps > 0, 'finalDecision.settleSteps must be a positive integer');
  const finalFrame = value.frames[value.frames.length - 1];
  assert(finalFrame.step === settleSteps, 'final recorded step must equal settleSteps');
  assert(finalFrame.decision.selectedMove === value.finalDecision.selectedMove, 'final recorded move does not match finalDecision');
}

function neuronCount(trace: MoveTrace): number {
  assert(trace.frames.length > 0, 'at least one activation frame is required');
  const first = trace.frames[0].neuronRates;
  const count = first instanceof Float32Array ? first.length : first.values.length;
  assert(count > 0, 'activation frames must contain neurons');
  for (const frame of trace.frames) {
    const rates = frame.neuronRates;
    assert((rates instanceof Float32Array ? rates.length : rates.values.length) === count, 'activation frame neuron counts differ');
    validateDecision(frame.decision, `frame ${frame.step} decision`);
  }
  return count;
}

export function exportMoveTrace(trace: MoveTrace): ExportedMoveTrace {
  const count = neuronCount(trace);
  const frameSizes = trace.frames.map(frame => frame.neuronRates instanceof Float32Array ? frame.neuronRates.byteLength : frame.neuronRates.values.byteLength);
  const totalBytes = BINARY_HEADER_BYTES + frameSizes.reduce((sum, size) => sum + FRAME_HEADER_BYTES + size, 0);
  const binary = new Uint8Array(totalBytes);
  const view = new DataView(binary.buffer);
  for (let index = 0; index < BINARY_MAGIC.length; index++) binary[index] = BINARY_MAGIC.charCodeAt(index);
  view.setUint16(4, BINARY_VERSION, true);
  view.setUint32(8, trace.frames.length, true);
  view.setUint32(12, count, true);
  let offset = BINARY_HEADER_BYTES;
  const frames: JsonFrame[] = trace.frames.map(frame => {
    const rates = frame.neuronRates;
    const full = rates instanceof Float32Array;
    let payload: Uint8Array;
    let maxRate: number | undefined;
    if (rates instanceof Float32Array) payload = new Uint8Array(rates.buffer, rates.byteOffset, rates.byteLength);
    else { payload = rates.values; maxRate = rates.maxRate; }
    view.setUint8(offset, full ? 2 : 1);
    view.setFloat32(offset + 4, maxRate ?? 0, true);
    view.setUint32(offset + 8, payload.byteLength, true);
    binary.set(payload, offset + FRAME_HEADER_BYTES);
    offset += FRAME_HEADER_BYTES + payload.byteLength;
    const { neuronRates: _rates, ...metadata } = frame;
    return {
      ...metadata,
      activationEncoding: full ? 'f32' : 'u8',
      ...(maxRate === undefined ? {} : { maxRate }),
    };
  });
  const document: MoveTraceJson = {
    ...trace,
    frames,
    activationBinary: { formatVersion: BINARY_VERSION, neuronCount: count, frameCount: frames.length },
  };
  return { json: `${JSON.stringify(document, null, 2)}\n`, binary };
}

export function importMoveTrace(json: string, binaryInput: ArrayBuffer | Uint8Array): MoveTrace {
  let parsed: unknown;
  try { parsed = JSON.parse(json); }
  catch (error) { throw new Error(`Invalid move trace: JSON parse failed: ${error instanceof Error ? error.message : String(error)}`); }
  validateTraceJson(parsed);
  const binary = binaryInput instanceof Uint8Array ? binaryInput : new Uint8Array(binaryInput);
  assert(binary.byteLength >= BINARY_HEADER_BYTES, 'binary payload is truncated');
  const view = new DataView(binary.buffer, binary.byteOffset, binary.byteLength);
  const magic = String.fromCharCode(...binary.subarray(0, 4));
  assert(magic === BINARY_MAGIC, 'binary magic is invalid');
  assert(view.getUint16(4, true) === BINARY_VERSION, `unsupported activation binary version ${view.getUint16(4, true)}`);
  const frameCount = view.getUint32(8, true);
  const count = view.getUint32(12, true);
  assert(frameCount === parsed.activationBinary.frameCount, 'binary frame count does not match JSON');
  assert(count === parsed.activationBinary.neuronCount, 'binary neuron count does not match JSON');
  let offset = BINARY_HEADER_BYTES;
  const frames: ActivationFrame[] = parsed.frames.map((frame, index) => {
    assert(offset + FRAME_HEADER_BYTES <= binary.byteLength, `binary frame ${index} header is truncated`);
    const encoding = view.getUint8(offset);
    const maxRate = view.getFloat32(offset + 4, true);
    const byteLength = view.getUint32(offset + 8, true);
    offset += FRAME_HEADER_BYTES;
    assert(offset + byteLength <= binary.byteLength, `binary frame ${index} payload is truncated`);
    assert((encoding === 1 ? 'u8' : encoding === 2 ? 'f32' : '') === frame.activationEncoding, `binary frame ${index} encoding does not match JSON`);
    const expectedBytes = encoding === 1 ? count : count * Float32Array.BYTES_PER_ELEMENT;
    assert(byteLength === expectedBytes, `binary frame ${index} has the wrong neuron count`);
    let neuronRates: ActivationFrame['neuronRates'];
    if (encoding === 1) {
      assert(maxRate === frame.maxRate, `binary frame ${index} maxRate does not match JSON`);
      neuronRates = { values: binary.slice(offset, offset + byteLength), maxRate };
    } else {
      const values = new Float32Array(count);
      for (let neuron = 0; neuron < count; neuron++) values[neuron] = view.getFloat32(offset + neuron * 4, true);
      neuronRates = values;
    }
    offset += byteLength;
    const { activationEncoding: _encoding, maxRate: _maxRate, ...metadata } = frame;
    return { ...metadata, neuronRates };
  });
  assert(offset === binary.byteLength, 'binary payload has trailing data');
  const { activationBinary: _activationBinary, ...trace } = parsed;
  return { ...trace, frames };
}
