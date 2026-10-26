import { t, formatDateTime, formatNumber, localizeBackendText } from "./i18n.ts";
import type { ModelState, SpeakerRecognition, VoiceProfile } from "./api";

/** Catalogue sizes of the two speaker models, used until the first progress event reports a total. */
export const speakerModelSizes: Record<string, number> = {
  "pyannote-segmentation-3.0": 5_992_913,
  "3dspeaker-campplus-zh-en-advanced": 28_281_164,
};

export interface DownloadProgress { downloaded: number; total: number }

export function combinedProgress(ids: string[], states: Record<string, ModelState | undefined>, progress: Record<string, DownloadProgress | undefined>): DownloadProgress {
  let downloaded = 0;
  let total = 0;
  for (const id of ids) {
    const size = progress[id]?.total || speakerModelSizes[id] || 0;
    const state = states[id];
    total += size;
    downloaded += state?.kind === "downloaded" ? size : Math.min(size, progress[id]?.downloaded ?? (state?.kind === "partial" ? state.bytes : 0));
  }
  return { downloaded, total };
}

export function speakerModelsReady(ids: string[], states: Record<string, ModelState | undefined>): boolean {
  return ids.length > 0 && ids.every((id) => states[id]?.kind === "downloaded");
}

export function percent(progress: DownloadProgress): number {
  return progress.total ? Math.min(100, Math.round(progress.downloaded / progress.total * 100)) : 0;
}

export type Enrollment =
  | { phase: "idle" }
  | { phase: "starting" }
  | { phase: "recording"; seconds: number }
  | { phase: "working" }
  | { phase: "done"; profile: VoiceProfile }
  | { phase: "error"; message: string };

export type EnrollmentEvent =
  | { kind: "request" }
  | { kind: "start" }
  | { kind: "tick"; seconds: number }
  | { kind: "finish" }
  | { kind: "succeeded"; profile: VoiceProfile }
  | { kind: "failed"; message: string }
  | { kind: "cancel" };

export function enrollmentStep(state: Enrollment, event: EnrollmentEvent): Enrollment {
  switch (event.kind) {
    case "request": return state.phase === "starting" || state.phase === "recording" || state.phase === "working" ? state : { phase: "starting" };
    case "start": return state.phase === "recording" || state.phase === "working" ? state : { phase: "recording", seconds: 0 };
    case "tick": return state.phase === "recording" ? { phase: "recording", seconds: Math.max(state.seconds, event.seconds) } : state;
    case "finish": return state.phase === "recording" ? { phase: "working" } : state;
    case "succeeded": return state.phase === "working" ? { phase: "done", profile: event.profile } : state;
    case "failed": return { phase: "error", message: event.message };
    case "cancel": return state.phase === "working" ? state : { phase: "idle" };
  }
}

export function enrollmentButton(state: Enrollment, clock: (seconds: number) => string): string {
  switch (state.phase) {
    case "starting": return t("settings.voice.starting_button");
    case "recording": return t("settings.voice.stop_button", { time: clock(state.seconds) });
    case "working": return t("settings.voice.working_button");
    case "done": return t("settings.voice.record_again");
    case "error": return t("settings.voice.retry");
    default: return t("settings.voice.start");
  }
}

export function enrollmentStatus(state: Enrollment): string {
  switch (state.phase) {
    case "starting": return t("settings.voice.starting_status");
    case "recording": return t("settings.voice.recording_status");
    case "working": return t("settings.voice.working_status");
    case "done": return t("settings.voice.done_status");
    case "error": return localizeBackendText(state.message);
    default: return t("settings.voice.idle_status");
  }
}

export function enrollmentTone(state: Enrollment): "neutral" | "success" | "error" {
  return state.phase === "done" ? "success" : state.phase === "error" ? "error" : "neutral";
}

export function trainedLabel(profile: VoiceProfile | null): string {
  if (!profile) return t("settings.voice.untrained");
  const date = new Date(profile.created_iso);
  return Number.isNaN(date.valueOf()) ? t("settings.voice.trained") : t("settings.voice.trained_at", { date: formatDateTime(date, { dateStyle: "medium" }) });
}

export function recognitionSummary(result: SpeakerRecognition): string {
  const parts = [result.voices ? t(result.voices === 1 ? "settings.voice.found_one" : "settings.voice.found_many", { count: formatNumber(result.voices) }) : t("settings.voice.found_none")];
  if (result.named) parts.push(t("settings.voice.named", { count: formatNumber(result.named) }));
  if (result.me_found) parts.push(t("settings.voice.me_found"));
  return parts.join(" · ");
}
