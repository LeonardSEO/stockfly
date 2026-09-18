import { Game } from './chess/game';
import { displaySquares, pieceNames } from './chess/board';
import { BrainView } from './brain/BrainView';
import type { ActivationFrame } from './brain/activation';
import { InferenceLifecycle } from './engine/lifecycle';
import type { ModelManifestInfo, StockFlyRequest, StockFlyResponse } from './engine/protocol';
import { exportMoveTrace, type MoveTrace } from './traces/MoveTrace';
import { TraceTimeline } from './traces/TraceTimeline';
import { DecisionPanel } from './ui/DecisionPanel';

const game = new Game();
const lifecycle = new InferenceLifecycle();
let humanSide: 'w' | 'b' = 'w';
let selectedSquare: string | null = null;
let legalTargets: string[] = [];
let flyThinking = false;
let lastMove: string[] = [];
let lastTrace: MoveTrace | null = null;
let timeline: TraceTimeline | null = null;
let pendingFrames: ActivationFrame[] = [];
let pendingFen = '';
let replayTimer: number | null = null;
let verificationSerial = 0;
let activeVerificationId: string | null = null;
let modelInfo: ModelManifestInfo | null = null;
let statusText = 'Loading StockFly’s brain…';
let loadFailed = false;
let moveFailed = false;
let pendingPromotion: { from: string; to: string } | null = null;

const app = document.getElementById('app')!;
app.innerHTML = `<nav class="sidebar" aria-label="Main navigation"><a class="brand" href="#play"><span class="brand-symbol" aria-hidden="true">♞</span>Stock<span>Fly</span></a><a class="nav-link current" href="#play"><span aria-hidden="true">▦</span> Play</a><a class="nav-link" href="#brain"><span aria-hidden="true">◉</span> Brain</a><p class="sidebar-caption">Chess, through<br>a fly’s connectome.</p><a class="asset-credit" href="/pieces/README.txt">Piece credits</a></nav>
  <main id="play"><header class="page-heading"><div><p class="eyebrow">THE CONNECTOME AT PLAY</p><h1>Play StockFly</h1></div><span class="mode-label">Human vs fly</span></header>
  <div class="workspace"><section class="board-workspace" aria-label="Chess game"><div class="player-strip" id="opponent"></div><div class="board" aria-label="Chessboard"></div><div class="player-strip" id="human"></div><p class="status game-status" role="status" aria-live="polite"></p><div class="promotion" hidden role="group" aria-label="Choose promotion"></div></section>
  <aside class="context"><section class="game-controls"><div class="control-heading"><h2>Your game</h2><span class="badge">Loading…</span></div><p class="muted">The fly’s neural activity chooses its move.</p><div class="game-options"><label>Play as<select id="side"><option value="w">White</option><option value="b">Black</option></select></label><button id="new-game" class="primary" disabled>New game</button></div><button id="retry" class="primary" hidden>Retry loading</button><div class="model-summary"></div><div class="moves" aria-label="Move history"><span class="muted">Moves will appear here.</span></div></section>
  <section id="brain" class="brain-panel" aria-label="Live brain visualization"></section>
  <details class="decision-details"><summary>Decision details &amp; replay</summary><div id="decision-panel"></div></details></aside></div></main>`;
const board = app.querySelector<HTMLElement>('.board')!;
const status = app.querySelector<HTMLElement>('.game-status')!;
const sideSelect = app.querySelector<HTMLSelectElement>('#side')!;
const newGameButton = app.querySelector<HTMLButtonElement>('#new-game')!;
const retryButton = app.querySelector<HTMLButtonElement>('#retry')!;
const brain = new BrainView(app.querySelector('#brain')!);
const worker = new Worker(new URL('./engine/stockfly.worker.ts', import.meta.url), { type: 'module' });
const post = (message: StockFlyRequest) => worker.postMessage(message);
const escapeHtml = (text: string): string => text.replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]!);
const decisionPanel = new DecisionPanel(app.querySelector('#decision-panel')!, {
  onSeek: seekTrace,
  onReplay: toggleReplay,
  onExport: exportTrace,
  onImport: importTrace,
  onVerify: verifyTrace,
});

function render(): void {
  const focus = document.activeElement instanceof HTMLElement ? document.activeElement.dataset.square : undefined;
  board.innerHTML = displaySquares(game.board(), humanSide).map(({ square, piece, light, row, column }) => {
    const classes = ['square', light ? 'light' : 'dark'];
    if (square === selectedSquare) classes.push('selected');
    if (legalTargets.includes(square)) classes.push('legal-target');
    if (lastMove.includes(square)) classes.push('last-move');
    const description = piece ? `${piece.color === 'w' ? 'white' : 'black'} ${pieceNames[piece.type]}` : 'empty';
    return `<button type="button" class="${classes.join(' ')}" data-square="${square}" aria-label="${square}, ${description}${legalTargets.includes(square) ? ', legal move' : ''}" aria-pressed="${square === selectedSquare}" ${!modelInfo || flyThinking || pendingPromotion || game.isGameOver() ? 'disabled' : ''}>${piece ? `<img src="/pieces/${piece.color}${piece.type.toUpperCase()}.svg" alt="" draggable="false">` : ''}${column === 0 ? `<span class="rank-coordinate" aria-hidden="true">${square[1]}</span>` : ''}${row === 7 ? `<span class="file-coordinate" aria-hidden="true">${square[0]}</span>` : ''}</button>`;
  }).join('');
  if (focus) board.querySelector<HTMLButtonElement>(`[data-square="${focus}"]`)?.focus({ preventScroll: true });
  status.textContent = game.result() ?? statusText;
  app.querySelector('#opponent')!.innerHTML = `<div class="avatar fly-avatar" aria-hidden="true">◉</div><div><strong>StockFly</strong><span>${modelInfo ? escapeHtml(modelInfo.modelLabel) : 'Loading model…'}</span></div><span class="player-turn">${flyThinking ? 'Thinking…' : humanSide === 'w' ? 'Black' : 'White'}</span>`;
  app.querySelector('#human')!.innerHTML = `<div class="avatar human-avatar" aria-hidden="true">${humanSide === 'w' ? 'W' : 'B'}</div><div><strong>You</strong><span>${humanSide === 'w' ? 'White pieces' : 'Black pieces'}</span></div><span class="player-turn">${modelInfo && !flyThinking && game.turn() === humanSide && !game.isGameOver() ? 'Your move' : ''}</span>`;
  app.querySelector('.badge')!.textContent = modelInfo?.modelBadge ?? (loadFailed ? 'Unavailable' : 'Loading…');
  newGameButton.disabled = !modelInfo;
  sideSelect.disabled = !modelInfo;
  retryButton.hidden = !loadFailed && !moveFailed;
  retryButton.textContent = loadFailed ? 'Retry loading' : 'Retry fly move';
  const summary = app.querySelector('.model-summary')!;
  summary.innerHTML = modelInfo ? `<span class="backend-dot" aria-hidden="true"></span><strong>${escapeHtml(modelInfo.backend)}</strong><span>${modelInfo.neuronCount.toLocaleString()} neurons · ${(modelInfo.edgeCount / 1e6).toFixed(1)}M edges</span><small>${escapeHtml(modelInfo.adapter)}${modelInfo.fallbackReason ? `<br>CPU fallback: ${escapeHtml(modelInfo.fallbackReason)}` : ''}</small>` : `<span class="muted">${loadFailed ? 'Model could not be loaded.' : 'Loading graph, learned weights and maps…'}</span>`;
  const history = game.history();
  app.querySelector('.moves')!.innerHTML = history.length ? history.map((move, index) => `${index % 2 === 0 ? `<span class="move-number">${Math.floor(index / 2) + 1}.</span>` : ''}<span>${escapeHtml(move)}</span>`).join('') : '<span class="muted">Moves will appear here.</span>';
}

function stopReplay(): void {
  if (replayTimer !== null) window.clearTimeout(replayTimer);
  replayTimer = null;
  decisionPanel.setReplaying(false);
}

function seekTrace(index: number): void {
  stopReplay();
  timeline?.seek(index);
}

function toggleReplay(): void {
  if (!timeline) return;
  if (replayTimer !== null) { stopReplay(); return; }
  decisionPanel.setReplaying(true);
  timeline.seek(0);
  const advance = () => {
    if (!timeline || timeline.currentIndex >= timeline.length - 1) { stopReplay(); return; }
    const delay = Math.max(40, Math.min(500, timeline.nextDelayMs()));
    replayTimer = window.setTimeout(() => { timeline!.seek(timeline!.currentIndex + 1); advance(); }, delay);
  };
  advance();
}

function download(name: string, contents: BlobPart, type: string): void {
  const url = URL.createObjectURL(new Blob([contents], { type }));
  const anchor = document.createElement('a');
  anchor.href = url; anchor.download = name; anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

function exportTrace(kind: 'json' | 'binary'): void {
  if (!lastTrace) return;
  const exported = exportMoveTrace(lastTrace);
  const base = `stockfly-${lastTrace.traceId.replace(/[^a-z0-9_-]/gi, '-')}`;
  if (kind === 'json') download(`${base}.json`, exported.json, 'application/json');
  else download(`${base}.activations.bin`, exported.binary.slice().buffer, 'application/octet-stream');
}

function useTrace(trace: MoveTrace, source: 'live' | 'imported'): void {
  stopReplay();
  lastTrace = trace;
  timeline = new TraceTimeline(trace, (frame, index) => { brain.update(frame); decisionPanel.showFrame(frame, index); });
  decisionPanel.setTrace(trace, source);
  timeline.seek(trace.frames.length - 1);
}

function importTrace(trace: MoveTrace): void {
  activeVerificationId = null;
  useTrace(trace, 'imported');
  statusText = `Imported recorded trace ${trace.traceId}. The board was not changed.`;
  render();
}

function verifyTrace(): void {
  if (!lastTrace) return;
  const verificationId = `verify:${++verificationSerial}`;
  activeVerificationId = verificationId;
  decisionPanel.setVerifying();
  post({ type: 'verify-trace', generation: lifecycle.generation, verificationId, trace: lastTrace });
}

function askFlyToMove(): void {
  if (!modelInfo || game.isGameOver() || game.turn() === humanSide) return;
  flyThinking = true; moveFailed = false;
  stopReplay();
  brain.reset();
  decisionPanel.reset();
  lastTrace = null; timeline = null; pendingFrames = [];
  statusText = 'StockFly is thinking. Watching actual neural activity…';
  const identity = lifecycle.begin();
  pendingFen = game.fen();
  render();
  post({ type: 'position', fen: pendingFen, ...identity, settleSteps: 16 });
}

function playHumanMove(from: string, to: string, promotion?: string): void {
  if (!game.applyMove(from, to, promotion)) return;
  lastMove = [from, to]; selectedSquare = null; legalTargets = [];
  pendingPromotion = null;
  app.querySelector<HTMLElement>('.promotion')!.hidden = true;
  statusText = 'Your move played.';
  render(); askFlyToMove();
}

function onSquareClick(square: string): void {
  if (!modelInfo || flyThinking || pendingPromotion || game.isGameOver() || game.turn() !== humanSide) return;
  if (selectedSquare && legalTargets.includes(square)) {
    const promotions = game.promotions(selectedSquare, square);
    if (promotions.length) {
      pendingPromotion = { from: selectedSquare, to: square };
      const chooser = app.querySelector<HTMLElement>('.promotion')!;
      chooser.hidden = false;
      chooser.innerHTML = `<strong>Promote to</strong>${promotions.map(piece => `<button data-promotion="${piece}" aria-label="Promote to ${pieceNames[piece]}"><img src="/pieces/${humanSide}${piece.toUpperCase()}.svg" alt="">${pieceNames[piece]}</button>`).join('')}<button data-promotion="cancel">Cancel</button>`;
      render(); chooser.querySelector('button')!.focus();
    } else playHumanMove(selectedSquare, square);
  } else {
    const targets = square === selectedSquare ? [] : game.legalMovesFrom(square);
    selectedSquare = targets.length ? square : null; legalTargets = targets;
    render();
  }
}

function resetGame(): void {
  const generation = lifecycle.reset();
  post({ type: 'reset', generation });
  flyThinking = false; moveFailed = false; game.reset();
  selectedSquare = null; legalTargets = []; lastMove = []; lastTrace = null; timeline = null; pendingFrames = []; pendingPromotion = null;
  activeVerificationId = null; stopReplay();
  app.querySelector<HTMLElement>('.promotion')!.hidden = true;
  brain.reset(); decisionPanel.reset(); statusText = 'New game. Your move.';
  render(); askFlyToMove();
}

board.addEventListener('click', event => {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-square]');
  if (button) onSquareClick(button.dataset.square!);
});
app.querySelector('.promotion')!.addEventListener('click', event => {
  const choice = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-promotion]')?.dataset.promotion;
  if (!choice || !pendingPromotion) return;
  if (choice === 'cancel') { pendingPromotion = null; app.querySelector<HTMLElement>('.promotion')!.hidden = true; render(); }
  else playHumanMove(pendingPromotion.from, pendingPromotion.to, choice);
});
newGameButton.onclick = resetGame;
sideSelect.onchange = () => { humanSide = sideSelect.value as 'w' | 'b'; resetGame(); };
retryButton.onclick = () => {
  if (loadFailed) { loadFailed = false; statusText = 'Loading StockFly’s brain…'; render(); post({ type: 'load', generation: lifecycle.generation }); }
  else askFlyToMove();
};

worker.addEventListener('message', (event: MessageEvent<StockFlyResponse>) => {
  const msg = event.data;
  if (msg.generation !== lifecycle.generation) return;
  if (msg.type === 'loaded') {
    modelInfo = msg.manifest; loadFailed = false;
    statusText = 'Your move. Select a piece to begin.'; render();
  } else if (msg.type === 'frame') {
    if (!lifecycle.accepts(msg)) return;
    pendingFrames.push(msg.frame);
    brain.update(msg.frame);
    decisionPanel.showLiveFrame(msg.frame);
  } else if (msg.type === 'decision') {
    if (!lifecycle.accepts(msg)) return;
    // Let the browser actually paint the final activation before the move.
    // Re-check identity after yielding: a new game may have invalidated it.
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (!lifecycle.accepts(msg)) return;
      const trace: MoveTrace = {
        format: 'stockfly-move-trace', version: 1, traceId: msg.traceId,
        provenance: {
          inputFen: pendingFen, backend: msg.backend, adapter: msg.adapter,
          ...(msg.fallbackReason ? { fallbackReason: msg.fallbackReason } : {}),
          modelKind: msg.modelLabel, modelBadge: modelInfo!.modelBadge,
          checkpointSha256: msg.checkpointSha256,
          graphManifestSha256: msg.graphManifestSha256,
          graphNeuronsSha256: msg.graphNeuronsSha256,
          sensoryMapSha256: msg.sensoryMapSha256,
          outputMapSha256: msg.outputMapSha256,
        },
        frames: pendingFrames,
        finalDecision: {
          selectedMove: msg.selectedMove,
          fromRates: msg.fromRates,
          toRates: msg.toRates,
          promotionRates: msg.promotionRates,
          legalScores: msg.legalScores,
          settleSteps: msg.settleSteps,
        },
      };
      useTrace(trace, 'live');
      flyThinking = false; lifecycle.finish();
      modelInfo = { ...modelInfo!, backend: msg.backend, adapter: msg.adapter, fallbackReason: msg.fallbackReason, modelLabel: msg.modelLabel };
      const san = game.applyUci(msg.selectedMove);
      if (san) { lastMove = [msg.selectedMove.slice(0, 2), msg.selectedMove.slice(2, 4)]; statusText = `StockFly played ${san}. Your move.`; }
      else { statusText = `The engine returned an illegal move: ${msg.selectedMove}.`; moveFailed = true; }
      render();
    }));
  } else if (msg.type === 'verification') {
    if (msg.verificationId !== activeVerificationId) return;
    activeVerificationId = null;
    decisionPanel.showVerification(msg.result);
  } else {
    if (msg.traceId && !lifecycle.accepts({ generation: msg.generation, traceId: msg.traceId })) return;
    if (msg.verificationId) {
      if (msg.verificationId === activeVerificationId) { activeVerificationId = null; decisionPanel.showError(`CPU verification failed: ${msg.message}`); }
      return;
    }
    statusText = `Error: ${msg.message}`; flyThinking = false;
    if (msg.traceId) { moveFailed = true; lifecycle.finish(); } else loadFailed = true;
    render();
  }
});
worker.addEventListener('error', event => {
  statusText = `Worker failed: ${event.message}. Reload the page to restart.`;
  flyThinking = false; lifecycle.reset(); modelInfo = null; render();
});
window.addEventListener('pagehide', event => {
  if (event.persisted) return; // Back/forward cache retains the live document.
  lifecycle.reset(); worker.terminate(); brain.dispose();
});
render();
post({ type: 'load', generation: lifecycle.generation });
