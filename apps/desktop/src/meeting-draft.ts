export class MeetingDraft {
  sessionId: string | null = null;
  value = "";
  dirty = false;

  sync(sessionId: string | null, value: string): void {
    if (this.sessionId !== sessionId) {
      this.sessionId = sessionId;
      this.value = value;
      this.dirty = false;
    } else if (!this.dirty) {
      this.value = value;
    }
  }

  edit(value: string): void {
    this.value = value;
    this.dirty = true;
  }

  acknowledge(sessionId: string | null, value: string): void {
    if (sessionId === this.sessionId && value === this.value) this.dirty = false;
  }

  async persist(write: (value: string) => Promise<void>): Promise<void> {
    if (!this.dirty) return;
    const sessionId = this.sessionId;
    const value = this.value;
    await write(value);
    this.acknowledge(sessionId, value);
  }
}
