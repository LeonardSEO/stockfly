import { Chess } from "chess.js";

export class Game {
  private chess = new Chess();

  fen(): string {
    return this.chess.fen();
  }

  board() {
    return this.chess.board();
  }

  isGameOver(): boolean {
    return this.chess.isGameOver();
  }

  turn(): "w" | "b" {
    return this.chess.turn();
  }

  legalMovesFrom(square: string): string[] {
    return this.chess.moves({ square: square as any, verbose: true }).map((m: any) => m.to);
  }

  /** Applies a move given as `from`+`to` (+ optional promotion), returns
   * the resulting SAN or null if illegal. */
  applyMove(from: string, to: string, promotion?: string): string | null {
    try {
      const move = this.chess.move({ from, to, promotion: promotion ?? "q" });
      return move ? move.san : null;
    } catch {
      return null;
    }
  }

  /** Applies a move given as a UCI string (e.g. "e2e4", "e7e8q"). */
  applyUci(uci: string): string | null {
    const from = uci.slice(0, 2);
    const to = uci.slice(2, 4);
    const promotion = uci.length > 4 ? uci.slice(4) : undefined;
    return this.applyMove(from, to, promotion);
  }

  reset() {
    this.chess.reset();
  }
}
