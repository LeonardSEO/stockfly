import type { ActivationFrame, DecisionReadout } from '../brain/activation';
import { importMoveTrace, type MoveTrace } from '../traces/MoveTrace';
import type { TraceVerification } from '../engine/protocol';

export interface DecisionPanelHandlers {
  onSeek: (index: number) => void;
  onReplay: () => void;
  onExport: (kind: 'json' | 'binary') => void;
  onImport: (trace: MoveTrace) => void;
  onVerify: () => void;
}

const squares = Array.from({ length: 64 }, (_, index) => `${'abcdefgh'[index % 8]}${Math.floor(index / 8) + 1}`);
const escapeHtml = (text: string): string => text.replace(/[&<>"']/g, character => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]!);

function bars(rates: number[], selectedSquare: string, label: string): string {
  const maximum = Math.max(...rates, 0.000001);
  return rates.map((rate, index) => {
    const square = squares[index];
    const height = Math.max(2, rate / maximum * 100);
    return `<div class="decision-bar${square === selectedSquare ? ' selected' : ''}" style="--rate-height:${height.toFixed(2)}%" title="${square}: ${rate.toFixed(6)}" aria-label="${label} ${square}: ${rate.toFixed(6)}"><span></span><small>${square}</small></div>`;
  }).join('');
}

function readoutHtml(decision: DecisionReadout): string {
  const from = decision.selectedMove.slice(0, 2);
  const to = decision.selectedMove.slice(2, 4);
  const promotion = decision.selectedMove.slice(4) || 'none';
  return `<div class="decision-heading"><div><span class="eyebrow">RECORDED POLICY READOUT</span><h3>${escapeHtml(decision.selectedMove)}</h3></div><span class="decision-route">${from} → ${to}${promotion === 'none' ? '' : ` · ${promotion}`}</span></div>
    <div class="decision-graphs"><section><h4>From square rates</h4><div class="rate-grid" role="img" aria-label="All 64 exact from-square rates">${bars(decision.fromRates, from, 'From rate')}</div></section><section><h4>To square rates</h4><div class="rate-grid" role="img" aria-label="All 64 exact to-square rates">${bars(decision.toRates, to, 'To rate')}</div></section></div>
    <div class="decision-lists"><section><h4>Top legal moves</h4><ol>${decision.legalScores.slice(0, 5).map(item => `<li${item.move === decision.selectedMove ? ' class="selected"' : ''}><span>${escapeHtml(item.move)}</span><strong>${item.score.toFixed(6)}</strong></li>`).join('')}</ol></section><section><h4>Promotion rates</h4><dl>${['queen', 'rook', 'bishop', 'knight'].map((piece, index) => `<div><dt>${piece}</dt><dd>${decision.promotionRates[index].toFixed(6)}</dd></div>`).join('')}</dl></section></div>`;
}

export class DecisionPanel {
  private readonly output: HTMLElement;
  private readonly slider: HTMLInputElement;
  private readonly timelineLabel: HTMLElement;
  private readonly sourceLabel: HTMLElement;
  private readonly verification: HTMLElement;
  private readonly provenance: HTMLElement;
  private readonly replayButton: HTMLButtonElement;
  private readonly verifyButton: HTMLButtonElement;
  private readonly exportButtons: HTMLButtonElement[];
  private trace: MoveTrace | null = null;

  constructor(host: HTMLElement, handlers: DecisionPanelHandlers) {
    host.innerHTML = `<div class="trace-actions"><button type="button" class="quiet" data-action="replay" disabled>Replay</button><button type="button" class="quiet" data-action="export-json" disabled>Export JSON</button><button type="button" class="quiet" data-action="export-binary" disabled>Export activations</button><label class="quiet import-trace">Import trace<input type="file" accept=".json,.bin,application/json,application/octet-stream" multiple></label><button type="button" class="quiet" data-action="verify" disabled>Verify on CPU</button></div>
      <div class="trace-source muted">No recorded trace.</div>
      <div class="trace-timeline"><input type="range" min="0" max="0" value="0" aria-label="Recorded activity timeline" disabled><output>Waiting for a recorded move.</output></div>
      <div class="decision-output muted">Play a move or import a trace to inspect its decision.</div>
      <details class="trace-provenance"><summary>Reproducibility provenance</summary><pre>No trace loaded.</pre></details>
      <div class="trace-verification muted" role="status" aria-live="polite"></div>`;
    this.output = host.querySelector('.decision-output')!;
    this.slider = host.querySelector('.trace-timeline input')!;
    this.timelineLabel = host.querySelector('.trace-timeline output')!;
    this.sourceLabel = host.querySelector('.trace-source')!;
    this.verification = host.querySelector('.trace-verification')!;
    this.provenance = host.querySelector('.trace-provenance pre')!;
    this.replayButton = host.querySelector('[data-action="replay"]')!;
    this.verifyButton = host.querySelector('[data-action="verify"]')!;
    this.exportButtons = [...host.querySelectorAll<HTMLButtonElement>('[data-action^="export-"]')];
    this.slider.addEventListener('input', () => handlers.onSeek(Number(this.slider.value)));
    this.replayButton.addEventListener('click', handlers.onReplay);
    host.querySelector<HTMLButtonElement>('[data-action="export-json"]')!.addEventListener('click', () => handlers.onExport('json'));
    host.querySelector<HTMLButtonElement>('[data-action="export-binary"]')!.addEventListener('click', () => handlers.onExport('binary'));
    this.verifyButton.addEventListener('click', handlers.onVerify);
    host.querySelector<HTMLInputElement>('input[type="file"]')!.addEventListener('change', async event => {
      const input = event.currentTarget as HTMLInputElement;
      const files = [...(input.files ?? [])];
      try {
        const jsonFile = files.find(file => file.name.endsWith('.json'));
        const binaryFile = files.find(file => file.name.endsWith('.bin'));
        if (!jsonFile || !binaryFile || files.length !== 2) throw new Error('Choose one trace JSON file and its activation .bin file.');
        handlers.onImport(await importMoveTrace(await jsonFile.text(), new Uint8Array(await binaryFile.arrayBuffer())));
      } catch (error) {
        this.verification.className = 'trace-verification failed';
        this.verification.textContent = error instanceof Error ? error.message : String(error);
      } finally { input.value = ''; }
    });
  }

  reset(): void {
    this.trace = null;
    this.slider.disabled = true; this.slider.min = '0'; this.slider.max = '0'; this.slider.value = '0';
    this.replayButton.disabled = true; this.verifyButton.disabled = true;
    this.exportButtons.forEach(button => { button.disabled = true; });
    this.sourceLabel.textContent = 'No recorded trace.';
    this.timelineLabel.textContent = 'Waiting for a recorded move.';
    this.output.className = 'decision-output muted';
    this.output.textContent = 'Play a move or import a trace to inspect its decision.';
    this.verification.textContent = '';
    this.provenance.textContent = 'No trace loaded.';
  }

  showLiveFrame(frame: ActivationFrame): void {
    this.sourceLabel.textContent = `Live simulation · ${frame.backend} · step ${frame.step}`;
    this.output.className = 'decision-output';
    this.output.innerHTML = readoutHtml(frame.decision);
    this.timelineLabel.textContent = `Live sample · ${frame.tMs.toFixed(1)} ms simulation · ${frame.elapsedMs.toFixed(0)} ms wall time`;
  }

  setTrace(trace: MoveTrace, source: 'live' | 'imported'): void {
    this.trace = trace;
    this.slider.disabled = false; this.slider.min = '0'; this.slider.max = String(trace.frames.length - 1);
    this.replayButton.disabled = false; this.verifyButton.disabled = false;
    this.exportButtons.forEach(button => { button.disabled = false; });
    this.sourceLabel.textContent = `${source === 'live' ? 'Recorded live simulation' : 'Imported recorded simulation'} · ${trace.provenance.backend} · ${trace.provenance.modelBadge}`;
    const p = trace.provenance;
    this.provenance.textContent = `FEN ${p.inputFen}\nbackend ${p.backend}\nadapter ${p.adapter}\nmodel ${p.modelKind}\ncheckpoint ${p.checkpointSha256 ?? 'untrained baseline (no checkpoint)'}\ngraph manifest ${p.graphManifestSha256}\ngraph neurons ${p.graphNeuronsSha256}\nsensory map ${p.sensoryMapSha256}\noutput map ${p.outputMapSha256}`;
    this.verification.className = 'trace-verification muted';
    this.verification.textContent = 'CPU reproducibility has not been checked for this trace.';
  }

  showFrame(frame: ActivationFrame, index: number): void {
    if (!this.trace) return;
    this.slider.value = String(index);
    this.output.className = 'decision-output';
    this.output.innerHTML = readoutHtml(frame.decision);
    this.timelineLabel.textContent = `Recorded replay · sample ${index + 1}/${this.trace.frames.length} · step ${frame.step} · ${frame.tMs.toFixed(1)} ms simulation · ${frame.elapsedMs.toFixed(0)} ms original wall time`;
  }

  setReplaying(replaying: boolean): void {
    this.replayButton.textContent = replaying ? 'Stop replay' : 'Replay';
  }

  setVerifying(): void {
    this.verifyButton.disabled = true;
    this.verification.className = 'trace-verification muted';
    this.verification.textContent = 'Re-simulating the same model and FEN on the CPU reference backend…';
  }

  showVerification(result: TraceVerification): void {
    this.verifyButton.disabled = false;
    this.verification.className = `trace-verification ${result.passed ? 'passed' : 'failed'}`;
    const recordedLabel = result.mode === 'cpu-replay' ? 'Recorded CPU trace' : 'Recorded GPU trace vs CPU reference';
    this.verification.textContent = `CPU reproducibility ${result.cpuReplayPassed ? 'passed' : 'failed'} · repeated-run activation Δ ${result.maxCpuReplayActivationDifference.toExponential(2)} · policy Δ ${result.maxCpuReplayDecisionRateDifference.toExponential(2)} (tol ${result.decisionRateTolerance.toExponential(2)}). ${recordedLabel} ${result.recordedTraceMatches ? 'matched' : 'did not match'} · move ${result.actualMove} · activation Δ ${result.maxActivationDifference.toExponential(2)} · ${result.failedActivationFrameCount} frame(s) outside their own f32/quantized tolerance (largest tolerance ${result.activationTolerance.toExponential(2)}) · policy Δ ${result.maxDecisionRateDifference.toExponential(2)}. ${result.note}`;
  }

  showError(message: string): void {
    this.verifyButton.disabled = false;
    this.verification.className = 'trace-verification failed';
    this.verification.textContent = message;
  }
}
