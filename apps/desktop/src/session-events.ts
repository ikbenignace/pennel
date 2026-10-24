export class FrontendBarrier {
  private active: number | null = null;

  get locked(): boolean {
    return this.active !== null;
  }

  begin(requestId: number): void {
    this.active = requestId;
  }

  isCurrent(requestId: number): boolean {
    return this.active === requestId;
  }

  complete(requestId: number): boolean {
    if (!this.isCurrent(requestId)) return false;
    this.active = null;
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
