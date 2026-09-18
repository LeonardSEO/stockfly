/** Rates are dimensionless values of the documented LIF/rate-hybrid model. */
export const MAX_RATE = 20;
export interface QuantizedActivation { values: Uint8Array; maxRate: number }
export interface ActivationFrame {
  step: number;
  tMs: number;
  elapsedMs: number;
  backend: string;
  neuronRates: Float32Array | QuantizedActivation;
  topNeurons: Array<{ denseIndex: number; rate: number }>;
  regionRates: Array<{ region: string; rate: number; neuronCount: number }>;
}
export function intensity(rate: number, maxRate = MAX_RATE): number {
  return Math.max(0, Math.min(1, Number.isFinite(rate) ? rate / maxRate : 0));
}
export function activationColor(rate: number): [number, number, number] {
  const value = intensity(rate);
  return [0.15 + value * 0.57, 0.19 + value * 0.71, 0.16 + value * 0.26];
}
export function rateAt(rates: ActivationFrame['neuronRates'] | undefined, index: number): number {
  if (!rates) return 0;
  return rates instanceof Float32Array ? rates[index] ?? 0 : (rates.values[index] ?? 0) * rates.maxRate / 255;
}
export function quantize(rates: Float32Array): QuantizedActivation {
  return { values: Uint8Array.from(rates, rate => Math.round(intensity(rate) * 255)), maxRate: MAX_RATE };
}
export function topNeurons(rates: Float32Array, count = 12): ActivationFrame['topNeurons'] {
  const top: ActivationFrame['topNeurons'] = [];
  rates.forEach((rate, denseIndex) => {
    if (rate <= 0 || (top.length === count && rate <= top[top.length - 1].rate)) return;
    const at = top.findIndex(item => rate > item.rate);
    top.splice(at < 0 ? top.length : at, 0, { denseIndex, rate });
    if (top.length > count) top.pop();
  });
  return top;
}

/** Exact averages over all simulated neurons with a source somaNeuromere label. */
export function summarizeRegions(rates: Float32Array, regions: string[]): ActivationFrame['regionRates'] {
  if (rates.length !== regions.length) throw new Error('Region labels do not match neuron rates');
  const groups = new Map<string, { sum: number; count: number }>();
  rates.forEach((rate, index) => {
    const region = regions[index] || 'Unannotated';
    const group = groups.get(region) ?? { sum: 0, count: 0 };
    group.sum += rate; group.count++;
    groups.set(region, group);
  });
  return [...groups].map(([region, group]) => ({ region, rate: group.sum / group.count, neuronCount: group.count }));
}
