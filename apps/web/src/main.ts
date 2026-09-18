import { Game } from './chess/game';
import { displaySquares, pieceNames } from './chess/board';
import { HumanVsFly, type MatchActor, type Side } from './chess/HumanVsFly';
import { EngineVsFly } from './chess/EngineVsFly';
import { BrainView } from './brain/BrainView';
import type { ActivationFrame } from './brain/activation';
import { InferenceLifecycle } from './engine/lifecycle';
import { ModelSelectionLifecycle, availableModel, parseModelCatalog, type ModelCatalog, type ModelId } from './engine/modelCatalog';
import type { ModelManifestInfo, StockFlyRequest, StockFlyResponse } from './engine/protocol';
import type { StockfishRequest, StockfishResponse } from './engine/stockfish.worker';
import { exportMoveTrace, MOVE_TRACE_VERSION, type MoveTrace } from './traces/MoveTrace';
import { TraceTimeline } from './traces/TraceTimeline';
import { DecisionPanel } from './ui/DecisionPanel';

const game = new Game();
const lifecycle = new InferenceLifecycle();
const modelLoads = new ModelSelectionLifecycle();
const humanMatch = new HumanVsFly('w');
const engineMatch = new EngineVsFly('b');
let matchMode: 'human' | 'engine' = 'human';
let selectedSquare: string | null = null;
let legalTargets: string[] = [];
let flyThinking = false;
let stockfishThinking = false;
let stockfishSerial = 0;
let activeStockfish: { generation: number; requestId: string } | null = null;
let lastMove: string[] = [];
let lastTrace: MoveTrace | null = null;
let timeline: TraceTimeline | null = null;
let pendingFrames: ActivationFrame[] = [];
let pendingAttempt = 0;
let pendingFen = '';
let replayTimer: number | null = null;
let verificationSerial = 0;
let activeVerificationId: string | null = null;
let modelInfo: ModelManifestInfo | null = null;
let modelCatalog: ModelCatalog | null = null;
let selectedModelId: ModelId = 'bio-full';
let modelLoading = true;
let statusText = 'Loading the trained model catalog…';
let loadFailed = false;
let moveFailed = false;
let pendingPromotion: { from: string; to: string } | null = null;
let endgameAnnounced = false;

const app = document.getElementById('app')!;
app.innerHTML = `<nav class="sidebar" aria-label="Main navigation"><a class="brand" href="#play"><span class="brand-symbol" aria-hidden="true">♞</span>Stock<span>Fly</span></a><a class="nav-link current" href="#play"><span aria-hidden="true">▦</span> Play</a><a class="nav-link" href="#brain"><span aria-hidden="true">◉</span> Brain</a><p class="sidebar-caption">Chess, through<br>a fly’s connectome.</p><a class="asset-credit" href="/pieces/README.txt">Piece credits</a></nav>
  <main id="play"><header class="page-heading"><div><p class="eyebrow">THE CONNECTOME AT PLAY</p><h1>Play StockFly</h1></div><span class="mode-label">Human vs Fly</span></header>
  <div class="workspace"><section class="board-workspace" aria-label="Chess game"><div class="player-strip" id="opponent"></div><div class="board" aria-label="Chessboard"></div><div class="player-strip" id="human"></div><p class="status game-status" role="status" aria-live="polite"></p><div class="promotion" hidden role="group" aria-label="Choose promotion"></div><section class="endgame-panel" role="dialog" aria-live="assertive" aria-labelledby="endgame-title" aria-describedby="endgame-reason" tabindex="-1" hidden><p class="eyebrow">GAME OVER</p><h2 id="endgame-title"></h2><p id="endgame-reason"></p><button id="endgame-restart" class="primary">Play again</button></section></section>
  <aside class="context"><section class="game-controls"><div class="control-heading"><h2>Your game</h2><span class="badge">Loading…</span></div><p class="muted">The fly’s neural activity chooses its move.</p><div class="model-options"><label>Trained model<select id="model" disabled><option>Loading catalog…</option></select></label><p class="model-unavailable muted"></p></div><div class="mode-options"><label>Mode<select id="mode"><option value="human">Human vs Fly</option><option value="engine">Stockfish vs Fly</option></select></label><label><span id="side-label">Play as</span><select id="side"><option value="w">White</option><option value="b">Black</option></select></label></div><div class="game-options"><button id="new-game" class="primary" disabled>Restart</button><button id="pause" class="secondary" hidden>Pause</button><button id="step" class="secondary" hidden>One ply</button></div><button id="retry" class="primary" hidden>Retry loading</button><div class="model-summary"></div><div class="moves" aria-label="Move history"><span class="muted">Moves will appear here.</span></div><a class="engine-credit" href="/vendor/stockfish/NOTICE.txt">Stockfish source and license</a></section>
  <section id="brain" class="brain-panel" aria-label="Live brain visualization"></section>
  <details class="decision-details"><summary>Decision details &amp; replay</summary><div id="decision-panel"></div></details></aside></div></main>`;
const board = app.querySelector<HTMLElement>('.board')!;
const status = app.querySelector<HTMLElement>('.game-status')!;
const modelSelect = app.querySelector<HTMLSelectElement>('#model')!;
const modeSelect = app.querySelector<HTMLSelectElement>('#mode')!;
const sideSelect = app.querySelector<HTMLSelectElement>('#side')!;
const sideLabel = app.querySelector<HTMLElement>('#side-label')!;
const newGameButton = app.querySelector<HTMLButtonElement>('#new-game')!;
const pauseButton = app.querySelector<HTMLButtonElement>('#pause')!;
const stepButton = app.querySelector<HTMLButtonElement>('#step')!;
const retryButton = app.querySelector<HTMLButtonElement>('#retry')!;
const endgamePanel = app.querySelector<HTMLElement>('.endgame-panel')!;
const endgameTitle = app.querySelector<HTMLElement>('#endgame-title')!;
const endgameReason = app.querySelector<HTMLElement>('#endgame-reason')!;
const brain = new BrainView(app.querySelector('#brain')!);
const worker = new Worker(new URL('./engine/stockfly.worker.ts', import.meta.url), { type: 'module' });
const post = (message: StockFlyRequest) => worker.postMessage(message);
const stockfishWorker = new Worker(new URL('./engine/stockfish.worker.ts', import.meta.url), { type: 'module' });
const postStockfish = (message: StockfishRequest) => stockfishWorker.postMessage(message);
const escapeHtml = (text: string): string => text.replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]!);
const decisionPanel = new DecisionPanel(app.querySelector('#decision-panel')!, {
  onSeek: seekTrace,
  onReplay: toggleReplay,
  onExport: exportTrace,
  onImport: importTrace,
  onVerify: verifyTrace,
});

const selectedSide = (): Side => matchMode === 'human' ? humanMatch.humanSide : engineMatch.flySide;
const actorForTurn = (): MatchActor | null => matchMode === 'human'
  ? humanMatch.actorFor(game.turn(), game.isGameOver())
  : engineMatch.actorFor(game.turn(), game.isGameOver());
const participantName = (side: Side): string => matchMode === 'human'
  ? humanMatch.nameFor(side)
  : engineMatch.nameFor(side);

function renderEndgame(): string | null {
  const ending = game.termination();
  endgamePanel.hidden = !ending;
  if (!ending) { endgameAnnounced = false; return null; }
  if (ending.outcome === 'win') {
    const winner = participantName(ending.winner);
    const loser = participantName(ending.winner === 'w' ? 'b' : 'w');
    endgameTitle.textContent = `${winner} wins`;
    endgameReason.textContent = `Checkmate. ${loser} has no legal move.`;
    if (!endgameAnnounced) { endgameAnnounced = true; requestAnimationFrame(() => endgamePanel.focus({ preventScroll: true })); }
    return `${winner} wins by checkmate.`;
  }
  endgameTitle.textContent = 'Draw';
  const reason = ending.reason[0].toUpperCase() + ending.reason.slice(1);
  endgameReason.textContent = `${reason}.`;
  if (!endgameAnnounced) { endgameAnnounced = true; requestAnimationFrame(() => endgamePanel.focus({ preventScroll: true })); }
  return `Draw by ${ending.reason}.`;
}

function render(): void {
  const focus = document.activeElement instanceof HTMLElement ? document.activeElement.dataset.square : undefined;
  const orientation = selectedSide();
  const actor = actorForTurn();
  board.innerHTML = displaySquares(game.board(), orientation).map(({ square, piece, light, row, column }) => {
    const classes = ['square', light ? 'light' : 'dark'];
    if (square === selectedSquare) classes.push('selected');
    if (legalTargets.includes(square)) classes.push('legal-target');
    if (lastMove.includes(square)) classes.push('last-move');
    const description = piece ? `${piece.color === 'w' ? 'white' : 'black'} ${pieceNames[piece.type]}` : 'empty';
    return `<button type="button" class="${classes.join(' ')}" data-square="${square}" aria-label="${square}, ${description}${legalTargets.includes(square) ? ', legal move' : ''}" aria-pressed="${square === selectedSquare}" ${!modelInfo || actor !== 'human' || flyThinking || stockfishThinking || pendingPromotion || game.isGameOver() ? 'disabled' : ''}>${piece ? `<img src="/pieces/${piece.color}${piece.type.toUpperCase()}.svg" alt="" draggable="false">` : ''}${column === 0 ? `<span class="rank-coordinate" aria-hidden="true">${square[1]}</span>` : ''}${row === 7 ? `<span class="file-coordinate" aria-hidden="true">${square[0]}</span>` : ''}</button>`;
  }).join('');
  if (focus) board.querySelector<HTMLButtonElement>(`[data-square="${focus}"]`)?.focus({ preventScroll: true });
  status.textContent = renderEndgame() ?? statusText;
  if (matchMode === 'human') {
    const humanSide = humanMatch.humanSide;
    app.querySelector('#opponent')!.innerHTML = `<div class="avatar fly-avatar" aria-hidden="true">◉</div><div><strong>StockFly</strong><span>${modelInfo ? escapeHtml(modelInfo.modelLabel) : 'Loading model…'}</span></div><span class="player-turn">${flyThinking ? 'Thinking…' : humanSide === 'w' ? 'Black' : 'White'}</span>`;
    app.querySelector('#human')!.innerHTML = `<div class="avatar human-avatar" aria-hidden="true">${humanSide === 'w' ? 'W' : 'B'}</div><div><strong>You</strong><span>${humanSide === 'w' ? 'White pieces' : 'Black pieces'}</span></div><span class="player-turn">${modelInfo && actor === 'human' ? 'Your move' : ''}</span>`;
  } else {
    app.querySelector('#opponent')!.innerHTML = `<div class="avatar engine-avatar" aria-hidden="true">S</div><div><strong>Stockfish 19 Lite</strong><span>Official isolated worker</span></div><span class="player-turn">${stockfishThinking ? 'Thinking…' : engineMatch.flySide === 'w' ? 'Black' : 'White'}</span>`;
    app.querySelector('#human')!.innerHTML = `<div class="avatar fly-avatar" aria-hidden="true">◉</div><div><strong>StockFly</strong><span>${modelInfo ? escapeHtml(modelInfo.modelLabel) : 'Loading model…'}</span></div><span class="player-turn">${flyThinking ? 'Thinking…' : engineMatch.flySide === 'w' ? 'White' : 'Black'}</span>`;
  }
  app.querySelector('.mode-label')!.textContent = matchMode === 'human' ? 'Human vs Fly' : 'Stockfish vs Fly';
  modeSelect.value = matchMode;
  sideLabel.textContent = matchMode === 'human' ? 'Play as' : 'StockFly side';
  sideSelect.value = selectedSide();
  app.querySelector('.badge')!.textContent = modelInfo?.modelBadge ?? (loadFailed ? 'Unavailable' : 'Loading…');
  newGameButton.disabled = !modelInfo;
  sideSelect.disabled = !modelInfo;
  modeSelect.disabled = !modelInfo;
  pauseButton.hidden = matchMode !== 'engine';
  stepButton.hidden = matchMode !== 'engine';
  pauseButton.textContent = engineMatch.isPaused ? 'Resume' : 'Pause';
  pauseButton.disabled = !modelInfo;
  stepButton.disabled = !modelInfo || flyThinking || stockfishThinking;
  modelSelect.disabled = !modelCatalog || modelLoading;
  if (modelCatalog) modelSelect.value = selectedModelId;
  retryButton.hidden = !loadFailed && !moveFailed;
  retryButton.textContent = loadFailed ? 'Retry loading' : 'Retry move';
  const summary = app.querySelector('.model-summary')!;
  summary.innerHTML = modelInfo ? `<span class="backend-dot" aria-hidden="true"></span><strong>${escapeHtml(modelInfo.backend)}</strong><span>${modelInfo.neuronCount.toLocaleString()} neurons · ${(modelInfo.edgeCount / 1e6).toFixed(1)}M edges</span><span class="model-training">${escapeHtml(modelInfo.modelLabel)} · ${escapeHtml(modelInfo.trainingPreset)} preset · ${modelInfo.trialsRun.toLocaleString()} trials</span><small>${escapeHtml(modelInfo.adapter)}${modelInfo.fallbackReason ? `<br>CPU fallback: ${escapeHtml(modelInfo.fallbackReason)}` : ''}<br>checkpoint SHA-256 <code>${escapeHtml(modelInfo.checkpointSha256 ?? '')}</code><br>catalog asset <code>${escapeHtml(modelInfo.checkpointUrl)}</code></small>` : `<span class="muted">${loadFailed ? 'Model could not be loaded.' : 'Loading graph, learned weights and maps…'}</span>`;
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

async function exportTrace(kind: 'json' | 'binary'): Promise<void> {
  if (!lastTrace) return;
  try {
    const exported = await exportMoveTrace(lastTrace);
    const base = `stockfly-${lastTrace.traceId.replace(/[^a-z0-9_-]/gi, '-')}`;
    if (kind === 'json') download(`${base}.json`, exported.json, 'application/json');
    else download(`${base}.activations.bin`, exported.binary.slice().buffer, 'application/octet-stream');
  } catch (error) {
    decisionPanel.showError(`Trace export failed: ${error instanceof Error ? error.message : String(error)}`);
  }
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
  if (!modelInfo || flyThinking || stockfishThinking || actorForTurn() !== 'stockfly') return;
  flyThinking = true; moveFailed = false;
  stopReplay();
  brain.reset();
  decisionPanel.reset();
  lastTrace = null; timeline = null; pendingFrames = []; pendingAttempt = 0;
  statusText = 'StockFly is thinking. Watching actual neural activity…';
  const identity = lifecycle.begin();
  pendingFen = game.fen();
  render();
  post({ type: 'position', fen: pendingFen, ...identity, settleSteps: 16 });
}

function askStockfishToMove(): void {
  if (!modelInfo || flyThinking || stockfishThinking || actorForTurn() !== 'stockfish') return;
  stockfishThinking = true;
  moveFailed = false;
  const requestId = `${lifecycle.generation}:stockfish:${++stockfishSerial}`;
  activeStockfish = { generation: lifecycle.generation, requestId };
  statusText = 'Stockfish 19 Lite is thinking…';
  render();
  postStockfish({ type: 'search', fen: game.fen(), generation: lifecycle.generation, requestId });
}

function advanceMatch(): void {
  if (actorForTurn() === 'stockfly') askFlyToMove();
  else if (actorForTurn() === 'stockfish') askStockfishToMove();
}

function completeEnginePly(): void {
  if (matchMode === 'engine') engineMatch.completePly();
}

function playHumanMove(from: string, to: string, promotion?: string): void {
  if (actorForTurn() !== 'human') return;
  if (!game.applyMove(from, to, promotion)) return;
  lastMove = [from, to]; selectedSquare = null; legalTargets = [];
  pendingPromotion = null;
  app.querySelector<HTMLElement>('.promotion')!.hidden = true;
  statusText = 'Your move played.';
  render(); advanceMatch();
}

function onSquareClick(square: string): void {
  if (!modelInfo || flyThinking || stockfishThinking || pendingPromotion || actorForTurn() !== 'human') return;
  if (selectedSquare && legalTargets.includes(square)) {
    const promotions = game.promotions(selectedSquare, square);
    if (promotions.length) {
      pendingPromotion = { from: selectedSquare, to: square };
      const chooser = app.querySelector<HTMLElement>('.promotion')!;
      chooser.hidden = false;
      chooser.innerHTML = `<strong>Promote to</strong>${promotions.map(piece => `<button data-promotion="${piece}" aria-label="Promote to ${pieceNames[piece]}"><img src="/pieces/${humanMatch.humanSide}${piece.toUpperCase()}.svg" alt="">${pieceNames[piece]}</button>`).join('')}<button data-promotion="cancel">Cancel</button>`;
      render(); chooser.querySelector('button')!.focus();
    } else playHumanMove(selectedSquare, square);
  } else {
    const targets = square === selectedSquare ? [] : game.legalMovesFrom(square);
    selectedSquare = targets.length ? square : null; legalTargets = targets;
    render();
  }
}

function clearGameState(): void {
  flyThinking = false; stockfishThinking = false; activeStockfish = null; moveFailed = false; game.reset();
  engineMatch.restart();
  selectedSquare = null; legalTargets = []; lastMove = []; lastTrace = null; timeline = null; pendingFrames = []; pendingAttempt = 0; pendingPromotion = null;
  activeVerificationId = null; stopReplay();
  app.querySelector<HTMLElement>('.promotion')!.hidden = true;
  brain.reset(); decisionPanel.reset();
}

function resetGame(): void {
  const generation = lifecycle.reset();
  post({ type: 'reset', generation });
  postStockfish({ type: 'reset', generation });
  clearGameState();
  statusText = matchMode === 'human' ? 'New game. Your move.' : 'Exhibition restarted.';
  render(); advanceMatch();
}

function populateModelOptions(): void {
  if (!modelCatalog) return;
  modelSelect.innerHTML = modelCatalog.models.map(model => {
    const suffix = model.availability === 'available'
      ? `${model.trainingPreset} · ${model.trialsRun.toLocaleString()} trials`
      : 'unavailable';
    return `<option value="${model.id}" ${model.availability === 'unavailable' ? 'disabled' : ''}>${escapeHtml(model.label)} — ${escapeHtml(suffix)}</option>`;
  }).join('');
  const lite = modelCatalog.models.find(model => model.id === 'lite');
  app.querySelector<HTMLElement>('.model-unavailable')!.textContent = lite?.availability === 'unavailable'
    ? `Lite unavailable: ${lite.reason}`
    : '';
}

function loadModel(modelId: ModelId): void {
  if (!modelCatalog) return;
  const selected = availableModel(modelCatalog, modelId);
  selectedModelId = modelId;
  const generation = lifecycle.reset();
  modelLoads.begin({ generation, modelId });
  post({ type: 'reset', generation });
  postStockfish({ type: 'reset', generation });
  clearGameState();
  modelInfo = null;
  modelLoading = true;
  loadFailed = false;
  statusText = `Loading ${selected.label} from its verified checkpoint…`;
  render();
  post({ type: 'load', generation, modelId });
}

async function loadCatalog(): Promise<void> {
  modelLoading = true;
  loadFailed = false;
  statusText = 'Loading the trained model catalog…';
  render();
  try {
    const response = await fetch('/vendor/models/catalog.json');
    if (!response.ok) throw new Error(`Model catalog HTTP ${response.status}`);
    modelCatalog = parseModelCatalog(await response.json());
    populateModelOptions();
    loadModel(selectedModelId);
  } catch (error) {
    modelCatalog = null;
    modelLoading = false;
    loadFailed = true;
    statusText = `Model catalog failed: ${error instanceof Error ? error.message : String(error)}`;
    render();
  }
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
app.querySelector<HTMLButtonElement>('#endgame-restart')!.onclick = resetGame;
modelSelect.onchange = () => loadModel(modelSelect.value as ModelId);
modeSelect.onchange = () => {
  matchMode = modeSelect.value as 'human' | 'engine';
  if (matchMode === 'engine') engineMatch.resume();
  resetGame();
};
sideSelect.onchange = () => {
  const side = sideSelect.value as Side;
  if (matchMode === 'human') humanMatch.setHumanSide(side);
  else engineMatch.setFlySide(side);
  resetGame();
};
pauseButton.onclick = () => {
  if (engineMatch.isPaused) { engineMatch.resume(); statusText = 'Exhibition resumed.'; }
  else { engineMatch.pause(); statusText = flyThinking || stockfishThinking ? 'Pausing after the current move…' : 'Exhibition paused.'; }
  render(); advanceMatch();
};
stepButton.onclick = () => {
  engineMatch.step();
  statusText = 'Playing one ply.';
  render(); advanceMatch();
};
retryButton.onclick = () => {
  if (loadFailed) {
    if (modelCatalog) loadModel(selectedModelId);
    else void loadCatalog();
  }
  else advanceMatch();
};

worker.addEventListener('message', (event: MessageEvent<StockFlyResponse>) => {
  const msg = event.data;
  if (msg.generation !== lifecycle.generation) return;
  if (msg.type === 'loaded') {
    if (!modelLoads.finish({ generation: msg.generation, modelId: msg.modelId })) return;
    modelInfo = msg.manifest; loadFailed = false;
    modelLoading = false;
    statusText = matchMode === 'human' ? `${modelInfo.modelLabel} ready. Select a piece to begin.` : `${modelInfo.modelLabel} exhibition ready.`;
    render(); advanceMatch();
  } else if (msg.type === 'frame-restart') {
    if (!lifecycle.accepts(msg) || msg.attempt <= pendingAttempt) return;
    pendingAttempt = msg.attempt;
    pendingFrames = [];
    brain.reset(); decisionPanel.reset();
    statusText = `Simulation restarted on ${msg.backend}; earlier backend samples were discarded.`;
    render();
  } else if (msg.type === 'frame') {
    if (!lifecycle.accepts(msg)) return;
    if (msg.attempt !== pendingAttempt) return;
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
        format: 'stockfly-move-trace', version: MOVE_TRACE_VERSION, traceId: msg.traceId,
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
      if (san) {
        lastMove = [msg.selectedMove.slice(0, 2), msg.selectedMove.slice(2, 4)];
        statusText = `StockFly played ${san}.`;
        completeEnginePly();
      }
      else { statusText = `The engine returned an illegal move: ${msg.selectedMove}.`; moveFailed = true; }
      render();
      if (san) advanceMatch();
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
    if (msg.modelId && !modelLoads.finish({ generation: msg.generation, modelId: msg.modelId })) return;
    statusText = `Error: ${msg.message}`; flyThinking = false;
    if (msg.traceId) { moveFailed = true; lifecycle.finish(); }
    else { loadFailed = true; modelLoading = false; }
    render();
  }
});
worker.addEventListener('error', event => {
  statusText = `Worker failed: ${event.message}. Reload the page to restart.`;
  flyThinking = false; modelLoading = false; loadFailed = true; modelLoads.cancel(); lifecycle.reset(); modelInfo = null; render();
});
stockfishWorker.addEventListener('message', (event: MessageEvent<StockfishResponse>) => {
  const msg = event.data;
  if (!activeStockfish || msg.generation !== lifecycle.generation
      || msg.generation !== activeStockfish.generation
      || msg.requestId !== activeStockfish.requestId) return;
  if (msg.type === 'error') {
    activeStockfish = null; stockfishThinking = false; moveFailed = true;
    statusText = `Stockfish error: ${msg.message}`;
    render();
    return;
  }
  activeStockfish = null; stockfishThinking = false;
  const san = game.applyUci(msg.uci);
  if (!san) {
    moveFailed = true;
    statusText = `Stockfish returned an illegal move: ${msg.uci}.`;
    render();
    return;
  }
  lastMove = [msg.uci.slice(0, 2), msg.uci.slice(2, 4)];
  statusText = `Stockfish played ${san}.`;
  completeEnginePly();
  render(); advanceMatch();
});
stockfishWorker.addEventListener('error', event => {
  if (!activeStockfish || activeStockfish.generation !== lifecycle.generation) return;
  activeStockfish = null; stockfishThinking = false; moveFailed = true;
  statusText = `Stockfish worker failed: ${event.message}`;
  render();
});
window.addEventListener('pagehide', event => {
  if (event.persisted) return; // Back/forward cache retains the live document.
  lifecycle.reset(); worker.terminate(); stockfishWorker.terminate(); brain.dispose();
});
render();
void loadCatalog();
