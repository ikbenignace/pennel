export type FrontendBarrierKind = "quit" | "recording" | "update" | "hide";

export class FrontendBarrier {
  private active: number | null = null;
  private activeKind: FrontendBarrierKind | null = null;

  get locked(): boolean {
    return this.active !== null;
  }

  get kind(): FrontendBarrierKind | null {
    return this.activeKind;
  }

  begin(requestId: number, kind: FrontendBarrierKind = "recording"): boolean {
    if (kind === "hide" && (this.activeKind === "quit" || this.activeKind === "update")) return false;
    this.active = requestId;
    this.activeKind = kind;
    return true;
  }

  async prepare(requestId: number, flush: () => Promise<void>, leave?: () => Promise<void>, kind: FrontendBarrierKind = "recording"): Promise<boolean> {
    if (!this.begin(requestId, kind)) return false;
    await flush();
    if (!this.isCurrent(requestId)) return false;
    await leave?.();
    return this.isCurrent(requestId);
  }

  isCurrent(requestId: number): boolean {
    return this.active === requestId;
  }

  complete(requestId: number, kind?: FrontendBarrierKind): boolean {
    if (!this.isCurrent(requestId) || (kind !== undefined && kind !== this.activeKind)) return false;
    this.active = null;
    this.activeKind = null;
    return true;
  }

  async resume(requestId: number, refresh: () => Promise<void>): Promise<boolean> {
    if (!this.isCurrent(requestId)) return false;
    await refresh();
    return this.complete(requestId);
  }
}

export function canShowFinished(
  status: { busy: boolean; session_id: string | null },
  completedSessionId: string,
  displayedSessionId: string | null,
): boolean {
  return !status.busy && (!status.session_id || status.session_id === completedSessionId)
    && (!displayedSessionId || displayedSessionId === completedSessionId);
}
