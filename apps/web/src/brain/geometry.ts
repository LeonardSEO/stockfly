export interface SomaGeometry { positions: Float32Array; denseIndices: Uint32Array }
export type NeuronAnnotation = [type: string, className: string, superclass: string, somaNeuromere: string];
export interface BrainData extends SomaGeometry {
  bodyIds: string[];
  annotations: NeuronAnnotation[];
  outputs: Map<number, string[]>;
  graphHash: string;
}
export function parseSomas(buffer: ArrayBuffer, neuronCount: number): SomaGeometry {
  if (buffer.byteLength % 16) throw new Error("Invalid soma geometry length");
  const count = buffer.byteLength / 16;
  const view = new DataView(buffer);
  const positions = new Float32Array(count * 3);
  const denseIndices = new Uint32Array(count);
  const seen = new Set<number>();
  for (let i = 0; i < count; i++) {
    const denseIndex = view.getUint32(i * 16, true);
    if (denseIndex >= neuronCount || seen.has(denseIndex)) throw new Error("Invalid soma neuron index");
    seen.add(denseIndex);
    denseIndices[i] = denseIndex;
    for (let axis = 0; axis < 3; axis++) {
      const value = view.getFloat32(i * 16 + 4 + axis * 4, true);
      if (!Number.isFinite(value)) throw new Error("Invalid soma coordinate");
      positions[i * 3 + axis] = value;
    }
  }
  if (!count) throw new Error("No soma geometry available");
  return { positions, denseIndices };
}
export function parseBodyIds(buffer: ArrayBuffer): string[] {
  if (buffer.byteLength % 8) throw new Error("Invalid neuron ID length");
  const view = new DataView(buffer);
  return Array.from({ length: buffer.byteLength / 8 }, (_, i) => view.getBigUint64(i * 8, true).toString());
}
export function parseAnnotations(value: unknown, count: number, graphHash: string): NeuronAnnotation[] {
  const doc = value as { formatVersion?: unknown; graphNeuronsSha256?: unknown; fields?: unknown; neurons?: unknown };
  if (!doc || doc.formatVersion !== 1 || doc.graphNeuronsSha256 !== graphHash
    || JSON.stringify(doc.fields) !== JSON.stringify(['type', 'class', 'superclass', 'somaNeuromere'])
    || !Array.isArray(doc.neurons) || doc.neurons.length !== count
    || !doc.neurons.every(row => Array.isArray(row) && row.length === 4 && row.every(item => typeof item === 'string'))) {
    throw new Error("Browser annotations do not match the loaded graph; run tools/browser/export_metadata.py");
  }
  return doc.neurons as NeuronAnnotation[];
}
async function required(url: string): Promise<Response> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response;
}
export async function loadBrain(): Promise<BrainData> {
  const [manifest, neuronBytes, somaBytes, metadata, output] = await Promise.all([
    required('/vendor/graph/manifest.json').then(r => r.json()),
    required('/vendor/graph/neurons.bin').then(r => r.arrayBuffer()),
    required('/vendor/graph/geometry_lod0.bin').then(r => r.arrayBuffer()),
    required('/vendor/brain/metadata.json').then(r => r.json()),
    required('/vendor/chess-maps/output-map.json').then(r => r.json()),
  ]);
  const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', neuronBytes)), b => b.toString(16).padStart(2, '0')).join('');
  if (hash !== manifest.neurons_sha256) throw new Error('Neuron IDs do not match the graph manifest');
  const bodyIds = parseBodyIds(neuronBytes);
  const annotations = parseAnnotations(metadata, bodyIds.length, hash);
  const outputs = new Map<number, string[]>();
  const add = (groups: number[][], prefix: string) => groups.forEach((group, index) => group.forEach(denseIndex => {
    if (!Number.isInteger(denseIndex) || denseIndex < 0 || denseIndex >= bodyIds.length) throw new Error('Invalid output neuron index');
    const label = `${'abcdefgh'[index % 8]}${Math.floor(index / 8) + 1}`;
    outputs.set(denseIndex, [...(outputs.get(denseIndex) ?? []), `${prefix}: ${label}`]);
  }));
  add(output.from_groups, 'from'); add(output.to_groups, 'to');
  for (const [name, group] of Object.entries(output.promotion_groups as Record<string, number[]>)) {
    group.forEach(index => outputs.set(index, [...(outputs.get(index) ?? []), `promotion: ${name}`]));
  }
  return { ...parseSomas(somaBytes, bodyIds.length), bodyIds, annotations, outputs, graphHash: hash };
}
