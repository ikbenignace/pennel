export interface HistoryFocusTarget { id: string; action?: string }
export interface HistoryFocusPlan {
  target: HistoryFocusTarget | null;
  fallback: "notice" | "heading" | null;
  scroll: "center" | "nearest" | null;
}

/** Process-local navigation state; no dictated content or persistent usage data is retained. */
export class HistoryFocusMemory {
  private value: HistoryFocusTarget | null = null;

  remember(id: string, action?: string): void {
    this.value = action ? { id, action } : { id };
  }

  selection(available: ReadonlySet<string>): HistoryFocusTarget | null {
    if (this.value && !available.has(this.value.id)) this.value = null;
    return this.value ? { ...this.value } : null;
  }

  plan(options: { requested?: string; previous: HistoryFocusTarget | null; available: ReadonlySet<string>; returning: boolean; unobstructed: boolean }): HistoryFocusPlan {
    const remembered = this.selection(options.available);
    if (options.requested) {
      if (!options.available.has(options.requested)) return { target: null, fallback: "notice", scroll: "center" };
      this.remember(options.requested);
      return { target: { id: options.requested }, fallback: null, scroll: "center" };
    }
    const none: HistoryFocusPlan = { target: null, fallback: null, scroll: null };
    if (!options.unobstructed) return none;
    const target = options.previous ?? (options.returning ? remembered : null);
    if (!target) return none;
    if (!options.available.has(target.id)) return { target: null, fallback: "heading", scroll: null };
    this.remember(target.id, target.action);
    return { target: { ...target }, fallback: null, scroll: options.returning ? "nearest" : null };
  }
}
