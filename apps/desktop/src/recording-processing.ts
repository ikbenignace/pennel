import type { MeetingStatus } from "./api";
import type { ProcessingStage } from "./processing-state";
import { t, uiLocale } from "./i18n.ts";

type FrozenRecognition = NonNullable<MeetingStatus["recognition"]>;

function speechLanguageName(language: string | null): string {
  if (!language) return t("common.automatic");
  try {
    const name = new Intl.DisplayNames([uiLocale()], { type: "language" }).of(language) ?? language;
    return name.charAt(0).toLocaleUpperCase(uiLocale()) + name.slice(1);
  } catch { return language; }
}

export function frozenRecognitionLabel(recognition?: FrozenRecognition): string {
  if (!recognition) return t("dictation.processing.unavailable");
  const provider = recognition.provider_name;
  const place = recognition.provider_kind === "local" ? t("dictation.processing.local_provider", { provider })
    : recognition.provider_kind === "apple_speech" ? t("dictation.processing.mac_provider", { provider })
    : recognition.provider_kind ? t("dictation.processing.provider", { provider }) : t("dictation.processing.unknown_provider", { provider });
  return [place, recognition.model || t("dictation.processing.no_model"), speechLanguageName(recognition.language)].join(" · ");
}

export function meetingProcessingSummary(stages: ProcessingStage[], recognition?: FrozenRecognition, active = false): ProcessingStage[] {
  return [
    { label: t("dictation.processing.audio"), destination: active ? frozenRecognitionLabel(recognition) : stages[0]?.destination ?? t("dictation.processing.not_configured") },
    { label: t("dictation.processing.optional_text"), destination: t("dictation.processing.optional_destination", { provider: stages[1]?.destination ?? t("dictation.processing.not_configured") }) },
  ];
}

export function processingLabel(stages: ProcessingStage[]): string {
  return stages.map((stage) => `${stage.label} → ${stage.destination}`).join(". ");
}

export function retainedMeetingFailure(status: Pick<MeetingStatus, "busy" | "phase" | "id" | "session_id" | "error" | "can_retry_persistence" | "persistence_error" | "terminal_error">): boolean {
  return !status.busy && !!status.id && !!status.session_id
    && !!(status.can_retry_persistence || status.persistence_error || status.terminal_error || status.phase === "error" && status.error);
}
