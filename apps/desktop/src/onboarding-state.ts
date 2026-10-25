import type { OnboardingProgress, OnboardingStep, PermissionInfo, Settings, SetupReadiness } from "./api";

export const onboardingSteps: OnboardingStep[] = ["welcome", "engine", "permissions", "practice"];

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
