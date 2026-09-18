export interface RequestIdentity { generation: number; traceId: string }
/** A reset invalidates frames, decisions, and errors together. */
export class InferenceLifecycle {
  generation = 0;
  private serial = 0;
  active: RequestIdentity | null = null;
  begin(): RequestIdentity {
    return this.active = { generation: this.generation, traceId: `${this.generation}:${++this.serial}` };
  }
  reset(): number { this.active = null; return ++this.generation; }
  accepts(message: RequestIdentity): boolean {
    return this.active?.generation === message.generation && this.active.traceId === message.traceId;
  }
  finish(): void { this.active = null; }
}
