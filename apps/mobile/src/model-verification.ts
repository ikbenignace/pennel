import type { NativeFailure, RuntimeSnapshot } from './domain.ts';
import { nativeFailure } from './errors.ts';

export type VerificationTask = 'dictation_stt' | 'meeting_stt' | 'llm';
export interface VerificationSelection {
  task: VerificationTask;
  provider_id: string;
  model: string;
  provider_kind: string;
  base_url: string;
  language: string | null;
}
export interface ModelTestResult { ok: boolean; message: string }
export interface VerificationState {
  running: boolean;
  changed: boolean;
  result: ModelTestResult | null;
  error: NativeFailure | null;
}

function identity(value: VerificationSelection | null): string {
  return value ? JSON.stringify([value.task, value.provider_id, value.model, value.provider_kind, value.base_url, value.language]) : '';
}

export function sameVerificationSelection(first: VerificationSelection | null, second: VerificationSelection | null): boolean { return identity(first) === identity(second); }

export function selectedVerification(snapshot: RuntimeSnapshot | null, task: VerificationTask): VerificationSelection | null {
  if (!snapshot) return null;
  const choice = snapshot.settings[task];
  const provider = snapshot.settings.providers.find((provider) => provider.id === choice.provider_id);
  const kind = snapshot.kinds.find((kind) => kind.kind === provider?.kind);
  if (!provider || !choice.model.trim() || !kind || kind.availability.status !== 'available' || !(task === 'llm' ? kind.llm : kind.stt)) return null;
  if (provider.kind === 'pennel_cloud' ? !snapshot.cloud.signed_in : kind.requires_key && !snapshot.keys[provider.id]) return null;
  return { task, provider_id: provider.id, model: choice.model, provider_kind: provider.kind, base_url: provider.base_url, language: snapshot.settings.language };
}

export function verificationPayload(selection: VerificationSelection) {
  return { provider_id: selection.provider_id, task: selection.task, expected_model: selection.model, verify_task: true };
}

export class ModelVerification {
  private state: VerificationState = { running: false, changed: false, result: null, error: null };
  private selection: VerificationSelection | null = null;
  private generation = 0;
  private pending: Promise<void> | null = null;
  private listeners = new Set<() => void>();
  private readonly request: (selection: VerificationSelection) => Promise<ModelTestResult>;

  constructor(request: (selection: VerificationSelection) => Promise<ModelTestResult>) { this.request = request; }
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };
  private update(patch: Partial<VerificationState>) { this.state = { ...this.state, ...patch }; for (const listener of this.listeners) listener(); }

  configure(selection: VerificationSelection | null): void {
    if (identity(selection) === identity(this.selection)) return;
    this.selection = selection ? { ...selection } : null;
    this.invalidate();
  }

  invalidate = (): void => {
    this.generation += 1;
    this.update({ result: null, error: null, changed: this.state.running });
  };

  verify = (): Promise<void> => {
    if (this.pending) return this.pending;
    if (!this.selection) return Promise.resolve();
    const selected = { ...this.selection };
    const generation = this.generation;
    this.update({ running: true, changed: false, result: null, error: null });
    this.pending = this.perform(selected, generation).finally(() => { this.pending = null; });
    return this.pending;
  };

  private async perform(selected: VerificationSelection, generation: number): Promise<void> {
    try {
      const result = await this.request(selected);
      if (generation === this.generation && identity(selected) === identity(this.selection)) {
        this.update({ result: { ok: result?.ok === true, message: typeof result?.message === 'string' ? result.message : '' } });
      }
    } catch (error) {
      if (generation === this.generation && identity(selected) === identity(this.selection)) this.update({ error: nativeFailure(error) });
    } finally { this.update({ running: false, changed: false }); }
  }
}
