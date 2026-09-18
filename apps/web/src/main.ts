import { Game } from "./chess/game";
import type { StockFlyResponse } from "./engine/protocol";

const PIECE_UNICODE: Record<string, string> = {
  p: "♟", n: "♞", b: "♝", r: "♜", q: "♛", k: "♚",
  P: "♙", N: "♘", B: "♗", R: "♖", Q: "♕", K: "♔",
};

const FILES = ["a", "b", "c", "d", "e", "f", "g", "h"];

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

const game = new Game();
let selectedSquare: string | null = null;
let legalTargets: string[] = [];
let flyThinking = false;
let lastDecision: Extract<StockFlyResponse, { type: "decision" }> | null = null;
let modelInfo: Extract<StockFlyResponse, { type: "loaded" }>["manifest"] | null = null;
let statusText = "Loading StockFly's brain...";

const worker = new Worker(new URL("./engine/stockfly.worker.ts", import.meta.url), { type: "module" });

worker.addEventListener("message", (event: MessageEvent<StockFlyResponse>) => {
  const msg = event.data;
  if (msg.type === "loaded") {
    modelInfo = msg.manifest;
    statusText = msg.manifest.fallbackReason
      ? `Your move (CPU fallback: ${msg.manifest.fallbackReason}).`
      : `Your move (${msg.manifest.backend} on ${msg.manifest.adapter}).`;
  } else if (msg.type === "decision") {
    lastDecision = msg;
    flyThinking = false;
    const san = game.applyUci(msg.selectedMove);
    statusText = san ? `StockFly played ${san}.` : `StockFly proposed ${msg.selectedMove} (illegal?!).`;
  } else if (msg.type === "error") {
    statusText = `Error: ${msg.message}`;
    flyThinking = false;
  }
  render();
});

worker.postMessage({ type: "load" });

function onSquareClick(square: string) {
  if (flyThinking || game.isGameOver() || game.turn() !== "w") return;

  if (selectedSquare === null) {
    const targets = game.legalMovesFrom(square);
    if (targets.length > 0) {
      selectedSquare = square;
      legalTargets = targets;
    }
  } else if (square === selectedSquare) {
    selectedSquare = null;
    legalTargets = [];
  } else if (legalTargets.includes(square)) {
    game.applyMove(selectedSquare, square);
    selectedSquare = null;
    legalTargets = [];
    askFlyToMove();
  } else {
    const targets = game.legalMovesFrom(square);
    selectedSquare = targets.length > 0 ? square : null;
    legalTargets = targets;
  }
  render();
}

function askFlyToMove() {
  if (game.isGameOver()) return;
  flyThinking = true;
  statusText = "StockFly is thinking (settling its neural activity)...";
  render();
  worker.postMessage({ type: "position", fen: game.fen(), traceId: String(Date.now()), settleSteps: 16 });
}

function renderBoard(): string {
  const board = game.board();
  let html = "";
  for (let rank = 7; rank >= 0; rank--) {
    for (let file = 0; file < 8; file++) {
      const square = `${FILES[file]}${rank + 1}`;
      const piece = board[rank][file];
      const isLight = (rank + file) % 2 === 1;
      const classes = ["square", isLight ? "light" : "dark"];
      if (square === selectedSquare) classes.push("selected");
      if (legalTargets.includes(square)) classes.push("legal-target");
      const symbol = piece ? PIECE_UNICODE[piece.color === "w" ? piece.type.toUpperCase() : piece.type] : "";
      html += `<div class="${classes.join(" ")}" data-square="${square}">${symbol}</div>`;
    }
  }
  return html;
}

function renderBars(rates: number[] | undefined, labels: string[]): string {
  if (!rates) return "";
  const max = Math.max(1e-6, ...rates);
  return rates
    .map((r, i) => {
      const pct = Math.max(0, Math.min(100, (r / max) * 100));
      return `<div class="bar-row"><span>${labels[i]}</span><div class="bar-track"><div class="bar-fill" style="width:${pct}%"></div></div></div>`;
    })
    .join("");
}

function render() {
  const app = document.getElementById("app")!;
  const squareLabels = [];
  for (let rank = 0; rank < 8; rank++) for (let file = 0; file < 8; file++) squareLabels.push(`${FILES[file]}${rank + 1}`);

  app.innerHTML = `
    <h1>StockFly <span class="badge">${modelInfo ? escapeHtml(modelInfo.modelLabel) : "loading"}</span></h1>
    <p class="status">${escapeHtml(statusText)}</p>
    <div class="layout">
      <div class="board">${renderBoard()}</div>
      <div class="panel">
        <h2>Model</h2>
        ${
          modelInfo
            ? `<p class="status">neurons: ${modelInfo.neuronCount.toLocaleString()}<br>edges: ${modelInfo.edgeCount.toLocaleString()}<br>backend: ${escapeHtml(modelInfo.backend)}<br>adapter: ${escapeHtml(modelInfo.adapter)}<br>graph hash: ${modelInfo.graphNeuronsSha256.slice(0, 12)}...</p>`
            : `<p class="status">loading full connectome into WebAssembly...</p>`
        }
        <h2>Selected move</h2>
        <p class="status">${lastDecision ? escapeHtml(lastDecision.selectedMove) : "-"}</p>
        <button id="new-game">New game</button>
      </div>
    </div>
    <div class="layout" style="margin-top:16px">
      <div class="panel">
        <h2>From-square activity</h2>
        ${renderBars(lastDecision?.fromRates, squareLabels)}
      </div>
      <div class="panel">
        <h2>To-square activity</h2>
        ${renderBars(lastDecision?.toRates, squareLabels)}
      </div>
    </div>
  `;

  app.querySelectorAll<HTMLDivElement>(".square").forEach((el) => {
    el.addEventListener("click", () => onSquareClick(el.dataset.square!));
  });
  app.querySelector("#new-game")?.addEventListener("click", () => {
    game.reset();
    selectedSquare = null;
    legalTargets = [];
    lastDecision = null;
    statusText = "New game. Your move.";
    render();
  });
}

render();
