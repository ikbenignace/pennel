import "./styles.css";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type AppInfo, type MeetingFinished, type Settings, type UpdateInfo } from "./api";
import { LatestRequest, SerialQueue, persistAll } from "./async";
import { errorText, h } from "./dom";
import { dictationView, dictionaryView, stylesView } from "./views/dictation";
import { historyView } from "./views/history";
import { displayedMeetingSession, finishMeetingDrafts, flushMeetingDrafts, meetingView, meetingsView } from "./views/meetings";
import { FrontendBarrier, canShowFinished } from "./session-events";
import { localNavigation } from "./navigation";
import { modelsView } from "./views/models";
import { providersView } from "./views/providers";
import { meetingSettingsView, recordingSettingsView } from "./views/settings";
import { onboardingView, promptView } from "./views/welcome";

export interface Context {
  info: AppInfo;
  save: (change: (settings: Settings) => void) => Promise<void>;
  mutate: (work: () => Promise<AppInfo>) => Promise<void>;
  update: (info: AppInfo) => void;
  refresh: () => Promise<void>;
  rerender: () => void;
  navigate: (tab: string, param?: string, jumpMs?: number) => void;
  report: (error: unknown) => void;
  onDispose: (cleanup: () => void) => void;
  param?: string;
  jumpMs?: number;
}

type View = (ctx: Context) => HTMLElement | Promise<HTMLElement>;
type Mode = "library" | "settings" | "history" | "live" | "onboarding" | "prompt";
const modes: Mode[] = ["library", "settings", "history", "live", "onboarding", "prompt"];
const tabs: [string, string, View][] = [
  ["providers", "Providers", providersView],
  ["models", "Modellen", modelsView],
  ["dictation", "Dicteren", dictationView],
  ["meeting-settings", "Meetings", meetingSettingsView],
  ["recording", "Opnemen", recordingSettingsView],
  ["dictionary", "Woordenlijst", dictionaryView],
  ["styles", "Stijlen", stylesView],
];
const query = new URLSearchParams(location.search);
const requested = query.get("window") as Mode;
const mode = modes.includes(requested) ? requested : "library";
let current = query.get("tab") ?? "providers";
if (current === "meetings" && mode === "settings") current = "meeting-settings";
if (!tabs.some(([id]) => id === current)) current = "providers";
let param = mode === "library" ? query.get("tab") ?? undefined : undefined;
let jumpMs: number | undefined;
let info: AppInfo;
let booted = false;
let capabilitiesDirty = false;
let generation = 0;
let activeCleanup: (() => void)[] = [];
const settingsQueue = new SerialQueue();
const barrier = new FrontendBarrier();
const finishedRequests = new LatestRequest();
let barrierKind: "quit" | "recording" | "update" | null = null;
let guardedFinishSeen = false;
const pendingSettings: ((settings: Settings) => void)[] = [];
const content = h("main", { id: "content", tabindex: -1 });
const banner = h("div", { class: "notice-banner", role: "status", hidden: true });
const tabButtons = new Map<string, HTMLButtonElement>();
const app = document.getElementById("app")!;

function report(error: unknown) {
  banner.textContent = errorText(error);
  banner.hidden = false;
}

function navigate(tab: string, nextParam?: string, nextJump?: number) {
  if (mode === "live" && (tab === "meeting" || tab === "live")) {
    param = undefined;
    void render();
    return;
  }
  if (mode === "library" && (tab === "meetings" || tab === "library" || tab === "chat")) {
    param = tab === "chat" ? "__chat__" : nextParam;
    jumpMs = nextJump;
    void render();
    return;
  }
  if (mode === "settings" && (tabs.some(([id]) => id === tab) || tab === "local" || tab === "meetings")) {
    current = tab === "local" ? "providers" : tab === "meetings" ? "meeting-settings" : tab;
    param = tab === "local" ? "__local__" : undefined;
    void render();
    return;
  }
  const kind = tab === "meeting" || tab === "live" ? "live"
    : tab === "history" ? "history"
    : tab === "meetings" || tab === "library" || tab === "chat" ? "library" : "settings";
  void api.showWindow(kind, kind === "settings" ? tab : nextParam ?? null).catch(report);
}

async function refresh(allowed: () => boolean = () => true) {
  await settingsQueue.run(async () => { info = await api.info(); });
  if (allowed()) await render(allowed);
}

async function flushSettings() {
  if (!pendingSettings.length) return;
  const changes = pendingSettings.slice();
  const latest = await api.info();
  const next = structuredClone(latest.settings);
  changes.forEach((change) => change(next));
  info = await api.saveSettings(next);
  pendingSettings.splice(0, changes.length);
}

async function render(allowed: () => boolean = () => true) {
  if (!allowed()) return;
  const mine = ++generation;
  const cleanup: (() => void)[] = [];
  for (const [id, button] of tabButtons) {
    button.setAttribute("aria-selected", String(id === current));
    button.tabIndex = id === current ? 0 : -1;
  }
  const visibleInfo = pendingSettings.length ? structuredClone(info) : info;
  if (pendingSettings.length) pendingSettings.forEach((change) => change(visibleInfo.settings));
  const ctx: Context = {
    info: visibleInfo,
    save: (change) => {
      pendingSettings.push(change);
      return settingsQueue.run(async () => {
        await flushSettings();
        banner.hidden = true;
        void render();
      }).catch(report);
    },
    mutate: (work) => settingsQueue.run(async () => {
        await flushSettings();
        info = await work();
        banner.hidden = true;
        void render();
    }).catch(report),
    update: (next) => { info = next; void render(); },
    refresh,
    rerender: () => { void render(); },
    navigate,
    report,
    onDispose: (action) => cleanup.push(action),
    param,
    jumpMs,
  };
  const view = mode === "settings" ? tabs.find(([id]) => id === current)![2]
    : mode === "history" ? historyView
    : mode === "live" ? meetingView
    : mode === "onboarding" ? onboardingView
    : mode === "prompt" ? promptView : meetingsView;
  try {
    const node = await view(ctx);
    if (mine !== generation || !allowed()) { cleanup.forEach((action) => action()); return; }
    activeCleanup.forEach((action) => action());
    activeCleanup = cleanup;
    if (ctx.jumpMs === jumpMs) jumpMs = undefined;
    const scroll = content.scrollTop;
    content.replaceChildren(node);
    content.scrollTop = scroll;
  } catch (error) {
    cleanup.forEach((action) => action());
    if (mine === generation && allowed()) report(error);
  }
}

function lockInterface() {
  app.inert = barrier.locked;
  if (barrier.locked) app.setAttribute("aria-busy", "true");
  else app.removeAttribute("aria-busy");
}

function prepareFlush(requestId: number, kind: "quit" | "recording" | "update") {
  barrier.begin(requestId);
  barrierKind = kind;
  lockInterface();
  void (async () => {
    try {
      await persistAll([
        async () => { await settingsQueue.drain(); await settingsQueue.run(flushSettings); await settingsQueue.drain(); },
        flushMeetingDrafts,
      ]);
      await api.frontendFlushed(requestId, true);
    } catch (error) {
      if (barrier.isCurrent(requestId)) report(error);
      try { await api.frontendFlushed(requestId, false, errorText(error)); }
      catch (ackError) { if (barrier.isCurrent(requestId)) report(ackError); }
    }
  })();
}

function meetingFinished(event: MeetingFinished) {
  finishMeetingDrafts(event);
  void finishedRequests.run(api.meetingStatus, (status) => {
    const accepted = canShowFinished(status, event.session_id, displayedMeetingSession());
    if (mode === "live" && accepted && event.id) param = event.id;
    if ((barrierKind === null || barrierKind === "recording") && ((mode === "live" && accepted) || (mode === "library" && param === event.id))) {
      const warning = event.error ?? event.flush_warning;
      if (warning) report(warning);
    }
    if (mode === "library" || mode === "live") void render();
  }, (error) => { if (!barrier.locked) report(error); });
}

function settingsTabs() {
  const strip = h("nav", { class: "settings-tabs segmented", role: "tablist", "aria-label": "Instellingen" });
  tabs.forEach(([id, label]) => {
    const button = h("button", { role: "tab", onclick: () => navigate(id), "aria-controls": "content" }, label);
    button.addEventListener("keydown", (event) => {
      const index = tabs.findIndex(([key]) => key === current);
      const next = event.key === "ArrowRight" ? (index + 1) % tabs.length
        : event.key === "ArrowLeft" ? (index + tabs.length - 1) % tabs.length
        : event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : -1;
      if (next < 0) return;
      event.preventDefault();
      navigate(tabs[next][0]);
      tabButtons.get(tabs[next][0])?.focus();
    });
    tabButtons.set(id, button);
    strip.append(button);
  });
  return h("header", { class: "settings-header" }, strip);
}

function settingsFooter() {
  const status = h("span", { class: "muted", role: "status" });
  const install = h("button", { class: "btn", hidden: true }, "Update installeren");
  install.addEventListener("click", async () => {
    install.disabled = true;
    status.textContent = "Update downloaden…";
    try { await api.installUpdate(); status.textContent = "Update geïnstalleerd. Start Pennel opnieuw."; }
    catch (error) { status.textContent = errorText(error); install.disabled = false; }
  });
  const check = h("button", { class: "btn" }, "Zoek naar updates…");
  check.addEventListener("click", async () => {
    check.disabled = true;
    status.textContent = "Zoeken…";
    try {
      const update = await api.checkForUpdates();
      status.textContent = update.available ? `Versie ${update.version} is beschikbaar.` : "Pennel is bijgewerkt.";
      install.hidden = !update.available;
    } catch (error) { status.textContent = errorText(error); }
    check.disabled = false;
  });
  void listen<UpdateInfo>("update-available", (event) => {
    install.hidden = !event.payload.available;
    status.textContent = event.payload.available ? `Versie ${event.payload.version} is beschikbaar.` : "";
  }).catch(report);
  return h("footer", { class: "settings-footer row" },
    h("span", { class: "muted" }, `Pennel ${info.version}${info.preview ? " · Preview" : ""}`),
    h("span", { class: "grow" }), status, check, install);
}

async function boot() {
  document.body.dataset.window = mode;
  app.replaceChildren(banner, content);
  await listen("capabilities-changed", () => {
    if (!booted) capabilitiesDirty = true;
    else void refresh().catch(report);
  });
  info = await api.info();
  app.replaceChildren(...(mode === "settings" ? [settingsTabs()] : []), banner, content);
  if (mode === "settings") app.append(settingsFooter());
  await listen<string>("navigate", (event) => {
    const route = localNavigation(mode, event.payload);
    if (!route) return;
    if (route.kind === "settings") { current = route.tab; param = route.param; }
    else if (route.kind === "library") param = route.param;
    else param = undefined;
    jumpMs = undefined;
    void render();
  });
  await listen("history-changed", () => { if (mode === "history") void render(); });
  await listen("settings-changed", () => { void refresh().catch(report); });
  await listen("meeting-updated", () => { if (mode === "library" || mode === "live") void render(); });
  await listen<MeetingFinished>("meeting-finished", (event) => {
    guardedFinishSeen = true;
    meetingFinished(event.payload);
  });
  await listen<string | null>("meeting-stopped", (event) => {
    void event;
    if (mode === "library" || (mode === "live" && !guardedFinishSeen)) void render();
  });
  if (mode !== "prompt") {
    await listen<number>("prepare-exit", (event) => prepareFlush(event.payload, "quit"));
    await listen<number>("prepare-recording-stop", (event) => prepareFlush(event.payload, "recording"));
    await listen<number>("prepare-update", (event) => prepareFlush(event.payload, "update"));
    await listen<number>("recording-flush-complete", (event) => {
      const requestId = event.payload;
      void barrier.resume(requestId, () => refresh(() => barrier.isCurrent(requestId))).then((resumed) => {
        if (resumed) barrierKind = null;
        lockInterface();
      }).catch((error) => {
        if (!barrier.isCurrent(requestId)) return;
        report(error); barrier.complete(requestId); barrierKind = null; lockInterface();
      });
    });
    await listen<number>("exit-cancelled", (event) => {
      if (barrier.complete(event.payload)) { barrierKind = null; lockInterface(); }
    });
    await listen<number>("update-cancelled", (event) => {
      if (barrier.complete(event.payload)) { barrierKind = null; lockInterface(); }
    });
    await api.frontendReady();
    void getCurrentWindow().onFocusChanged((event) => {
      if (event.payload) void refresh().catch(report);
    }).catch(() => undefined);
  }
  await render();
  booted = true;
  if (capabilitiesDirty) { capabilitiesDirty = false; await refresh(); }
}

window.addEventListener("unhandledrejection", (event) => { event.preventDefault(); report(event.reason); });
window.addEventListener("beforeunload", () => { activeCleanup.forEach((action) => action()); });
void boot().catch(report);
