import type { MeetingRetranscriptionPlan, MeetingRetranscriptionStatus } from "./api";

export function retranscriptionControl(
  plan: MeetingRetranscriptionPlan | undefined,
  status: MeetingRetranscriptionStatus | null | undefined,
  recording: boolean,
  working: boolean,
): { disabled: boolean; reason: string | null } {
  if (recording || working || status?.running) return { disabled: true, reason: null };
  return { disabled: !plan?.available || plan.recognition.provider_kind !== "local", reason: plan?.reason ?? null };
}

export function retranscriptionPercent(progress: MeetingRetranscriptionStatus["progress"]): number | null {
  if (!Number.isFinite(progress.total) || progress.total <= 0 || !Number.isFinite(progress.completed)) return null;
  return Math.max(0, Math.min(100, Math.round(progress.completed / progress.total * 100)));
}
