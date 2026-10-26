export class SerialQueue {
  private pending: Promise<unknown> = Promise.resolve();

  run<T>(work: () => Promise<T>): Promise<T> {
    const result = this.pending.then(work);
    this.pending = result.catch(() => undefined);
    return result;
  }

  async drain(): Promise<void> {
    await this.pending;
  }
}

export async function persistAll(tasks: (() => Promise<void>)[]): Promise<void> {
  const results = await Promise.allSettled(tasks.map((task) => Promise.resolve().then(task)));
  const errors = results.flatMap((result) => result.status === "rejected" ? [result.reason] : []);
  if (errors.length) throw new Error(errors.map((error: unknown) => error instanceof Error ? error.message : String(error)).join("\n"));
}

export class LatestRequest {
  private generation = 0;

  cancel(): void {
    this.generation += 1;
  }

  async run<T>(
    work: () => Promise<T>,
    accept: (result: T) => void,
    reject?: (error: unknown) => void,
  ): Promise<boolean> {
    const generation = ++this.generation;
    try {
      const result = await work();
      if (generation !== this.generation) return false;
      accept(result);
      return true;
    } catch (error) {
      if (generation !== this.generation) return false;
      if (!reject) throw error;
      reject(error);
      return true;
    }
  }
}

export async function orFallback<T>(work: () => Promise<T>, fallback: T, report: (error: unknown) => void): Promise<T> {
  try { return await work(); }
  catch (error) { report(error); return fallback; }
}

/** Resolves once the browser has painted, so a click's new state is on screen before work that
 * may hold the app's main thread: on macOS the webview cannot paint while that thread is busy. */
export function afterPaint(frame: ((callback: () => void) => unknown) | undefined = globalThis.requestAnimationFrame?.bind(globalThis)): Promise<void> {
  return new Promise((resolve) => {
    // Hidden windows get no frames.
    const fallback = setTimeout(resolve, 100);
    frame?.(() => setTimeout(() => { clearTimeout(fallback); resolve(); }, 0));
  });
}

export interface ContentSlot<T> { replaceChildren(...nodes: T[]): void }

/** Shows `placeholder` at once and the loaded content when it arrives, unless the view was left. */
export async function fillWhenReady<T>(slot: ContentSlot<T>, placeholder: T, load: () => Promise<T[]>, live: () => boolean): Promise<void> {
  slot.replaceChildren(placeholder);
  const content = await load();
  if (live()) slot.replaceChildren(...content);
}
