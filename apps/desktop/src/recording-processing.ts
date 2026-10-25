import type { MeetingStatus } from "./api";
import type { ProcessingStage } from "./processing-state";

type FrozenRecognition = NonNullable<MeetingStatus["recognition"]>;

export function frozenRecognitionLabel(recognition?: FrozenRecognition): string {
  if (!recognition) return "Opnameconfiguratie niet beschikbaar";
  const place = recognition.provider_kind === "local" ? `Op deze computer · ${recognition.provider_name}`
    : recognition.provider_kind === "apple_speech" ? `Op deze Mac · ${recognition.provider_name}`
    : `${recognition.provider_name} (${recognition.provider_kind ? "gekozen provider" : "bestemming niet vastgesteld"})`;
  const languages: Record<string, string> = { nl: "Nederlands", en: "Engels", fr: "Frans", de: "Duits" };
  const language = recognition.language ? languages[recognition.language] ?? recognition.language : "Automatisch";
  return [place, recognition.model || "Model niet vermeld", language].join(" · ");
}

export function meetingProcessingSummary(stages: ProcessingStage[], recognition?: FrozenRecognition, active = false): ProcessingStage[] {
  return [
    { label: "Audio", destination: active ? frozenRecognitionLabel(recognition) : stages[0]?.destination ?? "Nog niet ingesteld" },
    { label: "Tekst (optioneel)", destination: `${stages[1]?.destination ?? "Nog niet ingesteld"} · alleen bij samenvatten of vragen, met de dan gekozen provider` },
  ];
}

export function processingLabel(stages: ProcessingStage[]): string {
  return stages.map((stage) => `${stage.label} → ${stage.destination}`).join(". ");
}

export function retainedMeetingFailure(status: Pick<MeetingStatus, "busy" | "phase" | "id" | "session_id" | "error" | "can_retry_persistence" | "persistence_error" | "terminal_error">): boolean {
  return !status.busy && !!status.id && !!status.session_id
    && !!(status.can_retry_persistence || status.persistence_error || status.terminal_error || status.phase === "error" && status.error);
}
