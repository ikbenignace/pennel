import { formatNumber, t } from "./i18n.ts";

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
  return formatNumber(value, { maximumFractionDigits: 1 });
}

function duration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (!hours) return t("account.usage.minutes", { count: formatNumber(rest) });
  return rest ? t("account.usage.hours_minutes", { hours: formatNumber(hours), minutes: formatNumber(rest) })
    : t("account.usage.hours", { count: formatNumber(hours) });
}

function millions(tokens: number): string {
  return t("account.usage.millions", { count: decimal(tokens / 1e6) });
}

function size(bytes: number): string {
  return bytes >= 1e9 ? `${decimal(bytes / 1e9)} GB` : `${formatNumber(Math.round(bytes / 1e6))} MB`;
}

export function formatUsage(account: UsageNumbers): { audio: string; tokens: string; storage: string } {
  return {
    audio: t("account.usage.audio", { used: duration(account.audio_seconds), limit: duration(account.limit_audio_seconds) }),
    tokens: t("account.usage.tokens", { used: millions(account.tokens), limit: millions(account.limit_tokens) }),
    storage: t("account.usage.storage", { used: size(account.storage_bytes), limit: size(account.limit_storage_bytes) }),
  };
}

export function relativeTime(thenMs: number, nowMs: number): string {
  const minutes = Math.max(0, Math.floor((nowMs - thenMs) / 60000));
  if (minutes < 1) return t("account.relative.now");
  if (minutes < 60) return t("account.relative.minutes", { count: formatNumber(minutes) });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("account.relative.hours", { count: formatNumber(hours) });
  const days = Math.floor(hours / 24);
  return t(days === 1 ? "account.relative.day" : "account.relative.days", { count: formatNumber(days) });
}

export function copyStatusText(failure: string | null): string {
  return failure ?? t("account.copied");
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

type ReportView = { status: { kind: string }; brake: boolean; last_success_ms: number | null; uploaded: number; downloaded: number; conflicts: number };

const LAYOUT_KINDS = ["signed_out", "off", "needs_setup", "needs_recovery_code", "vault_reset"];

function layoutOf(report: ReportView): string {
  return LAYOUT_KINDS.includes(report.status.kind) ? report.status.kind : "active";
}

/**
 * What a sync-changed event needs from the account tab: nothing, only the status block, or everything.
 * A half-typed device name is never thrown away, except when the account is signed out underneath it.
 */
export function planSyncRedraw(prev: ReportView | null, next: ReportView, wasSyncing: boolean, editing: boolean): "skip" | "status" | "full" {
  if (!prev) return "full";
  if (layoutOf(prev) !== layoutOf(next)) return next.status.kind === "signed_out" || !editing ? "full" : "status";
  if (JSON.stringify(prev) === JSON.stringify(next)) return wasSyncing ? "status" : "skip";
  return "status";
}
