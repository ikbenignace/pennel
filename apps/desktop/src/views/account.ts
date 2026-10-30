import { api, type ClaudeAccessView, type ClaudeStatus, type CloudAccount, type CloudDevice, type Settings, type SyncReport } from "../api";
import { claudeInstructions, claudeStatusText } from "../claude-access";
import { accountScreenFor, afterSignOut, copyStatusText, formatUsage, initialAccountUi, planSyncRedraw, pollLogin, recoveryNeedsConfirmation, relativeTime, type LoginPoll } from "../cloud-login";
import { controlButton, textField, confirmDialog, errorCode, errorText, h, section, toggle } from "../dom";
import type { Context } from "../main";
import { bindLocalization, liveText, l, localizeBackendText, t, uiLocale, type UiText } from "../i18n";

const LOGIN_LIFETIME_MS = 15 * 60 * 1000;
const SYNC_NOW_TIMEOUT_MS = 60 * 1000;
const CONFIRMED_KEY = "pennel.recoveryCodeConfirmed";

function confirmedFlag(): boolean | null {
  const value = localStorage.getItem(CONFIRMED_KEY);
  return value === null ? null : value === "true";
}

function setConfirmed(value: boolean) { localStorage.setItem(CONFIRMED_KEY, String(value)); }

function showCode(code: string) {
  ui.shownCode = code;
  setConfirmed(false);
}

function heading() { return h("h1", {}, "Pennel Cloud"); }

function plainSection(...children: (Node | null | false)[]) {
  return h("section", { class: "pane-section" }, ...children);
}

let ui = initialAccountUi();
let loginRun = 0;
let syncingTimer: number | undefined;
let redraw: (() => void) | null = null;
let listening = false;
let shownReport: SyncReport | null = null;
let syncBlock: { element: HTMLElement; build: (report: SyncReport) => HTMLElement } | null = null;
let claudeShown: { line: HTMLElement; enabled: boolean; status: ClaudeStatus } | null = null;
let listeningClaude = false;

function update() { redraw?.(); }

function renamingDevice(): boolean {
  return [...document.querySelectorAll<HTMLInputElement>("input.device-name")]
    .some((input) => input === document.activeElement || input.value !== input.dataset.original);
}

function listenForSync() {
  if (listening) return;
  listening = true;
  void api.onSyncChanged((report) => {
    const wasSyncing = ui.syncing;
    ui.syncing = false;
    window.clearTimeout(syncingTimer);
    const plan = planSyncRedraw(shownReport, report, wasSyncing, renamingDevice());
    if (plan === "skip") return;
    if (plan === "status" && syncBlock?.element.isConnected) {
      shownReport = report;
      const next = syncBlock.build(report);
      syncBlock.element.replaceWith(next);
      syncBlock.element = next;
      return;
    }
    update();
  }).catch(() => { listening = false; });
}

function claudeLine(status: ClaudeStatus, enabled: boolean): string | null {
  return claudeStatusText(status, enabled, (then) => relativeTime(then, Date.now()));
}

function listenForClaude() {
  if (listeningClaude) return;
  listeningClaude = true;
  void api.onClaudeAccessChanged((status) => {
    if (!claudeShown?.line.isConnected) return;
    claudeShown.status = status;
    const text = claudeLine(status, claudeShown.enabled);
    claudeShown.line.textContent = text ?? "";
    claudeShown.line.className = status.kind === "error" ? "error" : "muted";
  }).catch(() => { listeningClaude = false; });
}

async function beginLogin(ctx: Context) {
  const run = ++loginRun;
  ui.notice = null;
  try {
    const start = await api.cloudLoginStart();
    if (run !== loginRun) return;
    ui.login = { kind: "code", start };
    update();
    await api.openUrl(start.verification_uri).catch(ctx.report);
    const outcome = await pollLogin(async (): Promise<LoginPoll> => {
      try { return await api.cloudLoginPoll(start.device_code); }
      catch (error) {
        if (errorCode(error) === "error") return { kind: "pending" };
        throw error;
      }
    }, {
      intervalMs: Math.max(1, start.interval_secs) * 1000,
      expiresAtMs: Date.now() + LOGIN_LIFETIME_MS,
      now: Date.now,
      sleep: (ms) => new Promise((resolve) => window.setTimeout(resolve, ms)),
      cancelled: () => run !== loginRun,
    });
    if (outcome === "cancelled") return;
    if (outcome === "expired") { ui.login = { kind: "expired" }; update(); return; }
    ui.login = { kind: "idle" };
    ui.offerCloud = true;
    await ctx.refresh();
  } catch (error) {
    if (run !== loginRun) return;
    ui.login = { kind: "idle" };
    ui.notice = errorText(error);
    update();
  }
}

function cancelLogin() {
  loginRun++;
  ui.login = { kind: "idle" };
  update();
}

function resetLocalState() {
  loginRun++;
  ui = afterSignOut();
  localStorage.removeItem(CONFIRMED_KEY);
}

export async function accountView(ctx: Context): Promise<HTMLElement> {
  listenForSync();
  listenForClaude();
  const mine = ctx.rerender;
  redraw = mine;
  ctx.onDispose(() => { if (redraw === mine) redraw = null; });

  syncBlock = null;
  shownReport = null;
  if (ui.login.kind !== "idle") return signedOut(ctx);

  let account: CloudAccount | null = null;
  let accountError: unknown = null;
  try { account = await api.cloudAccount(); }
  catch (error) {
    if (accountScreenFor(errorCode(error)) === "signed_out") return signedOut(ctx);
    accountError = error;
  }
  const [report, devices, warning, claude] = await Promise.all([
    api.cloudSyncLast(),
    account ? api.cloudDevices().catch(() => null) : Promise.resolve(null),
    api.cloudFolderWarning().catch(() => null),
    account ? api.claudeAccess().catch(() => null) : Promise.resolve(null),
  ]);
  const enabled = ctx.info.settings.cloud_sync === true;
  const build = (current: SyncReport) => syncSection(ctx, current, enabled, warning, !unlockVisible(current, enabled));
  const syncElement = build(report);
  shownReport = report;
  syncBlock = { element: syncElement, build };

  return h("div", {},
    heading(),
    cloudPrompt(ctx),
    accountSection(ctx, account, accountError),
    account ? usageSection(account) : null,
    syncElement,
    ui.shownCode ? recoverySheet(ctx, ui.shownCode) : null,
    unlockSection(ctx, report, enabled),
    enabled && !ui.shownCode && !needsVault(report) ? recoverySection(ctx) : null,
    claude ? claudeSection(ctx, claude) : null,
    devices ? devicesSection(ctx, devices) : null,
    deleteSection(ctx));
}

function needsVault(report: SyncReport): boolean {
  return ["needs_setup", "needs_recovery_code", "vault_reset"].includes(report.status.kind);
}

function signedOut(ctx: Context): HTMLElement {
  if (ui.login.kind === "code") {
    const { start } = ui.login;
    return h("div", {}, heading(), plainSection(
      h("p", {}, l("account.login.browser_code")),
      h("p", { class: "login-code", "aria-live": "polite" }, start.user_code),
      h("div", { class: "row" },
        controlButton({ class: "btn", onclick: () => void api.openUrl(start.verification_uri).catch(ctx.report) }, l("account.login.reopen_browser")),
        controlButton({ class: "btn", onclick: cancelLogin }, l("account.login.cancel")))));
  }
  return h("div", {}, heading(), plainSection(
    ...(ui.login.kind === "expired"
      ? [h("p", { class: "error", role: "alert" }, l("account.login.expired")),
         controlButton({ class: "btn primary", onclick: () => void beginLogin(ctx) }, l("account.login.start_again"))]
      : [h("p", {}, l("account.login.intro")),
         ui.notice ? h("p", { class: "error", role: "alert" }, ui.notice) : null,
         controlButton({ class: "btn primary", onclick: () => void beginLogin(ctx) }, l("account.login.sign_in"))])));
}

function slots(): [keyof Pick<Settings, "dictation_stt" | "llm" | "meeting_stt">, string][] { return [
  ["dictation_stt", t("account.slot.dictation")],
  ["llm", t("account.slot.text")],
  ["meeting_stt", t("account.slot.meetings")],
]; }

function emptySlots(settings: Settings) {
  return slots().filter(([key]) => !settings.providers.some((provider) => provider.id === settings[key].provider_id));
}

function localizedList(items: string[]): string {
  return new Intl.ListFormat(uiLocale(), { style: "long", type: "conjunction" }).format(items);
}

function cloudPrompt(ctx: Context): HTMLElement | null {
  if (!ui.offerCloud) return null;
  const empty = emptySlots(ctx.info.settings);
  if (!empty.length) return null;
  const close = () => { ui.offerCloud = false; update(); };
  const adopt = () => {
    ui.offerCloud = false;
    void ctx.mutate(async () => {
      let next = await api.info();
      if (!next.settings.providers.some((provider) => provider.kind === "pennel_cloud")) next = await api.addProvider("pennel_cloud");
      const settings = structuredClone(next.settings);
      const cloud = settings.providers.find((provider) => provider.kind === "pennel_cloud");
      const kind = next.kinds.find((item) => item.kind === "pennel_cloud");
      if (!cloud) return next;
      for (const [key] of emptySlots(settings)) {
        settings[key] = { provider_id: cloud.id, model: (key === "llm" ? kind?.default_llm_model : kind?.default_stt_model) ?? "" };
      }
      return api.saveSettings(settings);
    });
  };
  return plainSection(
    h("p", {}, l("account.cloud_offer", () => ({ tasks: localizedList(emptySlots(ctx.info.settings).map(([, label]) => label)) }))),
    h("div", { class: "row" },
      controlButton({ class: "btn primary", onclick: adopt }, l("common.yes")),
      controlButton({ class: "btn", onclick: close }, l("common.no"))));
}

function accountSection(ctx: Context, account: CloudAccount | null, error: unknown): HTMLElement {
  let signingOut = false;
  const logout = controlButton({ class: "btn" }, liveText(() => t(signingOut ? "account.logout.busy" : "account.logout.action")));
  bindLocalization(logout, () => { logout.textContent = t(signingOut ? "account.logout.busy" : "account.logout.action"); });
  logout.addEventListener("click", async () => {
    const unconfirmed = recoveryNeedsConfirmation({ signedIn: true, syncOn: ctx.info.settings.cloud_sync === true, confirmed: confirmedFlag() });
    if (unconfirmed && !await confirmDialog(l("account.logout.title"),
      l("account.logout.unconfirmed"),
      l("account.logout.action"))) return;
    signingOut = true; logout.disabled = true;
    logout.textContent = t("account.logout.busy");
    try { await api.cloudLogout(); resetLocalState(); await ctx.refresh(); }
    catch (e) {
      signingOut = false; logout.disabled = false;
      logout.textContent = t("account.logout.action");
      ctx.report(e);
    }
  });
  return section(l("account.account_title"), null,
    account ? h("p", {}, l("account.signed_in_as", () => ({ email: account.email }))) : h("p", { class: "error", role: "alert" }, errorText(error)),
    logout);
}

function usageSection(account: CloudAccount): HTMLElement {
  const audio = h("p", {});
  const tokens = h("p", {});
  const storage = h("p", {});
  const update = () => {
    const usage = formatUsage(account);
    audio.textContent = usage.audio; tokens.textContent = usage.tokens; storage.textContent = usage.storage;
  };
  const root = section(l("account.usage_title"), null, audio, tokens, storage);
  update(); bindLocalization(root, update);
  return root;
}

function statusLine(report: SyncReport, enabled: boolean): string | null {
  if (ui.syncing) return t("account.sync.busy");
  if (!enabled) return null;
  const status = report.status;
  switch (status.kind) {
    case "pending": return t("account.sync.first_pending");
    case "synced":
      return report.last_success_ms === null ? t("account.sync.synced")
        : t("account.sync.synced_at", { time: relativeTime(report.last_success_ms, Date.now()) });
    case "offline": return t("account.sync.offline");
    case "storage_full":
    case "error": return localizeBackendText(status.message);
    default: return null;
  }
}

async function action(ctx: Context, work: () => Promise<void>) {
  ui.notice = null;
  try { await work(); }
  catch (error) { ui.notice = errorText(error); }
  await ctx.refresh().catch(ctx.report);
}

function syncSection(ctx: Context, report: SyncReport, enabled: boolean, warning: string | null, showNotice: boolean): HTMLElement {
  const control = toggle(l("account.sync.title"), enabled, (checked) => void action(ctx, async () => {
    if (!checked) { await api.cloudSyncDisable(); return; }
    try { showCode(await api.cloudSyncEnable()); ui.askCode = false; }
    catch (error) {
      if (errorCode(error) !== "vault_locked") throw error;
      ui.askCode = true;
    }
  }));
  const line = statusLine(report, enabled);
  const lineElement = line ? h("p", { class: report.brake || report.status.kind === "error" || report.status.kind === "storage_full" ? "error" : "muted", role: "status" }, line) : null;
  if (lineElement) bindLocalization(lineElement, () => { lineElement.textContent = statusLine(report, enabled) ?? ""; });
  const brake = report.brake;
  const setup = enabled && report.status.kind === "needs_setup" && !ui.shownCode;
  return section(l("account.sync.title"), null,
    control,
    h("p", { class: "help" }, l("account.sync.privacy")),
    lineElement,
    showNotice && ui.notice ? h("p", { class: "error", role: "alert" }, ui.notice) : null,
    warning ? h("p", { class: "help" }, liveText(() => localizeBackendText(warning))) : null,
    brake ? controlButton({ class: "btn", onclick: async () => {
      const ok = await confirmDialog(l("account.sync.confirm_deletes_title"),
        l("account.sync.confirm_deletes_detail"),
        l("account.sync.confirm_deletes"));
      if (ok) await action(ctx, () => api.cloudSyncConfirmDeletes());
    } }, l("account.sync.confirm_deletes")) : null,
    setup ? controlButton({ class: "btn", onclick: () => void action(ctx, async () => {
      try { showCode(await api.cloudSyncEnable()); }
      catch (error) {
        if (errorCode(error) !== "vault_locked") throw error;
        ui.askCode = true;
      }
    }) }, l("account.recovery.create")) : null,
    enabled && !needsVault(report) ? controlButton({ class: "btn", disabled: ui.syncing, onclick: () => {
      ui.syncing = true;
      window.clearTimeout(syncingTimer);
      syncingTimer = window.setTimeout(() => { ui.syncing = false; update(); }, SYNC_NOW_TIMEOUT_MS);
      update();
      void api.cloudSyncNow().catch((error) => { ui.syncing = false; ctx.report(error); update(); });
    } }, l("account.sync.now")) : null);
}

const copyFeedback = new WeakMap<HTMLElement, { failure: string | null }>();

function showCopyStatus(status: HTMLElement, failure: string | null) {
  let feedback = copyFeedback.get(status);
  if (!feedback) {
    feedback = { failure };
    copyFeedback.set(status, feedback);
    const current = feedback;
    bindLocalization(status, () => { status.textContent = copyStatusText(current.failure); });
  }
  feedback.failure = failure;
  status.textContent = copyStatusText(failure);
}

async function copyRecoveryCode(code: string, status: HTMLElement) {
  try { await api.copySecretText(code); showCopyStatus(status, null); }
  catch (error) { showCopyStatus(status, errorText(error)); }
}

function printRecoveryCode(code: string) {
  const page = h("div", { id: "print-sheet" },
    h("h1", {}, l("account.recovery.print_title")),
    h("p", { class: "print-code" }, code),
    h("p", {}, l("account.recovery.store_safely")));
  document.body.append(page);
  document.body.classList.add("printing-sheet");
  const finish = () => { document.body.classList.remove("printing-sheet"); page.remove(); window.removeEventListener("afterprint", finish); };
  window.addEventListener("afterprint", finish);
  window.setTimeout(finish, 60000);
  window.print();
}

function recoverySheet(ctx: Context, code: string): HTMLElement {
  const status = h("span", { class: "muted", role: "status" });
  return section(l("account.recovery.title"), null,
    h("p", {}, l("account.recovery.description")),
    h("div", { class: "recovery-code", "aria-label": l("account.recovery.title") }, code.split("-").map((group) => h("span", {}, group))),
    h("div", { class: "row" },
      controlButton({ class: "btn", onclick: () => void copyRecoveryCode(code, status) }, l("account.recovery.copy")),
      controlButton({ class: "btn primary", onclick: () => { ui.shownCode = null; setConfirmed(true); update(); void ctx.refresh().catch(ctx.report); } }, l("account.recovery.saved")),
      h("button", { class: "text-button", onclick: () => printRecoveryCode(code) }, l("account.recovery.print")),
      status));
}

function unlockVisible(report: SyncReport, enabled: boolean): boolean {
  if (ui.shownCode) return false;
  return ui.askCode || (enabled && (report.status.kind === "needs_recovery_code" || report.status.kind === "vault_reset"));
}

function unlockSection(ctx: Context, report: SyncReport, enabled: boolean): HTMLElement | null {
  const reset = enabled && report.status.kind === "vault_reset";
  if (!unlockVisible(report, enabled)) return null;
  const input = textField({ type: "text", value: ui.codeDraft, autocomplete: "off", spellcheck: "false", autocapitalize: "characters", "aria-label": l("account.recovery.title") });
  input.addEventListener("input", () => { ui.codeDraft = input.value; });
  const submit = () => void action(ctx, async () => {
    await api.cloudSyncUnlock(input.value);
    setConfirmed(true);
    ui.codeDraft = "";
    ui.askCode = false;
  });
  const form = h("form", { class: "row", onsubmit: (event: Event) => { event.preventDefault(); submit(); } },
    input, controlButton({ class: "btn primary", type: "submit" }, l("account.recovery.unlock")));
  return section(l("account.recovery.title"), null,
    reset ? h("p", {}, l("account.recovery.vault_reset")) : null,
    form,
    ui.notice ? h("p", { class: "error", role: "alert" }, ui.notice) : null,
    h("button", { class: "text-button", onclick: () => { ui.lostCode = !ui.lostCode; update(); } }, l("account.recovery.lost")),
    ui.lostCode ? [
      h("p", { class: "help" }, l("account.recovery.lost_detail")),
      controlButton({ class: "btn danger", onclick: async () => {
        const ok = await confirmDialog(l("account.recovery.reset_title"),
          l("account.recovery.reset_detail"),
          l("account.recovery.reset"));
        if (!ok) return;
        await action(ctx, async () => { showCode(await api.cloudSyncReset()); ui.askCode = false; ui.lostCode = false; ui.codeDraft = ""; });
      } }, l("account.recovery.reset"))] : null);
}

function recoverySection(ctx: Context): HTMLElement {
  return section(l("account.recovery.title"), null,
    recoveryNeedsConfirmation({ signedIn: true, syncOn: true, confirmed: confirmedFlag() })
      ? h("p", { class: "error", role: "status" }, l("account.recovery.unconfirmed")) : null,
    controlButton({ class: "btn", onclick: async () => {
      const ok = await confirmDialog(l("account.recovery.new_title"), l("account.recovery.new_detail"), l("account.recovery.new"));
      if (ok) await action(ctx, async () => { showCode(await api.cloudSyncNewRecoveryCode()); });
    } }, l("account.recovery.new")));
}

function copyField(label: UiText, value: string): HTMLElement {
  const status = h("span", { class: "muted", role: "status" });
  return h("div", {},
    h("p", { class: "help" }, label),
    h("div", { class: "row" },
      textField({ type: "text", class: "grow", readonly: true, value, "aria-label": label, spellcheck: "false" }),
      controlButton({ class: "btn", onclick: async () => {
        try { await api.copyText(value); showCopyStatus(status, null); }
        catch (error) { showCopyStatus(status, errorText(error)); }
      } }, l("common.copy")),
      status));
}

function claudeSection(ctx: Context, view: ClaudeAccessView): HTMLElement {
  const control = toggle(l("account.claude.title"), view.enabled, (checked) => void action(ctx, async () => {
    await api.setClaudeAccess(checked);
  }));
  const text = claudeLine(view.status, view.enabled);
  const line = h("p", { class: view.status.kind === "error" && view.enabled ? "error" : "muted", role: "status" }, text ?? "");
  const current = { line, enabled: view.enabled, status: view.status };
  claudeShown = current;
  bindLocalization(line, () => { line.textContent = claudeLine(current.status, current.enabled) ?? ""; });
  const steps = claudeInstructions(view.mcp_url);
  return section(l("account.claude.title"), null,
    control,
    h("p", { class: "help" }, l("account.claude.help")),
    line,
    view.enabled ? [
      copyField(l("account.claude.connector_help"), steps.connectorUrl),
      copyField(l("account.claude.code_help"), steps.command),
      h("p", { class: "help" }, l("account.claude.sign_in_help")),
    ] : null);
}

function seenText(device: CloudDevice): string {
  const time = Date.parse(device.last_seen_iso);
  return t("account.devices.last_seen", { time: Number.isNaN(time) ? device.last_seen_iso : relativeTime(time, Date.now()) });
}

function devicesSection(ctx: Context, devices: CloudDevice[]): HTMLElement {
  return section(l("account.devices.title"), null, h("div", { class: "list" }, devices.map((device) => {
    const name = textField({ type: "text", class: "device-name", value: device.name, "aria-label": l("account.devices.name") });
    name.dataset.original = device.name;
    name.addEventListener("change", () => {
      const next = name.value.trim();
      if (!next || next === device.name) { name.value = device.name; return; }
      void action(ctx, () => api.cloudRenameDevice(device.id, next));
    });
    return h("div", { class: "row device-row" },
      name,
      h("span", { class: "muted" }, liveText(() => `${device.platform} · ${seenText(device)}`)),
      device.current ? h("span", { class: "muted" }, l("account.devices.current")) : null,
      device.current ? null : controlButton({ class: "btn", onclick: async () => {
        const ok = await confirmDialog(l("account.devices.revoke_title"),
          l("account.devices.revoke_detail"),
          l("account.devices.revoke"));
        if (ok) await action(ctx, () => api.cloudRevokeDevice(device.id));
      } }, l("account.devices.revoke")));
  })));
}

function deleteSection(ctx: Context): HTMLElement {
  return section(l("account.delete.title"), null, controlButton({ class: "btn danger", onclick: async () => {
    const ok = await confirmDialog(l("account.delete.confirm_title"),
      l("account.delete.detail"),
      l("account.delete.title"));
    if (!ok) return;
    try { await api.cloudDeleteAccount(); resetLocalState(); await ctx.refresh(); } catch (error) { ctx.report(error); }
  } }, l("account.delete.title")));
}
