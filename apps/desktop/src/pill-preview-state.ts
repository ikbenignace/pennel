import type { PillPreview } from "./api";

const segments = new Intl.Segmenter("nl", { granularity: "grapheme" });

export function previewTail(text: string, limit: number): string {
  const normalized = text.trim().replace(/\s+/gu, " ");
  const characters = Array.from(segments.segment(normalized), (part) => part.segment);
  return characters.length > limit ? `…${characters.slice(-limit).join("")}` : normalized;
}

export class PillPreviewState {
  snapshot: PillPreview | null = null;
  private limit: number;

  constructor(limit: number) { this.limit = limit; }

  receive(next: PillPreview | null): boolean {
    if (next === null) { this.snapshot = null; return true; }
    if (!next.session_id || !Number.isSafeInteger(next.revision) || next.revision < 0) return false;
    if (next.revision === 0 && next.session_id !== this.snapshot?.session_id) {
      this.snapshot = { ...next, text: "" };
      return true;
    }
    if (next.session_id !== this.snapshot?.session_id || next.revision <= this.snapshot.revision) return false;
    this.snapshot = { ...next, text: previewTail(next.text, this.limit) };
    return true;
  }

  restore(next: PillPreview | null | undefined) {
    this.snapshot = next?.session_id && Number.isSafeInteger(next.revision) && next.revision >= 0
      ? { ...next, text: previewTail(next.text, this.limit) } : null;
  }
}
