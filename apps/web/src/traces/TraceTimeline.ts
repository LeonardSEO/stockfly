import type { ActivationFrame } from '../brain/activation';
import type { MoveTrace } from './MoveTrace';

export class TraceTimeline {
  private index = 0;
  readonly trace: MoveTrace;
  private readonly onSeek?: (frame: ActivationFrame, index: number) => void;

  constructor(trace: MoveTrace, onSeek?: (frame: ActivationFrame, index: number) => void) {
    if (trace.frames.length === 0) throw new Error('A trace timeline needs at least one frame');
    this.trace = trace;
    this.onSeek = onSeek;
  }

  get currentIndex(): number { return this.index; }
  get currentFrame(): ActivationFrame { return this.trace.frames[this.index]; }
  get length(): number { return this.trace.frames.length; }

  seek(index: number): ActivationFrame {
    if (!Number.isInteger(index) || index < 0 || index >= this.trace.frames.length) throw new RangeError('Timeline index is out of range');
    this.index = index;
    const frame = this.trace.frames[index];
    this.onSeek?.(frame, index);
    return frame;
  }

  nextDelayMs(): number {
    if (this.index >= this.trace.frames.length - 1) return 0;
    return Math.max(0, this.trace.frames[this.index + 1].elapsedMs - this.currentFrame.elapsedMs);
  }
}
