import { practiceSucceeded } from './workflow-state.ts';

export interface PracticeSnapshot {
  phase: string;
  text: string | null;
  error: string | null;
  session_id?: string | null;
  result_session_id?: string | null;
  practice?: boolean;
}
export interface PracticeState { sessionId: string | null; pending: boolean; result: string | null }

export class PracticeSession {
  private state: PracticeState = { sessionId: null, pending: false, result: null };
  private attempt = 0;
  private previousId: string | null = null;
  private listeners = new Set<() => void>();

  getSnapshot = (): PracticeState => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };
  private update(patch: Partial<PracticeState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  begin(snapshot: PracticeSnapshot): number {
    this.previousId = snapshot.session_id ?? null;
    this.update({ sessionId: null, pending: true, result: null });
    return ++this.attempt;
  }
  acceptStart(attempt: number, snapshot: PracticeSnapshot): boolean {
    if (attempt !== this.attempt) return false;
    if (!snapshot.practice || !snapshot.session_id || snapshot.session_id === this.previousId) {
      this.rejectStart(attempt);
      return false;
    }
    this.update({ sessionId: snapshot.session_id, pending: false });
    this.observe(snapshot);
    return true;
  }
  rejectStart(attempt: number) {
    if (attempt === this.attempt) this.update({ sessionId: null, pending: false });
  }
  owns(snapshot: PracticeSnapshot): boolean {
    return Boolean(this.state.sessionId && snapshot.practice && snapshot.session_id === this.state.sessionId);
  }
  command(snapshot: PracticeSnapshot): { session_id: string } | null {
    return this.owns(snapshot) && snapshot.phase !== 'idle' ? { session_id: this.state.sessionId! } : null;
  }
  recoveryCommand(snapshot: PracticeSnapshot): { session_id: string } | null {
    return !this.state.pending && !this.owns(snapshot) && snapshot.practice && snapshot.session_id && snapshot.phase !== 'idle' ? { session_id: snapshot.session_id } : null;
  }
  observe(snapshot: PracticeSnapshot) {
    if (!this.state.result && practiceSucceeded(this.state.sessionId, snapshot)) this.update({ result: snapshot.text });
  }
  abandon() {
    this.attempt += 1;
    this.update({ sessionId: null, pending: false, result: null });
  }
}
