interface PollClock {
  set(work: () => void, interval: number): number;
  clear(timer: number): void;
}

export class VisibilityPoll {
  private visible = false;
  private disposed = false;
  private running = false;
  private pending = false;
  private timer: number | undefined;
  private readonly interval: number;
  private readonly work: () => Promise<void>;
  private readonly report: (error: unknown) => void;
  private readonly clock: PollClock;

  constructor(interval: number, work: () => Promise<void>, report: (error: unknown) => void, clock: PollClock = {
    set: (tick, delay) => window.setInterval(tick, delay),
    clear: (timer) => window.clearInterval(timer),
  }) {
    this.interval = interval; this.work = work; this.report = report; this.clock = clock;
  }

  setVisible(visible: boolean, refresh = true): void {
    if (this.disposed || this.visible === visible) return;
    this.visible = visible;
    if (this.timer !== undefined) this.clock.clear(this.timer);
    this.timer = undefined;
    this.pending = false;
    if (!visible) return;
    this.timer = this.clock.set(() => { void this.tick(); }, this.interval);
    if (refresh) void this.tick();
  }

  dispose(): void {
    this.setVisible(false);
    this.disposed = true;
  }

  private async tick(): Promise<void> {
    if (!this.visible || this.disposed) return;
    if (this.running) { this.pending = true; return; }
    this.running = true;
    try { await this.work(); }
    catch (error) { if (this.visible && !this.disposed) this.report(error); }
    finally {
      this.running = false;
      if (this.pending && this.visible && !this.disposed) {
        this.pending = false;
        void this.tick();
      }
    }
  }
}
