export interface ModelManifestInfo {
  neuronCount: number;
  edgeCount: number;
  graphNeuronsSha256: string;
}

export type StockFlyRequest =
  | { type: "load" }
  | { type: "position"; fen: string; traceId: string; settleSteps?: number }
  | { type: "reset" };

export type StockFlyResponse =
  | { type: "loaded"; manifest: ModelManifestInfo }
  | { type: "error"; message: string }
  | {
      type: "decision";
      traceId: string;
      selectedMove: string;
      fromRates: number[];
      toRates: number[];
      promotionRates: number[];
      settleSteps: number;
      graphNeuronsSha256: string;
      sensoryMapSha256: string;
      outputMapSha256: string;
    };

/** Exhaustive-switch guard: a compile error here means a new
 * StockFlyResponse variant was added without updating every consumer. */
export function assertNever(x: never): never {
  throw new Error(`unhandled response variant: ${JSON.stringify(x)}`);
}
