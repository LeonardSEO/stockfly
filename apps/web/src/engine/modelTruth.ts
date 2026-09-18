const MODEL_TRUTH_BADGES = {
  'bio-full': 'BIO FULL · complete MaleCNS',
  'max-full': 'MAX FULL · complete MaleCNS',
  lite: 'LITE · pruned MaleCNS subset',
  'untrained baseline': 'UNTRAINED · complete MaleCNS baseline',
} as const;

export type ModelKind = keyof typeof MODEL_TRUTH_BADGES;

export function modelTruthBadge(modelKind: string): string {
  if (!Object.hasOwn(MODEL_TRUTH_BADGES, modelKind)) throw new Error(`Unsupported model kind: ${modelKind}`);
  return MODEL_TRUTH_BADGES[modelKind as ModelKind];
}
