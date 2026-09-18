import { Chess, type Square } from "chess.js";

export type GameTermination =
  | { outcome: "win"; winner: "w" | "b"; reason: "checkmate" }
  | { outcome: "draw"; reason: "stalemate" | "insufficient material" | "threefold repetition" | "fifty-move rule" };

export class Game {
  private chess: Chess;

  constructor(fen?: string) {
    this.chess = new Chess(fen);
  }

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
    return this.chess.moves({ square: square as Square, verbose: true }).map((m) => m.to);
  }

  promotions(from: string, to: string): string[] {
    return this.chess.moves({ square: from as Square, verbose: true })
      .filter(move => move.to === to && move.promotion)
      .map(move => move.promotion!);
  }

  history(): string[] { return this.chess.history(); }

  result(): string | null {
    const ending = this.termination();
    if (!ending) return null;
    if (ending.outcome === "win") return `${ending.winner === "w" ? "White" : "Black"} wins by checkmate.`;
    return `Draw by ${ending.reason}.`;
  }

  termination(): GameTermination | null {
    if (this.chess.isCheckmate()) return { outcome: "win", winner: this.turn() === "w" ? "b" : "w", reason: "checkmate" };
    // Keep chess.js draw precedence when more than one terminal condition holds.
    if (this.chess.isDrawByFiftyMoves()) return { outcome: "draw", reason: "fifty-move rule" };
    if (this.chess.isStalemate()) return { outcome: "draw", reason: "stalemate" };
    if (this.chess.isInsufficientMaterial()) return { outcome: "draw", reason: "insufficient material" };
    if (this.chess.isThreefoldRepetition()) return { outcome: "draw", reason: "threefold repetition" };
    return null;
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
