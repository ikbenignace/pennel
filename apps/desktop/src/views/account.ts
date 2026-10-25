import { api, type CloudAccount, type CloudDevice, type Settings, type SyncReport } from "../api";
import { accountScreenFor, afterSignOut, copyStatusText, formatUsage, initialAccountUi, pollLogin, recoveryNeedsConfirmation, relativeTime, type LoginPoll } from "../cloud-login";
import { confirmDialog, errorCode, errorText, h, section, toggle } from "../dom";
import type { Context } from "../main";

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

function update() { redraw?.(); }

function listenForSync() {
  if (listening) return;
  listening = true;
  void api.onSyncChanged(() => {
    ui.syncing = false;
    window.clearTimeout(syncingTimer);
    update();
  }).catch(() => { listening = false; });
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
  const mine = ctx.rerender;
  redraw = mine;
  ctx.onDispose(() => { if (redraw === mine) redraw = null; });

  if (ui.login.kind !== "idle") return signedOut(ctx);

  let account: CloudAccount | null = null;
  let accountError: unknown = null;
  try { account = await api.cloudAccount(); }
  catch (error) {
    if (accountScreenFor(errorCode(error)) === "signed_out") return signedOut(ctx);
    accountError = error;
  }
  const [report, devices, warning] = await Promise.all([
    api.cloudSyncLast(),
    account ? api.cloudDevices().catch(() => null) : Promise.resolve(null),
    api.cloudFolderWarning().catch(() => null),
  ]);
  const enabled = ctx.info.settings.cloud_sync === true;

  return h("div", {},
    heading(),
    cloudPrompt(ctx),
    accountSection(ctx, account, accountError),
    account ? usageSection(account) : null,
    syncSection(ctx, report, enabled, warning, !unlockVisible(report, enabled)),
    ui.shownCode ? recoverySheet(ctx, ui.shownCode) : null,
    unlockSection(ctx, report, enabled),
    enabled && !ui.shownCode && !needsVault(report) ? recoverySection(ctx) : null,
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
      h("p", {}, "Je browser is geopend. Controleer daar deze code:"),
      h("p", { class: "login-code", "aria-live": "polite" }, start.user_code),
      h("div", { class: "row" },
        h("button", { class: "btn", onclick: () => void api.openUrl(start.verification_uri).catch(ctx.report) }, "Browser opnieuw openen"),
        h("button", { class: "btn", onclick: cancelLogin }, "Annuleren"))));
  }
  return h("div", {}, heading(), plainSection(
    ...(ui.login.kind === "expired"
      ? [h("p", { class: "error", role: "alert" }, "De code is verlopen."),
         h("button", { class: "btn primary", onclick: () => void beginLogin(ctx) }, "Opnieuw beginnen")]
      : [h("p", {}, "Log in om je meetings op al je toestellen te hebben en Pennel Cloud als provider te gebruiken, zonder eigen API-sleutel."),
         ui.notice ? h("p", { class: "error", role: "alert" }, ui.notice) : null,
         h("button", { class: "btn primary", onclick: () => void beginLogin(ctx) }, "Inloggen")])));
}

const SLOTS: [keyof Pick<Settings, "dictation_stt" | "llm" | "meeting_stt">, string][] = [
  ["dictation_stt", "dicteren"],
  ["llm", "opkuisen en samenvatten"],
  ["meeting_stt", "meetings"],
];

function emptySlots(settings: Settings) {
  return SLOTS.filter(([key]) => !settings.providers.some((provider) => provider.id === settings[key].provider_id));
}

function dutchList(items: string[]): string {
  return items.length < 2 ? items.join("") : `${items.slice(0, -1).join(", ")} en ${items.at(-1)}`;
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
    h("p", {}, `Pennel Cloud gebruiken voor ${dutchList(empty.map(([, label]) => label))}?`),
    h("div", { class: "row" },
      h("button", { class: "btn primary", onclick: adopt }, "Ja"),
      h("button", { class: "btn", onclick: close }, "Nee")));
}

function accountSection(ctx: Context, account: CloudAccount | null, error: unknown): HTMLElement {
  const logout = h("button", { class: "btn", onclick: async () => {
    try { await api.cloudLogout(); resetLocalState(); await ctx.refresh(); } catch (e) { ctx.report(e); }
  } }, "Uitloggen");
  return section("Account", null,
    account ? h("p", {}, `Ingelogd als ${account.email}`) : h("p", { class: "error", role: "alert" }, errorText(error)),
    logout);
}

function usageSection(account: CloudAccount): HTMLElement {
  const usage = formatUsage(account);
  return section("Gebruik deze maand", null,
    h("p", {}, usage.audio), h("p", {}, usage.tokens), h("p", {}, usage.storage));
}

function statusLine(report: SyncReport, enabled: boolean): string | null {
  if (ui.syncing) return "Bezig…";
  if (!enabled) return null;
  const status = report.status;
  switch (status.kind) {
    case "pending": return "Wacht op eerste sync…";
    case "synced":
      return report.last_success_ms === null ? "Gesynchroniseerd"
        : `Gesynchroniseerd · ${relativeTime(report.last_success_ms, Date.now())}`;
    case "offline": return "Offline — probeert later opnieuw";
    case "storage_full":
    case "error": return status.message;
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
  const control = toggle("Meetings synchroniseren", enabled, (checked) => void action(ctx, async () => {
    if (!checked) { await api.cloudSyncDisable(); return; }
    try { showCode(await api.cloudSyncEnable()); ui.askCode = false; }
    catch (error) {
      if (errorCode(error) !== "vault_locked") throw error;
      ui.askCode = true;
    }
  }));
  const line = statusLine(report, enabled);
  const brake = report.brake;
  const setup = enabled && report.status.kind === "needs_setup" && !ui.shownCode;
  return section("Meetings synchroniseren", null,
    control,
    h("p", { class: "help" }, "End-to-end versleuteld: Pennel Cloud kan je meetings niet lezen. Audio blijft op dit toestel."),
    line ? h("p", { class: brake || report.status.kind === "error" || report.status.kind === "storage_full" ? "error" : "muted", role: "status" }, line) : null,
    showNotice && ui.notice ? h("p", { class: "error", role: "alert" }, ui.notice) : null,
    warning ? h("p", { class: "help" }, warning) : null,
    brake ? h("button", { class: "btn", onclick: async () => {
      const ok = await confirmDialog("Verwijderingen bevestigen?",
        "Pennel verwijdert deze meetings dan ook in Pennel Cloud en op je andere toestellen. Ze blijven 30 dagen in de prullenbak op elk toestel.",
        "Verwijderingen bevestigen");
      if (ok) await action(ctx, () => api.cloudSyncConfirmDeletes());
    } }, "Verwijderingen bevestigen") : null,
    setup ? h("button", { class: "btn", onclick: () => void action(ctx, async () => {
      try { showCode(await api.cloudSyncEnable()); }
      catch (error) {
        if (errorCode(error) !== "vault_locked") throw error;
        ui.askCode = true;
      }
    }) }, "Herstelcode maken") : null,
    enabled && !needsVault(report) ? h("button", { class: "btn", disabled: ui.syncing, onclick: () => {
      ui.syncing = true;
      window.clearTimeout(syncingTimer);
      syncingTimer = window.setTimeout(() => { ui.syncing = false; update(); }, SYNC_NOW_TIMEOUT_MS);
      update();
      void api.cloudSyncNow().catch((error) => { ui.syncing = false; ctx.report(error); update(); });
    } }, "Nu synchroniseren") : null);
}

async function copyRecoveryCode(code: string, status: HTMLElement) {
  try { await api.copySecretText(code); status.textContent = copyStatusText(null); }
  catch (error) { status.textContent = copyStatusText(errorText(error)); }
}

function printRecoveryCode(code: string) {
  const page = h("div", { id: "print-sheet" },
    h("h1", {}, "Pennel Cloud herstelcode"),
    h("p", { class: "print-code" }, code),
    h("p", {}, "Bewaar deze code op een veilige plek."));
  document.body.append(page);
  document.body.classList.add("printing-sheet");
  const finish = () => { document.body.classList.remove("printing-sheet"); page.remove(); window.removeEventListener("afterprint", finish); };
  window.addEventListener("afterprint", finish);
  window.setTimeout(finish, 60000);
  window.print();
}

function recoverySheet(ctx: Context, code: string): HTMLElement {
  const status = h("span", { class: "muted", role: "status" });
  return section("Herstelcode", null,
    h("p", {}, "Bewaar deze herstelcode. Je hebt hem nodig om sync op een ander toestel aan te zetten. Zonder deze code en zonder ingelogd toestel kan niemand je clouddata nog lezen, ook wij niet."),
    h("div", { class: "recovery-code", "aria-label": "Herstelcode" }, code.split("-").map((group) => h("span", {}, group))),
    h("div", { class: "row" },
      h("button", { class: "btn", onclick: () => void copyRecoveryCode(code, status) }, "Kopiëren"),
      h("button", { class: "btn primary", onclick: () => { ui.shownCode = null; setConfirmed(true); update(); void ctx.refresh().catch(ctx.report); } }, "Ik heb hem bewaard"),
      h("button", { class: "text-button", onclick: () => printRecoveryCode(code) }, "Afdrukken"),
      status));
}

function unlockVisible(report: SyncReport, enabled: boolean): boolean {
  if (ui.shownCode) return false;
  return ui.askCode || (enabled && (report.status.kind === "needs_recovery_code" || report.status.kind === "vault_reset"));
}

function unlockSection(ctx: Context, report: SyncReport, enabled: boolean): HTMLElement | null {
  const reset = enabled && report.status.kind === "vault_reset";
  if (!unlockVisible(report, enabled)) return null;
  const input = h("input", { type: "text", value: ui.codeDraft, autocomplete: "off", spellcheck: "false", autocapitalize: "characters", "aria-label": "Herstelcode" });
  input.addEventListener("input", () => { ui.codeDraft = input.value; });
  const submit = () => void action(ctx, async () => {
    await api.cloudSyncUnlock(input.value);
    setConfirmed(true);
    ui.codeDraft = "";
    ui.askCode = false;
  });
  const form = h("form", { class: "row", onsubmit: (event: Event) => { event.preventDefault(); submit(); } },
    input, h("button", { class: "btn primary", type: "submit" }, "Ontgrendelen"));
  return section("Herstelcode", null,
    reset ? h("p", {}, "De kluis is opnieuw begonnen op een ander toestel. Voer de nieuwe herstelcode in.") : null,
    form,
    ui.notice ? h("p", { class: "error", role: "alert" }, ui.notice) : null,
    h("button", { class: "text-button", onclick: () => { ui.lostCode = !ui.lostCode; update(); } }, "Herstelcode kwijt?"),
    ui.lostCode ? [
      h("p", { class: "help" }, "Zonder herstelcode kan niemand de meetings in Pennel Cloud nog lezen. Je kunt de kluis opnieuw beginnen."),
      h("button", { class: "btn danger", onclick: async () => {
        const ok = await confirmDialog("Kluis opnieuw beginnen?",
          "Dit wist alle meetings in Pennel Cloud. Meetings op je toestellen blijven bestaan en worden opnieuw geüpload. Andere toestellen hebben daarna de nieuwe herstelcode nodig.",
          "Kluis opnieuw beginnen");
        if (!ok) return;
        await action(ctx, async () => { showCode(await api.cloudSyncReset()); ui.askCode = false; ui.lostCode = false; ui.codeDraft = ""; });
      } }, "Kluis opnieuw beginnen")] : null);
}

function recoverySection(ctx: Context): HTMLElement {
  return section("Herstelcode", null,
    recoveryNeedsConfirmation({ signedIn: true, syncOn: true, confirmed: confirmedFlag() })
      ? h("p", { class: "error", role: "status" }, "Je hebt je herstelcode nog niet bevestigd.") : null,
    h("button", { class: "btn", onclick: async () => {
      const ok = await confirmDialog("Nieuwe herstelcode maken?", "De oude herstelcode werkt daarna niet meer.", "Nieuwe herstelcode maken");
      if (ok) await action(ctx, async () => { showCode(await api.cloudSyncNewRecoveryCode()); });
    } }, "Nieuwe herstelcode maken"));
}

function seenText(device: CloudDevice): string {
  const time = Date.parse(device.last_seen_iso);
  return `laatst gezien ${Number.isNaN(time) ? device.last_seen_iso : relativeTime(time, Date.now())}`;
}

function devicesSection(ctx: Context, devices: CloudDevice[]): HTMLElement {
  return section("Toestellen", null, h("div", { class: "list" }, devices.map((device) => {
    const name = h("input", { type: "text", value: device.name, "aria-label": "Naam van toestel" });
    name.addEventListener("change", () => {
      const next = name.value.trim();
      if (!next || next === device.name) { name.value = device.name; return; }
      void action(ctx, () => api.cloudRenameDevice(device.id, next));
    });
    return h("div", { class: "row device-row" },
      name,
      h("span", { class: "muted" }, `${device.platform} · ${seenText(device)}`),
      device.current ? h("span", { class: "muted" }, "(dit toestel)") : null,
      device.current ? null : h("button", { class: "btn", onclick: async () => {
        const ok = await confirmDialog("Toestel afmelden?",
          "Dit toestel kan dan niet meer synchroniseren. Wat het al had, blijft erop staan. Vrees je dat het gestolen is, begin dan de kluis opnieuw.",
          "Afmelden");
        if (ok) await action(ctx, () => api.cloudRevokeDevice(device.id));
      } }, "Afmelden"));
  })));
}

function deleteSection(ctx: Context): HTMLElement {
  return section("Account verwijderen", null, h("button", { class: "btn danger", onclick: async () => {
    const ok = await confirmDialog("Account verwijderen?",
      "Je account en alle versleutelde data in Pennel Cloud worden gewist. Meetings op dit toestel blijven staan.",
      "Account verwijderen");
    if (!ok) return;
    try { await api.cloudDeleteAccount(); resetLocalState(); await ctx.refresh(); } catch (error) { ctx.report(error); }
  } }, "Account verwijderen"));
}
