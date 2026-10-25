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
