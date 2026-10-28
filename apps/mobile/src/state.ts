import type { NativeFailure, RuntimeClient, RuntimeSnapshot } from './domain.ts';
import { nativeFailure } from './errors.ts';

export interface RuntimeState { snapshot: RuntimeSnapshot | null; connecting: boolean; error: NativeFailure | null; busy: ReadonlySet<string> }

export function failure(error: unknown): NativeFailure {
  return nativeFailure(error);
}

export class RuntimeStore {
  readonly client: RuntimeClient;
  private state: RuntimeState = { snapshot: null, connecting: true, error: null, busy: new Set() };
  private listeners = new Set<() => void>();
  private nativeSubscription: (() => void) | null = null;
  private connection = 0;

  constructor(client: RuntimeClient) { this.client = client; }
  getSnapshot = (): RuntimeState => this.state;
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };

  private update(patch: Partial<RuntimeState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }

  accept = (snapshot: RuntimeSnapshot): void => {
    if (!Number.isSafeInteger(snapshot.revision) || snapshot.revision < 0) return;
    if (this.state.snapshot && snapshot.revision < this.state.snapshot.revision) return;
    this.update({ snapshot, connecting: false });
  };

  async connect(): Promise<void> {
    const connection = ++this.connection;
    this.nativeSubscription?.();
    this.update({ connecting: true, error: null });
    try {
      this.nativeSubscription = this.client.subscribe((snapshot) => { if (connection === this.connection) this.accept(snapshot); });
      const snapshot = await this.client.bootstrap();
      if (connection === this.connection) this.accept(snapshot);
    } catch (error) {
      if (connection === this.connection) this.update({ connecting: false, error: failure(error) });
    }
  }

  disconnect(): void { this.connection += 1; this.nativeSubscription?.(); this.nativeSubscription = null; }
  clearError = (): void => this.update({ error: null });
  async refresh(): Promise<void> {
    const connection = this.connection;
    try { const snapshot = await this.client.getSnapshot(); if (connection === this.connection) this.accept(snapshot); }
    catch (error) { if (connection === this.connection) this.update({ error: failure(error) }); }
  }

  async run<T>(key: string, method: string, payload?: Record<string, unknown>): Promise<T> {
    if (this.state.busy.has(key)) throw { code: 'operation_busy', message: 'This operation is already running.' };
    this.update({ busy: new Set([...this.state.busy, key]), error: null });
    try {
      const result = await this.client.invoke<T>(method, payload);
      await this.refresh();
      return result;
    } catch (error) {
      this.update({ error: failure(error) });
      throw error;
    } finally {
      const busy = new Set(this.state.busy); busy.delete(key); this.update({ busy });
    }
  }
}

export class LatestRequest {
  private revision = 0;
  async run<T>(request: () => Promise<T>, apply: (value: T) => void): Promise<void> {
    const revision = ++this.revision;
    try {
      const value = await request();
      if (revision === this.revision) apply(value);
    } catch (error) {
      if (revision === this.revision) throw error;
    }
  }
  cancel(): void { this.revision += 1; }
}
