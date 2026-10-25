export type LoginPoll =
  | { kind: "pending" }
  | { kind: "slow_down" }
  | { kind: "expired" }
  | { kind: "done"; email: string };

export interface PollOptions {
  intervalMs: number;
  expiresAtMs: number;
  now: () => number;
  sleep: (ms: number) => Promise<void>;
  cancelled?: () => boolean;
}

const SLOW_DOWN_MS = 5000;

export async function pollLogin(
  poll: () => Promise<LoginPoll>,
  options: PollOptions,
): Promise<"done" | "expired" | "cancelled"> {
  let interval = options.intervalMs;
  const cancelled = options.cancelled ?? (() => false);
  for (;;) {
    if (cancelled()) return "cancelled";
    if (options.now() >= options.expiresAtMs) return "expired";
    await options.sleep(interval);
    if (cancelled()) return "cancelled";
    if (options.now() >= options.expiresAtMs) return "expired";
    const result = await poll();
    if (cancelled()) return "cancelled";
    if (result.kind === "done") return "done";
    if (result.kind === "expired") return "expired";
    if (result.kind === "slow_down") interval += SLOW_DOWN_MS;
  }
}

export interface UsageNumbers {
  audio_seconds: number;
  tokens: number;
  storage_bytes: number;
  limit_audio_seconds: number;
  limit_tokens: number;
  limit_storage_bytes: number;
}

function decimal(value: number): string {
  return String(Math.round(value * 10) / 10).replace(".", ",");
}

function duration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (!hours) return `${rest} min`;
  return rest ? `${hours} u ${rest} min` : `${hours} u`;
}

function millions(tokens: number): string {
  return tokens === 0 ? "0 mln" : `${decimal(tokens / 1e6)} mln`;
}

function size(bytes: number): string {
  return bytes >= 1e9 ? `${decimal(bytes / 1e9)} GB` : `${Math.round(bytes / 1e6)} MB`;
}

export function formatUsage(account: UsageNumbers): { audio: string; tokens: string; storage: string } {
  return {
    audio: `${duration(account.audio_seconds)} van ${duration(account.limit_audio_seconds)}`,
    tokens: `${millions(account.tokens)} van ${millions(account.limit_tokens)} tokens`,
    storage: `${size(account.storage_bytes)} van ${size(account.limit_storage_bytes)}`,
  };
}

export function relativeTime(thenMs: number, nowMs: number): string {
  const minutes = Math.max(0, Math.floor((nowMs - thenMs) / 60000));
  if (minutes < 1) return "zojuist";
  if (minutes < 60) return `${minutes} min geleden`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} u geleden`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "1 dag geleden" : `${days} dagen geleden`;
}

export const MASS_DELETE_MESSAGE = "Veel meetings zijn verdwenen uit je map. Sync heeft niets verwijderd; bevestig in Instellingen → Account als dat de bedoeling was.";

export function isMassDeleteBrake(status: { kind: string; message?: string }): boolean {
  return status.kind === "error" && status.message === MASS_DELETE_MESSAGE;
}
