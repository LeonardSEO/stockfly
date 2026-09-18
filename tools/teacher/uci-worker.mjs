#!/usr/bin/env node
/**
 * Isolated Stockfish teacher worker. Reads newline-delimited JSON requests
 * on stdin, spawns a single long-lived Stockfish UCI process, and writes
 * newline-delimited JSON responses on stdout.
 *
 * Request:  {"id":1,"fen":"...","nodes":2000,"multiPv":3}
 * Response: {"id":1,"bestmove":"e2e4","lines":[{"move":"e2e4","cp":31}]}
 *
 * Isolation: this process only ever receives UCI requests and returns a
 * best move plus diagnostic scores. It never loads a StockFly model file,
 * and its output is training-time-only -- the StockFly simulator itself
 * never sees these evaluations (see the design spec's Stockfish-isolation
 * requirement and the no-teacher causal-audit control).
 *
 * Binary: STOCKFISH_BIN env var, defaulting to a native Stockfish 19
 * install (`brew install stockfish`). The design spec calls for the
 * official Stockfish 19 Lite single-threaded WASM build specifically for
 * the in-browser opponent mode, where WASM execution is a hard
 * requirement; that fetch/verification is implemented separately in
 * fetch-stockfish.mjs for that browser task. For local curriculum
 * generation, any real Stockfish 19 UCI engine produces the same kind of
 * ground truth this protocol needs, so this worker is deliberately
 * binary-path-agnostic.
 */
import { spawn } from "node:child_process";
import readline from "node:readline";

const STOCKFISH_BIN = process.env.STOCKFISH_BIN || "/opt/homebrew/bin/stockfish";

export class UciWorker {
  constructor(binPath = STOCKFISH_BIN) {
    this.proc = spawn(binPath, [], { stdio: ["pipe", "pipe", "pipe"] });
    this.rl = readline.createInterface({ input: this.proc.stdout });
    this.pendingLines = [];
    this.readyResolvers = [];
    this.rl.on("line", (line) => this._onLine(line));
    this._ready = this._handshake();
  }

  async _handshake() {
    await this._sendAndWaitFor("uci", "uciok");
    await this._sendAndWaitFor("isready", "readyok");
  }

  _send(cmd) {
    this.proc.stdin.write(cmd + "\n");
  }

  _onLine(line) {
    this.pendingLines.push(line);
    for (const resolver of [...this.readyResolvers]) {
      if (resolver.test(line)) {
        this.readyResolvers = this.readyResolvers.filter((r) => r !== resolver);
        resolver.resolve(line);
      }
    }
  }

  _sendAndWaitFor(cmd, marker) {
    return new Promise((resolve) => {
      this.readyResolvers.push({ test: (line) => line.trim() === marker, resolve });
      this._send(cmd);
    });
  }

  async evaluate({ fen, nodes = 2000, multiPv = 1 }) {
    await this._ready;
    this._send(`setoption name MultiPV value ${multiPv}`);
    this._send(`position fen ${fen}`);

    const infoByPv = new Map();
    const bestmove = await new Promise((resolve) => {
      const onLine = (line) => {
        if (line.startsWith("info") && line.includes("multipv")) {
          const pvMatch = line.match(/multipv (\d+)/);
          const cpMatch = line.match(/score cp (-?\d+)/);
          const mateMatch = line.match(/score mate (-?\d+)/);
          const moveMatch = line.match(/ pv (\S+)/);
          if (pvMatch && moveMatch) {
            const pv = parseInt(pvMatch[1], 10);
            infoByPv.set(pv, {
              move: moveMatch[1],
              cp: cpMatch ? parseInt(cpMatch[1], 10) : undefined,
              mate: mateMatch ? parseInt(mateMatch[1], 10) : undefined,
            });
          }
        } else if (line.startsWith("bestmove")) {
          this.rl.off("line", onLine);
          resolve(line.split(" ")[1]);
        }
      };
      this.rl.on("line", onLine);
      this._send(`go nodes ${nodes}`);
    });

    const lines = [...infoByPv.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([, v]) => v);

    return { bestmove, lines };
  }

  close() {
    this._send("quit");
    this.proc.kill();
  }
}

async function main() {
  const worker = new UciWorker();
  const rl = readline.createInterface({ input: process.stdin });

  rl.on("line", async (line) => {
    if (!line.trim()) return;
    let request;
    try {
      request = JSON.parse(line);
    } catch (e) {
      process.stdout.write(JSON.stringify({ error: `invalid JSON: ${e.message}` }) + "\n");
      return;
    }
    try {
      const result = await worker.evaluate(request);
      process.stdout.write(JSON.stringify({ id: request.id, ...result }) + "\n");
    } catch (e) {
      process.stdout.write(JSON.stringify({ id: request.id, error: String(e) }) + "\n");
    }
  });

  rl.on("close", () => worker.close());
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}
