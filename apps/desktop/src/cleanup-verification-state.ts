import type { CleanupVerificationAvailability, Settings } from "./api";
import { t } from "./i18n.ts";

export function cleanupVerificationControl(settings: Settings, status?: CleanupVerificationAvailability | null): { enabled: boolean; disabled: boolean } {
  const enabled = settings.cleanup_verification_enabled === true;
  return { enabled, disabled: !enabled && (settings.mode !== "clean" || status?.available !== true) };
}

export function cleanupVerificationReason(status?: CleanupVerificationAvailability | null): string {
  const code = status?.reason_code ?? "unqualified";
  switch (code) {
    case "unconfigured": return t("dictation.verification.unconfigured");
    case "mutation_in_progress": return t("dictation.verification.mutation_in_progress");
    case "configuration_changed": return t("dictation.verification.configuration_changed");
    case "disabled":
    case "unqualified": return t("dictation.verification.unqualified");
    default: return t("dictation.verification.unavailable");
  }
}

export function cleanupVerificationDestination(status?: CleanupVerificationAvailability | null): string {
  if (!status?.endpoint) return t("dictation.verification.none");
  try {
    const url = new URL(status.endpoint);
    if (!["https:", "http:"].includes(url.protocol)) return t("dictation.verification.none");
    const host = url.hostname.replace(/^\[|\]$/g, "").toLowerCase();
    if (host === "localhost" || host === "::1" || /^127\./.test(host)) return t("dictation.verification.current_device", { host: url.host });
    return url.host;
  } catch {
    return t("dictation.verification.none");
  }
}

export function cleanupVerificationStage(settings: Settings, status?: CleanupVerificationAvailability | null, frozen = false): { label: string; destination: string } | null {
  if ((!frozen && settings.mode === "fast") || status?.reason_code === "disabled" || !(frozen ? status?.enabled : settings.cleanup_verification_enabled)) return null;
  return {
    label: t("dictation.verification.title"),
    destination: status?.available ? cleanupVerificationDestination(status) : t("dictation.verification.fallback"),
  };
}

export function cleanupVerificationCurrent(status: CleanupVerificationAvailability): string {
  const state = !status.enabled || status.reason_code === "disabled" ? t("dictation.verification.inactive") : status.available
    ? cleanupVerificationDestination(status) : `${t("dictation.verification.fallback")}. ${cleanupVerificationReason(status)}`;
  return t("dictation.verification.current_state", { state });
}
