export const MODEL_IDS = ['bio-full', 'max-full', 'lite'] as const;
export type ModelId = typeof MODEL_IDS[number];

const EXPECTED_KINDS: Record<ModelId, ModelId> = {
  'bio-full': 'bio-full',
  'max-full': 'max-full',
  lite: 'lite',
};

interface ModelCatalogBase {
  id: ModelId;
  label: string;
  expectedKind: ModelId;
}

export interface AvailableModelCatalogEntry extends ModelCatalogBase {
  availability: 'available';
  checkpointUrl: string;
  checkpointSha256: string;
  trainingPreset: string;
  trialsRun: number;
  graphNeuronsSha256: string;
  sensoryMapSha256: string;
  outputMapSha256: string;
}

export interface UnavailableModelCatalogEntry extends ModelCatalogBase {
  availability: 'unavailable';
  reason: string;
}

export type ModelCatalogEntry = AvailableModelCatalogEntry | UnavailableModelCatalogEntry;

export interface ModelCatalog {
  formatVersion: 1;
  models: ModelCatalogEntry[];
}

export interface CheckpointIdentity {
  formatVersion: number;
  modelKind: string;
  trainingPreset: string;
  trialsRun: number;
  graphNeuronsSha256: string;
  sensoryMapSha256: string;
  outputMapSha256: string;
}

export interface ModelLoadIdentity {
  generation: number;
  modelId: ModelId;
}

export class ModelSelectionLifecycle {
  private active: ModelLoadIdentity | null = null;

  begin(identity: ModelLoadIdentity): void { this.active = identity; }
  cancel(): void { this.active = null; }
  accepts(identity: ModelLoadIdentity): boolean {
    return this.active?.generation === identity.generation && this.active.modelId === identity.modelId;
  }
  finish(identity: ModelLoadIdentity): boolean {
    if (!this.accepts(identity)) return false;
    this.active = null;
    return true;
  }
}

function record(value: unknown, name: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${name} must be an object`);
  return value as Record<string, unknown>;
}

function text(value: unknown, name: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${name} must be a non-empty string`);
  return value;
}

function hash(value: unknown, name: string): string {
  const result = text(value, name);
  if (!/^[a-f0-9]{64}$/.test(result)) throw new Error(`${name} must be a lowercase SHA-256`);
  return result;
}

function modelId(value: unknown): ModelId {
  if (typeof value !== 'string' || !MODEL_IDS.includes(value as ModelId)) throw new Error(`Unsupported model id: ${String(value)}`);
  return value as ModelId;
}

export function parseModelCatalog(value: unknown): ModelCatalog {
  const source = record(value, 'Model catalog');
  if (source.formatVersion !== 1) throw new Error('Unsupported model catalog format');
  if (!Array.isArray(source.models)) throw new Error('Model catalog models must be an array');
  const models = source.models.map((raw, index): ModelCatalogEntry => {
    const item = record(raw, `models[${index}]`);
    const id = modelId(item.id);
    const expectedKind = modelId(item.expectedKind);
    if (expectedKind !== EXPECTED_KINDS[id]) throw new Error(`${id} catalog entry has incompatible expected kind ${expectedKind}`);
    const base = { id, label: text(item.label, `${id}.label`), expectedKind };
    if (item.availability === 'unavailable') {
      return { ...base, availability: 'unavailable', reason: text(item.reason, `${id}.reason`) };
    }
    if (item.availability !== 'available') throw new Error(`${id}.availability is invalid`);
    const checkpointUrl = text(item.checkpointUrl, `${id}.checkpointUrl`);
    if (!/^\/vendor\/models\/checkpoints\/[a-z0-9.-]+\.sfckpt$/.test(checkpointUrl)) {
      throw new Error(`${id}.checkpointUrl is outside the model asset catalog`);
    }
    const trialsRun = item.trialsRun;
    if (!Number.isSafeInteger(trialsRun) || (trialsRun as number) < 1) throw new Error(`${id}.trialsRun must be a positive integer`);
    return {
      ...base,
      availability: 'available',
      checkpointUrl,
      checkpointSha256: hash(item.checkpointSha256, `${id}.checkpointSha256`),
      trainingPreset: text(item.trainingPreset, `${id}.trainingPreset`),
      trialsRun: trialsRun as number,
      graphNeuronsSha256: hash(item.graphNeuronsSha256, `${id}.graphNeuronsSha256`),
      sensoryMapSha256: hash(item.sensoryMapSha256, `${id}.sensoryMapSha256`),
      outputMapSha256: hash(item.outputMapSha256, `${id}.outputMapSha256`),
    };
  });
  for (const id of MODEL_IDS) {
    if (models.filter(model => model.id === id).length !== 1) throw new Error(`Model catalog must contain exactly one ${id} entry`);
  }
  if (models.find(model => model.id === 'bio-full')?.availability !== 'available'
      || models.find(model => model.id === 'max-full')?.availability !== 'available') {
    throw new Error('Bio Full and Max Full must have prepared checkpoints');
  }
  if (models.find(model => model.id === 'lite')?.availability !== 'unavailable') {
    throw new Error('Lite must remain unavailable until its prerequisite passes');
  }
  return { formatVersion: 1, models };
}

export function availableModel(catalog: ModelCatalog, id: ModelId): AvailableModelCatalogEntry {
  const entry = catalog.models.find(model => model.id === id);
  if (!entry) throw new Error(`Model ${id} is not in the catalog`);
  if (entry.availability !== 'available') throw new Error(`${entry.label} is unavailable: ${entry.reason}`);
  return entry;
}

/** Reads the generated checkpoint header without parsing the very large deltas array twice. */
export function parseCheckpointIdentity(checkpointText: string): CheckpointIdentity {
  const deltasMarker = checkpointText.indexOf(',"deltas":');
  if (!checkpointText.startsWith('{') || deltasMarker < 0) throw new Error('Checkpoint metadata header or deltas field is missing');
  const source = record(JSON.parse(`${checkpointText.slice(0, deltasMarker)}}`), 'Checkpoint');
  const trialsRun = source.trials_run;
  if (!Number.isSafeInteger(trialsRun) || (trialsRun as number) < 1) throw new Error('Checkpoint trials_run must be a positive integer');
  if (!Number.isSafeInteger(source.format_version) || (source.format_version as number) < 1) throw new Error('Checkpoint format_version is invalid');
  return {
    formatVersion: source.format_version as number,
    modelKind: text(source.model_kind, 'Checkpoint model_kind'),
    trainingPreset: text(source.preset, 'Checkpoint preset'),
    trialsRun: trialsRun as number,
    graphNeuronsSha256: hash(source.graph_neurons_sha256, 'Checkpoint graph_neurons_sha256'),
    sensoryMapSha256: hash(source.sensory_map_sha256, 'Checkpoint sensory_map_sha256'),
    outputMapSha256: hash(source.output_map_sha256, 'Checkpoint output_map_sha256'),
  };
}

export function validateCheckpointIdentity(
  entry: AvailableModelCatalogEntry,
  checkpoint: CheckpointIdentity,
  checkpointSha256: string,
): void {
  if (checkpoint.formatVersion !== 1) throw new Error(`Unsupported checkpoint format ${checkpoint.formatVersion}`);
  const comparisons: Array<[string, string | number, string | number]> = [
    ['model kind', checkpoint.modelKind, entry.expectedKind],
    ['training preset', checkpoint.trainingPreset, entry.trainingPreset],
    ['trial count', checkpoint.trialsRun, entry.trialsRun],
    ['checkpoint SHA-256', checkpointSha256, entry.checkpointSha256],
    ['graph neurons SHA-256', checkpoint.graphNeuronsSha256, entry.graphNeuronsSha256],
    ['sensory map SHA-256', checkpoint.sensoryMapSha256, entry.sensoryMapSha256],
    ['output map SHA-256', checkpoint.outputMapSha256, entry.outputMapSha256],
  ];
  const mismatch = comparisons.find(([, actual, expected]) => actual !== expected);
  if (mismatch) throw new Error(`${entry.label} ${mismatch[0]} mismatch: expected ${mismatch[2]}, got ${mismatch[1]}`);
}
