import { rateAt, type ActivationFrame } from '../brain/activation.ts';

export const CPU_DECISION_TOLERANCE = 1e-5;

export interface ActivationComparison {
  matches: boolean;
  maxDifference: number;
  tolerance: number;
}

export function compareRecordedActivation(
  recorded: ActivationFrame['neuronRates'],
  actual: Float32Array,
): ActivationComparison {
  const recordedLength = recorded instanceof Float32Array ? recorded.length : recorded.values.length;
  const tolerance = recorded instanceof Float32Array
    ? CPU_DECISION_TOLERANCE
    : recorded.maxRate / 510 + 1e-6;
  if (recordedLength !== actual.length) return { matches: false, maxDifference: Infinity, tolerance };
  let maxDifference = 0;
  for (let index = 0; index < actual.length; index++) {
    maxDifference = Math.max(maxDifference, Math.abs(actual[index] - rateAt(recorded, index)));
  }
  return { matches: maxDifference <= tolerance, maxDifference, tolerance };
}
