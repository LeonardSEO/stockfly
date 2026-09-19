import { MAX_RATE } from './activation';

export function createBrainLegend(): HTMLElement {
  const legend = document.createElement('div');
  legend.className = 'brain-legend';
  legend.innerHTML = `<span>Quiet · 0</span><span class="legend-scale" aria-hidden="true"></span><span>Active · ${MAX_RATE}</span>`;
  legend.title = 'Dimensionless simulated rates. Color uses quantized samples; decision readouts retain full precision.';
  return legend;
}
