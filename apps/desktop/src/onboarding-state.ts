import type { LocalModel, OnboardingProgress, OnboardingStep, PermissionInfo, Settings, SetupReadiness } from "./api";

export const onboardingSteps: OnboardingStep[] = ["welcome", "engine", "permissions", "practice", "voice"];

/** Readiness knows setup up to the practice; after a practice the saved step may be the optional voice step. */
export function resumeStep(ready: OnboardingStep, saved: OnboardingStep | undefined): OnboardingStep {
  return ready === "practice" && saved === "voice" ? "voice" : ready;
}

export function stepIndex(step: unknown): number {
  const index = onboardingSteps.indexOf(step as OnboardingStep);
  return index < 0 ? 0 : index;
}

export function onboardingProgressChange(current: OnboardingProgress, step: OnboardingStep, status?: "skipped" | "completed"): OnboardingProgress | null {
  const next: OnboardingProgress = { version: 1, resume_step: step,
    status: status === "completed" ? "completed" : current.status === "completed" ? "completed" : status ?? current.status };
  return current.version === next.version && current.resume_step === next.resume_step && current.status === next.status ? null : next;
}

export function permissionGranted(permissions: PermissionInfo[], kind: string): boolean {
  const status = permissions.find((permission) => permission.kind === kind)?.status;
  return status === "granted" || status === "not_required";
}

export function manualPracticeAllowed(readiness: SetupReadiness | null, permissions: PermissionInfo[], settings: Settings): boolean {
  if (readiness) return readiness.configured && readiness.can_record;
  const microphone = permissions.find((permission) => permission.kind === "microphone")?.status;
  const speech = permissions.find((permission) => permission.kind === "speech")?.status;
  const apple = settings.providers.find((provider) => provider.id === settings.dictation_stt.provider_id)?.kind === "apple_speech";
  return !["denied", "restricted", "unsupported"].includes(microphone ?? "unknown")
    && (!apple || speech === "granted") && !!settings.dictation_stt.model;
}

export function defaultModelId(models: LocalModel[], current: string): string {
  if (models.some((model) => model.id === current)) return current;
  return models.find((model) => model.recommended)?.id ?? models[0]?.id ?? current;
}

export function modelOptionLabel(model: LocalModel, formatSize: (bytes: number) => string): string {
  return [model.name, formatSize(model.size_bytes), model.recommended ? "Aanbevolen" : null].filter(Boolean).join(" · ");
}
