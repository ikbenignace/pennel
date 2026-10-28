export interface ChildContainer<T> {
  readonly children: ArrayLike<T>;
  insertBefore(child: T, before: T | null): unknown;
  removeChild(child: T): unknown;
}

/** Moving only out-of-place children preserves focus and text selection in unchanged rows. */
export function reconcileChildren<T>(parent: ChildContainer<T>, children: readonly T[]): void {
  const wanted = new Set(children);
  for (const child of Array.from(parent.children)) if (!wanted.has(child)) parent.removeChild(child);
  children.forEach((child, index) => {
    const current = parent.children[index];
    if (current !== child) parent.insertBefore(child, current ?? null);
  });
}

export class ListTypeahead {
  private prefix = "";
  private lastInput = 0;

  find(titles: readonly string[], selected: number, key: string, now: number, locale: string): number {
    if (!titles.length || key.length !== 1 || (!this.prefix && key === " ")) return -1;
    const normalize = (value: string) => value.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLocaleLowerCase(locale);
    const input = normalize(key);
    this.prefix = now - this.lastInput <= 750 ? this.prefix + input : input;
    this.lastInput = now;
    const repeated = [...this.prefix].every((character) => character === input);
    const prefix = repeated ? input : this.prefix;
    const start = repeated || prefix.length === 1 ? selected + 1 : Math.max(0, selected);
    for (let offset = 0; offset < titles.length; offset++) {
      const index = (Math.max(0, start) + offset) % titles.length;
      if (normalize(titles[index]).startsWith(prefix)) return index;
    }
    return -1;
  }
}

export function listKeyIndex(key: string, selected: number, count: number): number {
  if (!count) return -1;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  if (key === "ArrowDown") return Math.min(count - 1, selected + 1);
  if (key === "ArrowUp") return Math.max(0, selected - 1);
  return -1;
}
