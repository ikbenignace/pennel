import type { ModelChoice, ProviderAccount, Settings } from "./api";

export type ProcessingTask = "dictation" | "meeting";
export interface ProcessingStage { label: string; destination: string }

const cliKinds = new Set(["claude_code", "codex", "open_code", "copilot", "cursor_agent", "gemini_cli"]);

export function providerDestination(account: ProviderAccount | undefined): string {
  if (!account) return "Nog niet ingesteld";
  if (account.kind === "local") return "Op deze computer";
  if (account.kind === "apple_speech") return "Op deze Mac (Apple-dicteren)";
  if (account.kind === "apple_intelligence") return "Op deze Mac (Apple Intelligence)";
  if (cliKinds.has(account.kind)) return `Via ${account.name}; bestemming hangt af van je CLI-instellingen`;
  try {
    const url = new URL(account.base_url);
    if (["http:", "https:"].includes(url.protocol)) return `${account.name} (${url.host})`;
  } catch { /* An unfinished address remains editable without making an offline claim. */ }
  return `${account.name} (gekozen server)`;
}

export function choiceDestination(settings: Settings, choice: ModelChoice): string {
  return providerDestination(settings.providers.find((account) => account.id === choice.provider_id));
}

export function processingSummary(settings: Settings, task: ProcessingTask): ProcessingStage[] {
  return [
    { label: "Audio", destination: choiceDestination(settings, task === "meeting" ? settings.meeting_stt : settings.dictation_stt) },
    { label: task === "meeting" ? "Samenvatten en vragen" : "Tekst", destination: task === "dictation" && settings.mode === "fast"
      ? "Geen opkuisbeurt (Snel)" : choiceDestination(settings, settings.llm) },
  ];
}
