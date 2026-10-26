import { t, localizeBackendText } from "./i18n.ts";
import type { Capability, SetupReadiness } from "./api";

export interface ShortcutAccessHint { message: string; recoverable: boolean }

export function shortcutAccessHint(readiness: SetupReadiness, capability: Capability): ShortcutAccessHint | null {
  if (readiness.can_use_shortcut) return null;
  if (capability.status === "unsupported") return {
    message: capability.reason ? localizeBackendText(capability.reason) : t("dictation.shortcut.unsupported"),
    recoverable: false,
  };
  if (!readiness.can_record && capability.status === "available") return null;
  return {
    message: capability.reason ? localizeBackendText(capability.reason) : t("dictation.shortcut.not_ready"),
    recoverable: true,
  };
}
