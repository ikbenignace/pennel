import type { ProviderKindInfo, Settings } from './domain.ts';

export type OnboardingGoal = 'dictation' | 'meeting';
export type OnboardingStage = 'welcome' | 'engine' | 'permissions' | 'practice';

export function restoreOnboardingStep(step: string | undefined): OnboardingStage {
  if (step === 'engine' || step === 'permissions' || step === 'practice') return step;
  return step === 'voice' ? 'practice' : 'welcome';
}

export function speechReadiness(settings: Pick<Settings, 'dictation_stt' | 'meeting_stt' | 'providers'>, kinds: readonly ProviderKindInfo[], keys: Record<string, boolean>, goal: OnboardingGoal): boolean {
  const selection = goal === 'meeting' ? settings.meeting_stt : settings.dictation_stt;
  const provider = settings.providers.find((provider) => provider.id === selection.provider_id);
  const kind = kinds.find((kind) => kind.kind === provider?.kind);
  return Boolean(provider && kind?.stt && kind.availability.status === 'available' && selection.model.trim() && (!kind.requires_key || keys[provider.id]));
}

export function practiceSucceeded(ownedSession: string | null, dictation: { phase: string; text: string | null; error: string | null; practice?: boolean; session_id?: string | null; result_session_id?: string | null }): boolean {
  return Boolean(ownedSession && dictation.practice && (!dictation.session_id || dictation.session_id === ownedSession) && dictation.result_session_id === ownedSession && dictation.phase === 'idle' && !dictation.error && dictation.text?.trim());
}

export function recordingPresentation(phase: string | undefined, failedSave = false) {
  const capturing = phase === 'recording';
  const canStop = capturing || phase === 'interrupted';
  const busy = phase === 'starting' || phase === 'finishing';
  const status = phase === 'interrupted' ? 'paused' : capturing ? 'recording' : phase === 'finishing' ? 'finishing' : phase === 'starting' ? 'starting' : failedSave ? 'saveFailed' : 'stopped';
  return { capturing, canStop, busy, status } as const;
}

export function summaryHasHeading(summary: string | null): boolean {
  return /^\s*#{1,6}\s+\S/.test(summary ?? '');
}
