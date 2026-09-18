import test from "node:test";
import assert from "node:assert/strict";
import { EngineVsFly } from "./EngineVsFly.ts";
import { HumanVsFly } from "./HumanVsFly.ts";
import { Game } from "./game.ts";
import { StockfishBridge, type StockfishResponse } from "../engine/stockfish.worker.ts";

class FakeEngine {
  commands: string[] = [];
  terminated = false;
  private messageListener: ((event: MessageEvent<unknown>) => void) | null = null;
  private errorListener: ((event: ErrorEvent) => void) | null = null;
  postMessage(message: string): void { this.commands.push(message); }
  addEventListener(type: "message" | "error", listener: ((event: MessageEvent<unknown>) => void) | ((event: ErrorEvent) => void)): void {
    if (type === "message") this.messageListener = listener as (event: MessageEvent<unknown>) => void;
    else this.errorListener = listener as (event: ErrorEvent) => void;
  }
  terminate(): void { this.terminated = true; }
  line(data: unknown): void { this.messageListener?.({ data } as MessageEvent<unknown>); }
  fail(message: string): void { this.errorListener?.({ message } as ErrorEvent); }
}

test("Stockfish worker boundary drops evaluation and PV and emits only bestmove", () => {
  const engine = new FakeEngine();
  const emitted: StockfishResponse[] = [];
  const bridge = new StockfishBridge(() => engine, message => emitted.push(message));
  bridge.search({ type: "search", fen: "start-fen", generation: 3, requestId: "sf:7" });
  engine.line("id name Stockfish 19 Lite\nuciok");
  engine.line("readyok");
  assert.deepEqual(engine.commands, ["uci", "isready", "position fen start-fen", "go movetime 250"]);
  engine.line("info depth 9 score cp 31 pv e2e4 e7e5\nbestmove e2e4 ponder e7e5");
  assert.deepEqual(emitted, [{ type: "move", uci: "e2e4", generation: 3, requestId: "sf:7" }]);
});

test("Stockfish reset terminates the search and discards its stale bestmove", () => {
  const engine = new FakeEngine();
  const emitted: StockfishResponse[] = [];
  const bridge = new StockfishBridge(() => engine, message => emitted.push(message));
  bridge.search({ type: "search", fen: "old-fen", generation: 4, requestId: "old" });
  engine.line("uciok\nreadyok");
  bridge.reset();
  engine.line("bestmove d2d4");
  assert.equal(engine.terminated, true);
  assert.deepEqual(emitted, []);
});

test("replaced Stockfish port cannot relabel stale moves or errors as the new request", () => {
  const first = new FakeEngine();
  const second = new FakeEngine();
  const engines = [first, second];
  const emitted: StockfishResponse[] = [];
  const bridge = new StockfishBridge(() => engines.shift()!, message => emitted.push(message));

  bridge.search({ type: "search", fen: "old-fen", generation: 8, requestId: "old" });
  first.line("uciok\nreadyok");
  bridge.search({ type: "search", fen: "new-fen", generation: 9, requestId: "new" });
  assert.equal(first.terminated, true);

  first.line("bestmove a2a4");
  first.fail("late failure from replaced worker");
  assert.deepEqual(emitted, []);

  second.line("uciok\nreadyok");
  second.line("info score cp 12 pv e2e4 e7e5\nbestmove e2e4");
  assert.deepEqual(emitted, [{ type: "move", uci: "e2e4", generation: 9, requestId: "new" }]);
});

test("controllers route human and engine turns and pause exactly after one stepped ply", () => {
  const human = new HumanVsFly("w");
  assert.equal(human.actorFor("w", false), "human");
  assert.equal(human.actorFor("b", false), "stockfly");
  assert.equal(human.actorFor("b", true), null);

  const engines = new EngineVsFly("b");
  assert.equal(engines.actorFor("w", false), "stockfish");
  engines.pause();
  assert.equal(engines.actorFor("w", false), null);
  engines.step();
  assert.equal(engines.actorFor("w", false), "stockfish");
  engines.completePly();
  assert.equal(engines.actorFor("b", false), null);
  engines.resume();
  assert.equal(engines.actorFor("b", false), "stockfly");
  engines.setFlySide("w");
  assert.equal(engines.actorFor("b", false), "stockfish");
});

test("engine controller stops scheduling at game over and restart preserves pause state", () => {
  const engines = new EngineVsFly("w");
  engines.pause();
  engines.restart();
  assert.equal(engines.isPaused, true);
  assert.equal(engines.actorFor("w", true), null);
  engines.resume();
  assert.equal(engines.actorFor("w", true), null);
});

test("terminal results identify a real winner and every supported draw reason", () => {
  const mate = new Game();
  for (const move of [["f2", "f3"], ["e7", "e5"], ["g2", "g4"], ["d8", "h4"]]) {
    assert.ok(mate.applyMove(move[0], move[1]));
  }
  assert.deepEqual(mate.termination(), { outcome: "win", winner: "b", reason: "checkmate" });

  assert.deepEqual(new Game("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").termination(),
    { outcome: "draw", reason: "stalemate" });
  assert.deepEqual(new Game("8/8/8/8/8/8/2k5/K7 w - - 0 1").termination(),
    { outcome: "draw", reason: "insufficient material" });
  assert.deepEqual(new Game("8/8/8/8/8/8/2k4R/K7 w - - 100 51").termination(),
    { outcome: "draw", reason: "fifty-move rule" });

  const repetition = new Game();
  for (let cycle = 0; cycle < 2; cycle++) {
    for (const move of [["g1", "f3"], ["g8", "f6"], ["f3", "g1"], ["f6", "g8"]]) {
      assert.ok(repetition.applyMove(move[0], move[1]));
    }
  }
  assert.deepEqual(repetition.termination(), { outcome: "draw", reason: "threefold repetition" });
});

test("winner names follow the assigned side in both match modes", () => {
  const human = new HumanVsFly("w");
  assert.equal(human.nameFor("w"), "You");
  assert.equal(human.nameFor("b"), "StockFly");
  const engines = new EngineVsFly("b");
  assert.equal(engines.nameFor("w"), "Stockfish 19 Lite");
  assert.equal(engines.nameFor("b"), "StockFly");
});
