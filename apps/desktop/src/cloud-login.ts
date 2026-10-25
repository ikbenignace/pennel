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

export function copyStatusText(failure: string | null): string {
  return failure ?? "Gekopieerd";
}

export interface AccountUi {
  login:
    | { kind: "idle" }
    | { kind: "code"; start: { user_code: string; verification_uri: string; device_code: string; interval_secs: number } }
    | { kind: "expired" };
  offerCloud: boolean;
  shownCode: string | null;
  askCode: boolean;
  lostCode: boolean;
  codeDraft: string;
  notice: string | null;
  syncing: boolean;
}

export function initialAccountUi(): AccountUi {
  return { login: { kind: "idle" }, offerCloud: false, shownCode: null, askCode: false, lostCode: false, codeDraft: "", notice: null, syncing: false };
}

/** After logout or account deletion nothing of the signed-in session may linger on screen. */
export function afterSignOut(): AccountUi {
  return initialAccountUi();
}

export function accountScreenFor(errorCode: string | null): "signed_out" | "signed_in" {
  return errorCode === "not_signed_in" ? "signed_out" : "signed_in";
}

/** Only an explicit "false" counts: the flag is absent on devices that never created a code. */
export function recoveryNeedsConfirmation(state: { signedIn: boolean; syncOn: boolean; confirmed: boolean | null }): boolean {
  return state.signedIn && state.syncOn && state.confirmed === false;
}
