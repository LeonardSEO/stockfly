export interface FrameObservation {
  step: number;
  backend: string;
}

export interface FrameAttempt {
  attempt: number;
  restarted: boolean;
}

/** Tracks the single inference trajectory that is safe to retain as a trace. */
export class FrameStreamTracker {
  private attempt = 0;
  private backend: string | null = null;
  private lastStep = 0;

  observe(frame: FrameObservation): FrameAttempt {
    const restarted = this.backend !== null
      && (frame.backend !== this.backend || frame.step <= this.lastStep);
    if (restarted) this.attempt++;
    this.backend = frame.backend;
    this.lastStep = frame.step;
    return { attempt: this.attempt, restarted };
  }
}
