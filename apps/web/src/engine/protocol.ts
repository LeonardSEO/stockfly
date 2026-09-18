import type { ActivationFrame } from "../brain/activation";
import type { MoveTrace } from "../traces/MoveTrace";

export interface ModelManifestInfo {
  neuronCount: number;
  edgeCount: number;
  graphNeuronsSha256: string;
  graphManifestSha256: string;
  sensoryMapSha256: string;
  outputMapSha256: string;
  checkpointSha256: string | null;
  modelLabel: string;
  modelBadge: string;
  backend: string;
  adapter: string;
  fallbackReason?: string;
}

export interface TraceVerification {
  mode: "cpu-replay" | "cpu-reference";
  passed: boolean;
  provenanceMatches: boolean;
  decisionPathMatches: boolean;
  finalMoveMatches: boolean;
  expectedMove: string;
  actualMove: string;
  maxActivationDifference: number;
  activationTolerance: number;
  failedActivationFrameCount: number;
  maxDecisionRateDifference: number;
  decisionRateTolerance: number;
  cpuReplayPassed: boolean;
  recordedTraceMatches: boolean;
  maxCpuReplayActivationDifference: number;
  maxCpuReplayDecisionRateDifference: number;
  note: string;
}

export type StockFlyRequest =
  | { type: "load"; generation: number }
  | { type: "position"; fen: string; traceId: string; generation: number; settleSteps?: number; frameMode?: "quantized" | "full" }
  | { type: "verify-trace"; generation: number; verificationId: string; trace: MoveTrace }
  | { type: "reset"; generation: number };

export type StockFlyResponse =
  | { type: "loaded"; generation: number; manifest: ModelManifestInfo }
  | { type: "error"; generation: number; traceId?: string; verificationId?: string; message: string }
  | { type: "frame-restart"; generation: number; traceId: string; attempt: number; backend: string }
  | { type: "frame"; generation: number; traceId: string; attempt: number; frame: ActivationFrame }
  | { type: "verification"; generation: number; verificationId: string; result: TraceVerification }
  | {
      type: "decision";
      traceId: string;
      generation: number;
      selectedMove: string;
      fromRates: number[];
      toRates: number[];
      promotionRates: number[];
      legalScores: Array<{ move: string; score: number }>;
      settleSteps: number;
      graphNeuronsSha256: string;
      graphManifestSha256: string;
      sensoryMapSha256: string;
      outputMapSha256: string;
      checkpointSha256: string | null;
      backend: string;
      adapter: string;
      fallbackReason?: string;
      modelLabel: string;
    };

/** Exhaustive-switch guard: a compile error here means a new
 * StockFlyResponse variant was added without updating every consumer. */
export function assertNever(x: never): never {
  throw new Error(`unhandled response variant: ${JSON.stringify(x)}`);
}
