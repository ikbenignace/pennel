/** Keeps late account operations from restoring secrets or status into a newer account view. */
export class AccountActionEpoch {
  private identity: string | null | undefined;
  private epoch = 0;
  private operation = 0;

  observe(identity: string | null): boolean {
    if (this.identity === undefined) { this.identity = identity; return false; }
    if (this.identity === identity) return false;
    this.identity = identity;
    this.invalidate();
    return true;
  }

  invalidate(): void { this.epoch++; this.operation++; }

  captureIdentity(): () => boolean {
    const epoch = this.epoch;
    return () => this.epoch === epoch;
  }

  begin(): () => boolean {
    const epoch = this.epoch;
    const operation = ++this.operation;
    return () => this.epoch === epoch && this.operation === operation;
  }
}
