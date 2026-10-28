export interface NoteSnapshot { baseline: string; draft: string; remote: string; dirty: boolean; conflict: boolean; saving: boolean; failed: boolean }
export class NoteSession {
  private state: NoteSnapshot;
  private listeners = new Set<() => void>();
  private pending: Promise<boolean> | null = null;
  private writing: string | null = null;
  private readonly persist: (notes: string, expected: string) => Promise<boolean>;
  constructor(initial: string, persist: (notes: string, expected: string) => Promise<boolean>) {
    this.persist = persist;
    this.state = { baseline: initial, draft: initial, remote: initial, dirty: false, conflict: false, saving: false, failed: false };
  }
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };
  private update(patch: Partial<NoteSnapshot>) {
    const next = { ...this.state, ...patch };
    next.dirty = next.draft !== next.baseline;
    next.conflict = next.dirty && next.remote !== next.baseline && next.remote !== this.writing;
    this.state = next;
    for (const listener of this.listeners) listener();
  }
  edit = (draft: string) => this.update({ draft, failed: false });
  adoptRemote = (remote: string) => {
    if (remote === this.state.remote) return;
    if (!this.state.dirty && !this.state.saving) this.update({ remote, baseline: remote, draft: remote });
    else this.update({ remote });
  };
  discard = () => this.update({ baseline: this.state.remote, draft: this.state.remote, failed: false });
  save = (): Promise<boolean> => {
    if (this.pending) return this.pending;
    this.pending = this.flush().finally(() => { this.pending = null; });
    return this.pending;
  };
  private async flush(): Promise<boolean> {
    while (this.state.dirty) {
      if (this.state.conflict) return false;
      const written = this.state.draft;
      const expected = this.state.baseline;
      this.writing = written;
      this.update({ saving: true, failed: false });
      let saved = false;
      try { saved = await this.persist(written, expected); } catch { saved = false; }
      const remote = this.state.remote;
      this.writing = null;
      if (!saved) { this.update({ saving: false, failed: true }); return false; }
      const nextRemote = remote === expected || remote === written ? written : remote;
      if (this.state.draft === written && nextRemote !== written) this.update({ baseline: nextRemote, draft: nextRemote, remote: nextRemote, saving: false });
      else this.update({ baseline: written, remote: nextRemote, saving: false });
    }
    return true;
  }
}
