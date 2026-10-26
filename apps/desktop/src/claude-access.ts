import { t } from "./i18n.ts";

export type ClaudeStatus =
  | { kind: "off" }
  | { kind: "connecting" }
  | { kind: "connected"; last_request_ms: number | null }
  | { kind: "error"; message: string };

/** `ago` formats a past moment, e.g. `relativeTime` from cloud-login. */
export function claudeStatusText(status: ClaudeStatus, enabled: boolean, ago: (thenMs: number) => string): string | null {
  if (!enabled) return null;
  switch (status.kind) {
    case "off":
    case "connecting": return t("account.claude.connecting");
    case "connected":
      return status.last_request_ms === null
        ? t("account.claude.connected")
        : t("account.claude.last_request", { time: ago(status.last_request_ms) });
    case "error": return t("account.claude.error");
  }
}

export function claudeInstructions(mcpUrl: string): { connectorUrl: string; command: string } {
  return { connectorUrl: mcpUrl, command: `claude mcp add --transport http pennel ${mcpUrl}` };
}
