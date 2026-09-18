#!/usr/bin/env node
/**
 * Deterministic local chess curriculum generator. No web scraping: every
 * position comes from seeded random legal play (verified by chess.js) or
 * a small set of hand-coded minimal-material endgame skeletons, per the
 * design spec's "do not scrape the web for weekend MVP" instruction.
 *
 * mate1 positions are discovered, not hand-typed from memory: the
 * generator samples reduced-material positions and one-ply-searches every
 * legal move with chess.js's own isCheckmate(), so the recorded
 * "bestmove" is mechanically guaranteed correct rather than a
 * human-recalled puzzle that might be wrong.
 *
 * Output: JSONL rows {"fen":..., "bestmove":..., "stage":..., "nodes":..., "seed":...}
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Chess } from "chess.js";
import { UciWorker } from "./uci-worker.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CONFIG_PATH = path.join(HERE, "curriculum-config.json");

// Minimal-material skeletons used as seeds for the "endgame" stage and as
// a richer pool for mate1 discovery (mates are much more common with few
// pieces on the board).
const ENDGAME_SKELETONS = [
  "8/8/8/8/8/4k3/4P3/4K3 w - - 0 1", // K+P vs K
  "8/8/8/8/8/3k4/8/R3K3 w - - 0 1", // K+R vs K
  "8/8/8/8/8/3k4/8/Q3K3 w - - 0 1", // K+Q vs K
  "8/8/8/8/3k4/8/3P4/3K4 b - - 0 1", // K+P vs K, black to move
];

function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function pick(rng, arr) {
  return arr[Math.floor(rng() * arr.length)];
}

function randomWalk(rng, startFen, minPlies, maxPlies) {
  const chess = new Chess(startFen);
  const plies = minPlies + Math.floor(rng() * (maxPlies - minPlies + 1));
  for (let i = 0; i < plies; i++) {
    const moves = chess.moves();
    if (moves.length === 0) break;
    chess.move(pick(rng, moves));
  }
  return chess;
}

function generateLegal(rng) {
  return randomWalk(rng, undefined, 2, 30);
}

function generateCapture(rng) {
  for (let attempt = 0; attempt < 200; attempt++) {
    const chess = randomWalk(rng, undefined, 2, 40);
    if (chess.isGameOver()) continue;
    const hasCapture = chess.moves({ verbose: true }).some((m) => m.captured);
    if (hasCapture) return chess;
  }
  return randomWalk(rng, undefined, 2, 40);
}

function generateCheckEvasion(rng) {
  for (let attempt = 0; attempt < 300; attempt++) {
    const chess = randomWalk(rng, undefined, 2, 40);
    if (!chess.isGameOver() && chess.isCheck()) return chess;
  }
  return null; // caller falls back to another stage on repeated failure
}

/**
 * Constructs a real, verified back-rank mate-in-1 skeleton directly rather
 * than hoping random play stumbles into one (random legal play very rarely
 * delivers checkmate against a lone king by chance). The mating rook/queen
 * square and the defending king's corner are varied by the seed; every
 * candidate is still verified with chess.js's own isCheckmate() before
 * being accepted, so a construction mistake fails loudly instead of
 * silently producing a wrong "mate1" label.
 */
function generateMate1(rng) {
  const kingside = rng() < 0.5;
  const shieldRank = "7";
  const backRank = "8";
  const kingFile = kingside ? "g" : "b";
  const shieldFiles = kingside ? ["f", "g", "h"] : ["a", "b", "c"];
  const attackerIsQueen = rng() < 0.5;
  const attackerFile = pick(
    rng,
    "abcdefgh".split("").filter((f) => !shieldFiles.includes(f) && f !== kingFile)
  );
  const attackerRank = String(2 + Math.floor(rng() * 5)); // ranks 2-6, clear path to the 8th

  const board = Array.from({ length: 8 }, () => Array(8).fill(null));
  const fileIdx = (f) => f.charCodeAt(0) - "a".charCodeAt(0);
  const rankIdx = (r) => 8 - parseInt(r, 10); // board[0] = rank 8

  board[rankIdx(backRank)][fileIdx(kingFile)] = "k";
  for (const f of shieldFiles) board[rankIdx(shieldRank)][fileIdx(f)] = "p";
  board[rankIdx(attackerRank)][fileIdx(attackerFile)] = attackerIsQueen ? "Q" : "R";
  // Defending white king tucked safely away from the action.
  const whiteKingFile = kingside ? "a" : "h";
  board[rankIdx("1")][fileIdx(whiteKingFile)] = "K";

  const fenBoard = board
    .map((row) => {
      let s = "";
      let empty = 0;
      for (const cell of row) {
        if (cell === null) {
          empty++;
        } else {
          if (empty > 0) {
            s += empty;
            empty = 0;
          }
          s += cell;
        }
      }
      if (empty > 0) s += empty;
      return s;
    })
    .join("/");
  const fen = `${fenBoard} w - - 0 1`;

  const chess = new Chess(fen);
  const mateSquare = `${attackerFile}${backRank}`;
  const attackerSquare = `${attackerFile}${attackerRank}`;
  const moves = chess.moves({ verbose: true });
  const mateMove = moves.find((m) => m.from === attackerSquare && m.to === mateSquare);
  if (!mateMove) return null;

  const trial = new Chess(fen);
  trial.move(mateMove.san);
  if (!trial.isCheckmate()) return null; // construction guard, should not trigger

  return { chess, mateMove: `${mateMove.from}${mateMove.to}` };
}

function generateEndgame(rng) {
  const skeleton = pick(rng, ENDGAME_SKELETONS);
  return randomWalk(rng, skeleton, 0, 15);
}

function generateMixed(rng) {
  return randomWalk(rng, undefined, 10, 60);
}

async function main() {
  const args = Object.fromEntries(
    process.argv.slice(2).reduce((acc, arg, i, arr) => {
      if (arg.startsWith("--")) acc.push([arg.slice(2), arr[i + 1]]);
      return acc;
    }, [])
  );
  const preset = args.preset || "smoke";
  const outPath = args.out || `data/teacher/${preset}.jsonl`;
  const seedBase = parseInt(args.seed || "42", 10);
  const configPath = args.config || CONFIG_PATH;

  const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
  const presetConfig = config.presets[preset];
  if (!presetConfig) {
    console.error(`unknown preset: ${preset}. Available: ${Object.keys(config.presets).join(", ")}`);
    process.exit(1);
  }

  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  const out = fs.createWriteStream(outPath);
  const worker = new UciWorker();

  const stageNames = Object.keys(config.stages);
  const targetPerStage = {};
  for (const name of stageNames) {
    targetPerStage[name] = Math.round(presetConfig.total_examples * config.stages[name].share);
  }

  const deadline = Date.now() + presetConfig.wall_clock_seconds * 1000;
  let written = 0;
  let seedCounter = seedBase;

  outer: for (const stage of stageNames) {
    const nodes = config.stages[stage].nodes;
    const target = targetPerStage[stage];
    for (let i = 0; i < target; i++) {
      if (Date.now() > deadline) break outer;
      const seed = seedCounter++;
      const rng = mulberry32(seed);

      let fen, bestmove;
      if (stage === "legal") {
        const chess = generateLegal(rng);
        if (chess.isGameOver()) continue;
        fen = chess.fen();
        bestmove = (await worker.evaluate({ fen, nodes })).bestmove;
      } else if (stage === "capture") {
        const chess = generateCapture(rng);
        if (chess.isGameOver()) continue;
        fen = chess.fen();
        bestmove = (await worker.evaluate({ fen, nodes })).bestmove;
      } else if (stage === "check-evasion") {
        const chess = generateCheckEvasion(rng);
        if (!chess) continue;
        fen = chess.fen();
        bestmove = (await worker.evaluate({ fen, nodes })).bestmove;
      } else if (stage === "mate1") {
        const result = generateMate1(rng);
        if (!result) continue;
        fen = result.chess.fen();
        bestmove = result.mateMove;
      } else if (stage === "endgame") {
        const chess = generateEndgame(rng);
        if (chess.isGameOver()) continue;
        fen = chess.fen();
        bestmove = (await worker.evaluate({ fen, nodes })).bestmove;
      } else if (stage === "mixed") {
        const chess = generateMixed(rng);
        if (chess.isGameOver()) continue;
        fen = chess.fen();
        bestmove = (await worker.evaluate({ fen, nodes })).bestmove;
      }

      if (!fen || !bestmove) continue;
      out.write(JSON.stringify({ fen, bestmove, stage, nodes, seed }) + "\n");
      written++;
    }
  }

  worker.close();
  out.end();
  console.log(`wrote ${written} examples to ${outPath} (preset=${preset})`);
}

main();
