import type { Capability, SetupReadiness } from "./api";

export interface ShortcutAccessHint { message: string; recoverable: boolean }

export function shortcutAccessHint(readiness: SetupReadiness, capability: Capability): ShortcutAccessHint | null {
  if (readiness.can_use_shortcut) return null;
  if (capability.status === "unsupported") return {
    message: capability.reason ?? "De globale sneltoets wordt hier niet ondersteund. Gebruik de knop om te dicteren.",
    recoverable: false,
  };
  if (!readiness.can_record && capability.status === "available") return null;
  return {
    message: capability.reason ?? "De sneltoets is nog niet beschikbaar. Je kunt de knop gebruiken om hier te dicteren.",
    recoverable: true,
  };
}
