export interface StockfishSearch {
  type: "search";
  fen: string;
  generation: number;
  requestId: string;
}

export type StockfishRequest = StockfishSearch | { type: "reset"; generation: number };
export type StockfishResponse =
  | { type: "move"; uci: string; generation: number; requestId: string }
  | { type: "error"; message: string; generation: number; requestId?: string };

interface EnginePort {
  postMessage(message: string): void;
  addEventListener(type: "message", listener: (event: MessageEvent<unknown>) => void): void;
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void;
  terminate(): void;
}

type PortFactory = () => EnginePort;
type Emit = (message: StockfishResponse) => void;

const UCI_MOVE = /^[a-h][1-8][a-h][1-8][qrbn]?$/;

/**
 * The isolation boundary around the official UCI worker. All `info`, score and
 * PV output terminates here; only a validated best move or an operational error
 * reaches the application worker's parent.
 */
export class StockfishBridge {
  private port: EnginePort | null = null;
  private portEpoch = 0;
  private ready = false;
  private searching = false;
  private pending: StockfishSearch | null = null;
  private readonly createPort: PortFactory;
  private readonly emit: Emit;

  constructor(createPort: PortFactory, emit: Emit) {
    this.createPort = createPort;
    this.emit = emit;
  }

  search(request: StockfishSearch): void {
    if (this.pending || this.searching) this.stopEngine();
    this.pending = request;
    this.ensureEngine();
    this.startIfReady();
  }

  reset(): void {
    this.pending = null;
    this.stopEngine();
  }

  private handleEngineOutput(output: unknown, port: EnginePort, epoch: number): void {
    if (port !== this.port || epoch !== this.portEpoch) return;
    if (typeof output !== "string") return;
    for (const line of output.split(/\r?\n/)) {
      const trimmed = line.trim();
      if (trimmed === "uciok") {
        this.port?.postMessage("isready");
      } else if (trimmed === "readyok") {
        this.ready = true;
        this.startIfReady();
      } else if (trimmed.startsWith("bestmove ")) {
        const uci = trimmed.split(/\s+/)[1];
        if (!this.searching || !this.pending || !UCI_MOVE.test(uci)) continue;
        const request = this.pending;
        this.pending = null;
        this.searching = false;
        this.emit({ type: "move", uci, generation: request.generation, requestId: request.requestId });
      }
      // Intentionally discard all UCI identity, evaluation, score and PV lines.
    }
  }

  private ensureEngine(): void {
    if (this.port) return;
    const port = this.createPort();
    const epoch = ++this.portEpoch;
    this.port = port;
    port.addEventListener("message", event => this.handleEngineOutput(event.data, port, epoch));
    port.addEventListener("error", event => {
      if (port !== this.port || epoch !== this.portEpoch) return;
      const request = this.pending;
      this.emit({
        type: "error",
        message: event.message || "Stockfish worker failed",
        generation: request?.generation ?? 0,
        ...(request ? { requestId: request.requestId } : {}),
      });
      this.pending = null;
      this.stopEngine();
    });
    port.postMessage("uci");
  }

  private startIfReady(): void {
    if (!this.ready || !this.pending || this.searching) return;
    this.searching = true;
    this.port!.postMessage(`position fen ${this.pending.fen}`);
    this.port!.postMessage("go movetime 250");
  }

  private stopEngine(): void {
    const port = this.port;
    this.port = null;
    this.portEpoch++;
    this.ready = false;
    this.searching = false;
    port?.terminate();
  }
}

const workerScope = globalThis as typeof globalThis & {
  document?: unknown;
  postMessage?: (message: StockfishResponse) => void;
  addEventListener?: (type: "message", listener: (event: MessageEvent<StockfishRequest>) => void) => void;
  location?: Location;
  Worker?: typeof Worker;
};

if (typeof workerScope.document === "undefined" && workerScope.postMessage && workerScope.addEventListener && workerScope.Worker && workerScope.location) {
  const script = new URL("/vendor/stockfish/stockfish-19-lite-single.js", workerScope.location.origin);
  const bridge = new StockfishBridge(
    () => new workerScope.Worker!(script, { type: "classic", name: "stockfish-19-lite" }),
    message => workerScope.postMessage!(message),
  );
  workerScope.addEventListener("message", event => {
    if (event.data.type === "search") bridge.search(event.data);
    else bridge.reset();
  });
}
