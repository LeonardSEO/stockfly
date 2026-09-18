export type Side = "w" | "b";
export type MatchActor = "human" | "stockfly" | "stockfish";

export class HumanVsFly {
  humanSide: Side;

  constructor(humanSide: Side = "w") {
    this.humanSide = humanSide;
  }

  actorFor(turn: Side, gameOver: boolean): MatchActor | null {
    if (gameOver) return null;
    return turn === this.humanSide ? "human" : "stockfly";
  }

  setHumanSide(side: Side): void {
    this.humanSide = side;
  }

  nameFor(side: Side): "You" | "StockFly" {
    return side === this.humanSide ? "You" : "StockFly";
  }
}
