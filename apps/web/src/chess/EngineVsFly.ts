import type { MatchActor, Side } from "./HumanVsFly";

/** Owns only match pacing. Engine computation and board mutation stay outside. */
export class EngineVsFly {
  private paused = false;
  private pauseAfterPly = false;
  flySide: Side;

  constructor(flySide: Side = "b") {
    this.flySide = flySide;
  }

  actorFor(turn: Side, gameOver: boolean): MatchActor | null {
    if (gameOver || this.paused) return null;
    return turn === this.flySide ? "stockfly" : "stockfish";
  }

  completePly(): void {
    if (this.pauseAfterPly) {
      this.pauseAfterPly = false;
      this.paused = true;
    }
  }

  pause(): void {
    this.paused = true;
    this.pauseAfterPly = false;
  }

  resume(): void {
    this.paused = false;
    this.pauseAfterPly = false;
  }

  step(): void {
    this.paused = false;
    this.pauseAfterPly = true;
  }

  restart(): void {
    this.pauseAfterPly = false;
  }

  setFlySide(side: Side): void {
    this.flySide = side;
  }

  nameFor(side: Side): "StockFly" | "Stockfish 19 Lite" {
    return side === this.flySide ? "StockFly" : "Stockfish 19 Lite";
  }

  get isPaused(): boolean {
    return this.paused;
  }
}
