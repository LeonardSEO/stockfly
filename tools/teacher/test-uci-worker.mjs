import { test } from "node:test";
import assert from "node:assert/strict";
import { UciWorker } from "./uci-worker.mjs";

test("worker returns a legal UCI move for the start position", async () => {
  const worker = new UciWorker();
  try {
    const result = await worker.evaluate({
      fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
      nodes: 100,
    });
    assert.match(result.bestmove, /^[a-h][1-8][a-h][1-8][qrbn]?$/);
  } finally {
    worker.close();
  }
});

test("multiPv returns multiple ranked lines", async () => {
  const worker = new UciWorker();
  try {
    const result = await worker.evaluate({
      fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
      nodes: 2000,
      multiPv: 3,
    });
    assert.ok(result.lines.length >= 1);
    for (const line of result.lines) {
      assert.match(line.move, /^[a-h][1-8][a-h][1-8][qrbn]?$/);
    }
  } finally {
    worker.close();
  }
});
