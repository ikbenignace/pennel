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
    case "connecting": return "Verbinden met Pennel Cloud…";
    case "connected":
      return status.last_request_ms === null
        ? "Verbonden. Claude kan je meetings op deze computer doorzoeken."
        : `Verbonden · laatste vraag van Claude ${ago(status.last_request_ms)}`;
    case "error": return "Geen verbinding met Pennel Cloud. Pennel probeert het zo opnieuw.";
  }
}

export function claudeInstructions(mcpUrl: string): { connectorUrl: string; command: string } {
  return { connectorUrl: mcpUrl, command: `claude mcp add --transport http pennel ${mcpUrl}` };
}
