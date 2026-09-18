#!/usr/bin/env node
/** Actual games: opponent info/PV lines are discarded, only its move is retained.
 * The separate native model process accepts only {fen}; graph/model stay loaded.
 */
import { spawn, spawnSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { Chess } from 'chess.js';

export class LineProcess {
  constructor(command, args, timeout = 120000) {
    this.timeout = timeout;
    this.child = spawn(command, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    this.lines = createInterface({ input: this.child.stdout });
    this.stderr = '';
    this.child.stderr.on('data', chunk => { this.stderr = (this.stderr + chunk).slice(-4000); });
    this.lines.on('line', line => {
      if (this.pending?.test(line)) {
        const pending = this.pending;
        this.pending = null;
        clearTimeout(pending.timer);
        pending.resolve(line);
      }
      // No history of UCI info/PV: memory stays bounded across long matches.
    });
    this.child.on('error', error => this.fail(error));
    this.child.on('exit', (code, signal) => this.fail(new Error(`process exit ${code}/${signal}: ${this.stderr}`)));
    this.child.stdin.on('error', error => this.fail(error));
  }
  fail(error) {
    this.failure = error;
    if (this.pending) {
      clearTimeout(this.pending.timer);
      this.pending.reject(error);
      this.pending = null;
    }
  }
  wait(test, commands = []) {
    if (this.failure) return Promise.reject(this.failure);
    if (this.pending) return Promise.reject(new Error('overlapping process requests'));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.fail(Object.assign(new Error('process timeout'), { code: 'TIMEOUT' }));
        this.close();
      }, this.timeout);
      this.pending = { test, resolve, reject, timer };
      for (const command of commands) this.child.stdin.write(command + '\n');
    });
  }
  close() {
    this.fail(new Error('process closed'));
    this.lines.close();
    if (this.child.exitCode === null) this.child.kill();
  }
}

export class Opponent {
  constructor(file, config) { this.process = new LineProcess(process.execPath, [file], config.timeout_ms); this.config = config; }
  async ready() {
    await this.process.wait(line => line === 'uciok', ['uci']);
    await this.process.wait(line => line === 'readyok', [
      'setoption name Threads value 1', `setoption name Hash value ${this.config.hash_mb}`,
      'setoption name Ponder value false', 'setoption name UCI_LimitStrength value false',
      'setoption name Skill Level value 20', 'setoption name MultiPV value 1',
      'setoption name SyzygyPath value <empty>', 'isready',
    ]);
  }
  async newGame() {
    await this.process.wait(line => line === 'readyok', ['ucinewgame', 'setoption name Clear Hash', 'isready']);
  }
  async move(fen, nodes, history) {
    const position = history ? `position startpos moves ${history.join(" ")}` : `position fen ${fen}`;
    const line = await this.process.wait(line => line.startsWith('bestmove '), [position, `go nodes ${nodes}`]);
    return line.split(/\s+/)[1];
  }
  close() { this.process.close(); }
}

export class Fly {
  constructor(binary, model, control, config) {
    this.process = new LineProcess(binary, ['ladder-infer', '--model', model, '--control', control, '--seed', String(config.control_seed), '--settle-steps', String(config.settle_steps)], config.timeout_ms);
  }
  async ready() {
    const ready = JSON.parse(await this.process.wait(line => line.startsWith('{')));
    if (!ready.ready || !ready.identity) throw new Error('invalid model handshake');
    return ready.identity;
  }
  async move(fen) {
    const reply = JSON.parse(await this.process.wait(line => line.startsWith('{'), [JSON.stringify({ fen })]));
    if (reply.error) throw new Error(reply.error);
    return reply.move;
  }
  close() { this.process.close(); }
}

function rng(seed) {
  let state = seed >>> 0;
  return () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state; };
}
export function opening(config, pair) {
  const seed = (config.opening_seed + pair) >>> 0;
  const random = rng(seed);
  const chess = new Chess();
  const moves = [];
  for (let ply = 0; ply < config.opening_plies; ply++) {
    const legal = chess.moves({ verbose: true }).sort((a, b) => a.lan.localeCompare(b.lan));
    if (!legal.length || chess.isGameOver()) throw new Error('terminal opening: increase/change declared opening seed');
    const move = legal[random() % legal.length];
    chess.move(move.lan);
    moves.push(move.lan);
  }
  if (chess.isGameOver()) throw new Error('terminal opening');
  return { seed, fen: chess.fen(), moves };
}
export function terminal(chess, modelColor) {
  if (chess.isCheckmate()) return { termination: 'checkmate', score: chess.turn() === modelColor ? 0 : 1 };
  if (chess.isStalemate()) return { termination: 'stalemate', score: 0.5 };
  if (chess.isInsufficientMaterial()) return { termination: 'insufficient-material', score: 0.5 };
  if (chess.isThreefoldRepetition()) return { termination: 'threefold-repetition', score: 0.5 };
  if (chess.isDrawByFiftyMoves()) return { termination: 'fifty-move', score: 0.5 };
  return null;
}
export async function playGame({ fly, opponent, config, nodes, pair, color }) {
  const book = opening(config, pair);
  // Replay opening history so repetition claims include the opening.
  const chess = new Chess();
  for (const move of book.moves) chess.move(move);
  const game = { pair, color, opening: book, nodes, moves: [], score: null, termination: null, started_at: new Date().toISOString() };
  const start = Date.now();
  let actor = 'opponent';
  try {
    await opponent.newGame();
    for (let ply = 0; ; ply++) {
      const result = terminal(chess, color);
      if (result) { Object.assign(game, result); break; }
      if (ply >= config.max_plies) { game.termination = 'ply-cap'; break; }
      actor = chess.turn() === color ? 'model' : 'opponent';
      const uci = actor === 'model' ? await fly.move(chess.fen()) : await opponent.move(chess.fen(), nodes, [...book.moves, ...game.moves]);
      if (typeof uci !== 'string' || !/^[a-h][1-8][a-h][1-8][qrbn]?$/.test(uci)) throw new Error(`invalid ${actor} move: ${uci}`);
      const move = chess.move({ from: uci.slice(0, 2), to: uci.slice(2, 4), promotion: uci[4] });
      game.moves.push(move.lan);
    }
  } catch (error) {
    game.termination = error.code === 'TIMEOUT' ? `${actor}-timeout` : `${actor}-error`;
    game.error = String(error);
  }
  game.final_fen = chess.fen();
  game.elapsed_ms = Date.now() - start;
  return game;
}

const digest = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function save(file, report) {
  fs.writeFileSync(file + '.partial', JSON.stringify(report, null, 2) + '\n');
  fs.renameSync(file + '.partial', file);
}
export function sameIdentity(actual, expected) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('resume identity/config mismatch; use a fresh output path');
}
function summarize(binary, games, config) {
  const result = spawnSync(binary, ['elo-summary'], { input: JSON.stringify({ games, replicates: config.bootstrap_replicates, seed: config.bootstrap_seed }), encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr || 'statistics process failed');
  return JSON.parse(result.stdout);
}

export async function main(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!['--model', '--out', '--binary', '--control', '--budgets', '--pairs'].includes(argv[i]) || !argv[i+1] || args[argv[i]]) throw new Error('invalid arguments');
    args[argv[i]] = argv[i+1];
  }
  if (!args['--model'] || !args['--out']) throw new Error('usage: node tools/ladder/run.mjs --model checkpoint --out report.json [--control intact|reset|shuffled|output-permuted|brain-bypass] [--budgets 50,100,...] [--pairs 10] [--binary target/release/stockfly-train]');
  const config = JSON.parse(fs.readFileSync('crates/stockfly-train/resources/ladder.json'));
  const control = args['--control'] || 'intact';
  if (control !== 'intact' && !config.controls.includes(control)) throw new Error('invalid control');
  const budgets = args['--budgets'] ? args['--budgets'].split(',').map(Number) : control === 'intact' ? config.budgets : [config.control_budget];
  const pairs = args['--pairs'] ? Number(args['--pairs']) : config.opening_pairs;
  if (!Number.isInteger(pairs) || pairs < 1 || !budgets.length || new Set(budgets).size !== budgets.length || budgets.some(n => !config.budgets.includes(n))) throw new Error('invalid pairs/budgets');
  const binary = path.resolve(args['--binary'] || 'target/release/stockfly-train');
  const vendor = 'data/vendor/stockfish-19-lite';
  const sources = JSON.parse(fs.readFileSync(`${vendor}/sources.json`));
  const assets = Object.fromEntries(sources.files.filter(f => /\.(js|wasm)$/.test(f.file)).map(f => {
    const actual = digest(`${vendor}/${f.file}`);
    if (actual !== f.sha256) throw new Error('Stockfish asset hash mismatch');
    return [f.file, actual];
  }));
  const out = path.resolve(args['--out']);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  const lock = fs.openSync(out + '.lock', 'wx');
  fs.writeSync(lock, String(process.pid));
  let fly, opponent;
  try {
    fly = new Fly(binary, args['--model'], control, config);
    const modelIdentity = await fly.ready();
    opponent = new Opponent(`${vendor}/stockfish-19-lite-single.js`, config);
    await opponent.ready();
    const identity = { protocol: config, budgets, target_pairs: pairs, model: modelIdentity, opponent: { version: sources.version, assets, threads: 1, hash_mb: config.hash_mb, ponder: false, limit_strength: false, skill_level: 20, multipv: 1, tablebases: 'disabled', reset: 'ucinewgame + Clear Hash before every game' }, controller_sha256: digest(fileURLToPath(import.meta.url)), chess_js_sha256: digest('node_modules/chess.js/dist/cjs/chess.js'), node: process.version };
    const report = fs.existsSync(out) ? JSON.parse(fs.readFileSync(out)) : { report_version: 1, identity, games: [], summary: {}, complete: false };
    sameIdentity(report.identity, identity);
    for (const nodes of budgets) {
      let pair = 0;
      const seen = new Set();
      while (true) {
        const fen = opening(config, pair).fen;
        if (seen.has(fen)) throw new Error('duplicate opening cluster');
        seen.add(fen);
        for (const color of ['w', 'b']) {
          const existing = report.games.filter(g => g.nodes === nodes && g.pair === pair && g.color === color);
          if (existing.length > 1) throw new Error('duplicate saved game');
          if (existing.length) continue;
          const game = await playGame({ fly, opponent, config, nodes, pair, color });
          report.games.push(game);
          report.summary[nodes] = summarize(binary, report.games.filter(g => g.nodes === nodes), config);
          save(out, report);
          console.log(JSON.stringify({ out, nodes, pair, color, termination: game.termination, score: game.score, elapsed_ms: game.elapsed_ms, summary: report.summary[nodes] }));
          if (game.termination.endsWith('error') || game.termination.endsWith('timeout')) throw new Error(`saved ${game.termination}; resolve failure before continuing`);
        }
        if (report.summary[nodes]?.pairs >= pairs) break;
        pair++;
        if (pair >= pairs*5) throw new Error('too many incomplete pairs; report retained without claiming completion');
      }
    }
    report.complete = true;
    report.completed_at = new Date().toISOString();
    save(out, report);
  } finally {
    fly?.close(); opponent?.close();
    fs.closeSync(lock); fs.unlinkSync(out + '.lock');
  }
}
if (process.argv[1] && import.meta.url === `file://${path.resolve(process.argv[1])}`) {
  main(process.argv.slice(2)).catch(error => { console.error(error); process.exitCode = 1; });
}
