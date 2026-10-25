import "./styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, fixtureClient, listen, type AppInfo, type MeetingFinished, type Settings, type UpdateInfo } from "./api";
import { LatestRequest, SerialQueue, persistAll } from "./async";
import { errorText, h, select } from "./dom";
import { icon, type IconName } from "./icons";
import { dictationView, dictionaryView, stylesView } from "./views/dictation";
import { historyView } from "./views/history";
import { displayedMeetingSession, finishMeetingDrafts, flushMeetingDrafts, meetingView, meetingsView } from "./views/meetings";
import { FrontendBarrier, canShowFinished } from "./session-events";
import { legacyWorkspaceRoute, parseWorkspaceRoute, routeKey, samePadRoute, settingTabs, WorkspaceNavigation, workspaceRoute, type WorkspaceRoute, type WorkspaceSection } from "./workspace-state";
import { modelsView } from "./views/models";
import { accountView } from "./views/account";
import { providersView } from "./views/providers";
import { meetingSettingsView, recordingSettingsView } from "./views/settings";
import { promptView } from "./views/welcome";
import { onboardingView } from "./views/onboarding";
import { padView } from "./views/pad";
import { appInfoContentEqual, appearanceIsDark, hotkeyLabel } from "./presentation";

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
  beforeLeave: (flush: () => Promise<void>) => void;
  onFlush: (flush: () => Promise<void>) => void;
  onInfoChange: (listener: (info: AppInfo) => void) => void;
  onFocusChange: (listener: (focused: boolean) => void) => void;
  param?: string;
  jumpMs?: number;
}

type View = (ctx: Context) => HTMLElement | Promise<HTMLElement>;
interface Mounted {
  key: string;
  cleanup: (() => void)[];
  flush: (() => Promise<void>)[];
  leave: (() => Promise<void>)[];
  info: ((info: AppInfo) => void)[];
  focus: ((focused: boolean) => void)[];
}

const tabs: [string, string, View][] = [
  ["providers", "Providers", providersView], ["models", "Modellen", modelsView],
  ["dictation", "Dicteren", dictationView], ["meeting-settings", "Meetings", meetingSettingsView],
  ["recording", "Opnemen", recordingSettingsView], ["dictionary", "Woordenlijst", dictionaryView],
  ["styles", "Stijlen", stylesView], ["appearance", "Weergave", appearanceView],
  ["account", "Account", accountView],
];
const query = new URLSearchParams(location.search);
const mode = query.get("window") === "onboarding" ? "onboarding" : ["prompt", "record-prompt"].includes(query.get("window") ?? "") ? "prompt" : "workspace";
const navigation = new WorkspaceNavigation();
const initial = legacyWorkspaceRoute(query.get("window") ?? "workspace", query.get("tab"));
if (initial) navigation.set(initial);
if (fixtureClient() && query.has("section")) {
  const route = parseWorkspaceRoute({ section: query.get("section"), tab: query.get("tab"), meeting_id: query.get("meeting_id"), jump_ms: query.has("jump_ms") ? Number(query.get("jump_ms")) : null });
  if (route) navigation.set(route);
}
let info: AppInfo;
let mounted: Mounted | null = null;
let generation = 0;
let navigationGeneration = 0;
let booted = false;
let capabilitiesDirty = false;
let needsRefresh = false;
let guardedFinishSeen = false;
let barrierKind: "quit" | "recording" | "update" | null = null;
const settingsQueue = new SerialQueue();
const pendingSettings: ((settings: Settings) => void)[] = [];
const barrier = new FrontendBarrier();
const finishedRequests = new LatestRequest();
const app = document.getElementById("app")!;
const content = h("main", { id: "content" });
const banner = h("div", { class: "notice-banner", role: "status", hidden: true });
const navButtons = new Map<WorkspaceSection, HTMLButtonElement>();
const toolbarTitle = h("span", { class: "workspace-title" });
const liveButton = h("button", { class: "workspace-live text-button", hidden: true, onclick: () => { void go(workspaceRoute("recording")); } });
const recentList = h("div", { class: "workspace-recents" });

function report(error: unknown) {
  banner.textContent = errorText(error);
  banner.hidden = false;
}

function visibleInfo(): AppInfo {
  if (!pendingSettings.length) return info;
  const value = structuredClone(info);
  pendingSettings.forEach((change) => change(value.settings));
  return value;
}

function applyAppearance() {
  document.documentElement.dataset.appearance = visibleInfo().settings.appearance ?? "system";
  const button = app.querySelector("[data-theme-toggle]");
  if (button) button.replaceChildren(icon(appearanceIsDark(visibleInfo().settings.appearance, matchMedia("(prefers-color-scheme: dark)").matches) ? "sun" : "moon"));
}

function notifyInfo() {
  applyAppearance();
  const shortcut = app.querySelector("[data-shortcut-label]");
  if (shortcut) shortcut.textContent = hotkeyLabel(visibleInfo().settings.hotkey, info.platform);
  mounted?.info.forEach((listener) => listener(visibleInfo()));
}

async function flushSettings() {
  if (!pendingSettings.length) return;
  const changes = pendingSettings.slice();
  const latest = await api.info();
  const next = structuredClone(latest.settings);
  changes.forEach((change) => change(next));
  info = await api.saveSettings(next);
  pendingSettings.splice(0, changes.length);
  notifyInfo();
}

async function flushAll() {
  await persistAll([
    async () => { await settingsQueue.drain(); await settingsQueue.run(flushSettings); await settingsQueue.drain(); },
    flushMeetingDrafts,
    ...(mounted?.flush ?? []),
  ]);
}

function editing(): boolean {
  return !!document.querySelector("dialog[open]") || !!document.activeElement?.matches("input,textarea,select,[contenteditable=true]");
}

async function refresh(allowed: () => boolean = () => true, preserveSameContent = false) {
  const previous = visibleInfo();
  await settingsQueue.run(async () => { info = await api.info(); });
  if (!allowed()) return;
  notifyInfo();
  if (preserveSameContent && appInfoContentEqual(previous, visibleInfo())) return;
  if (mounted?.info.length || navigation.current.section === "recording") return;
  if (editing()) { needsRefresh = true; return; }
  await render(allowed);
}

function navigate(tab: string, param?: string, jumpMs?: number) {
  if (tab === "onboarding") { void api.showWindow("onboarding").catch(report); return; }
  if (tab === "meetings" || tab === "library" || tab === "chat") {
    void go(workspaceRoute("meetings", tab === "chat" ? "chat" : null, param ?? null, jumpMs ?? null)); return;
  }
  if (tab === "meeting" || tab === "live") { void go(workspaceRoute("recording", null, param ?? null)); return; }
  if (tab === "history") { void go(workspaceRoute("history")); return; }
  if (tab === "dictation-home") { void go(workspaceRoute("dictation")); return; }
  if (tab === "dictation-pad") { void go(workspaceRoute("dictation", "pad")); return; }
  if (settingTabs.includes(tab as typeof settingTabs[number]) || tab === "local") void go(workspaceRoute("settings", tab));
}

async function go(route: WorkspaceRoute) {
  if (mode !== "workspace") { await api.showWorkspace(route).catch(report); return; }
  if (samePadRoute(navigation.current, route)) { ++navigationGeneration; content.inert = false; return; }
  const mine = ++navigationGeneration;
  try {
    content.inert = true;
    await flushAll();
    if (mine !== navigationGeneration) return;
    await persistAll(mounted?.leave ?? []);
    if (mine !== navigationGeneration) return;
    navigation.set(route);
    await render();
  } catch (error) { report(error); }
  finally { if (mine === navigationGeneration) content.inert = false; }
}

function contextFor(target: Mounted): Context {
  const route = navigation.current;
  return {
    get info() { return visibleInfo(); },
    save: (change) => {
      pendingSettings.push(change);
      applyAppearance();
      return settingsQueue.run(async () => { await flushSettings(); banner.hidden = true; }).then(async () => {
        if (target.info.length || mounted !== target) return;
        if (editing()) { needsRefresh = true; return; }
        await render();
      }).catch((error: unknown) => { report(error); throw error; });
    },
    mutate: (work) => settingsQueue.run(async () => { await flushSettings(); info = await work(); notifyInfo(); banner.hidden = true; }).then(async () => {
      if (!target.info.length && mounted === target) await render();
    }).catch((error: unknown) => { report(error); throw error; }),
    update: (next) => { info = next; notifyInfo(); },
    refresh,
    rerender: () => { if (mounted === target) void render().catch(report); },
    navigate,
    report,
    onDispose: (cleanup) => target.cleanup.push(cleanup),
    beforeLeave: (flush) => target.leave.push(flush),
    onFlush: (flush) => target.flush.push(flush),
    onInfoChange: (listener) => target.info.push(listener),
    onFocusChange: (listener) => target.focus.push(listener),
    param: route.section === "dictation" && route.tab === "pad" ? "__pad__" : route.section === "settings" && route.tab === "local" ? "__local__" : route.tab === "chat" ? "__chat__" : route.meeting_id ?? undefined,
    jumpMs: route.jump_ms ?? undefined,
  };
}

async function settingsView(ctx: Context): Promise<HTMLElement> {
  const current = navigation.current.tab === "local" ? "providers" : navigation.current.tab ?? "providers";
  const nav = h("nav", { class: "settings-categories", "aria-label": "Instellingencategorieën" }, tabs.map(([id, label]) => h("button", {
    "aria-current": id === current ? "page" : null, onclick: () => navigate(id),
  }, label)));
  const selected = tabs.find(([id]) => id === current) ?? tabs[0];
  const pane = h("section", { class: "settings-pane" }, await selected[2](ctx));
  return h("div", { class: "settings-workspace" }, h("h1", {}, "Instellingen"), h("div", { class: "settings-layout" }, nav, pane), settingsFooter(ctx));
}

function appearanceView(ctx: Context): HTMLElement {
  const picker = select([["system", "Volg systeem"], ["light", "Licht"], ["dark", "Donker"]], ctx.info.settings.appearance ?? "system", (value) => {
    void ctx.save((settings) => { settings.appearance = value; });
  });
  picker.setAttribute("aria-label", "Weergave");
  ctx.onInfoChange((next) => { picker.value = next.settings.appearance; });
  return h("div", {}, h("h2", {}, "Weergave"), h("label", { class: "preference-row" }, h("span", {}, "Weergave"), picker),
    h("p", { class: "help" }, "Volg systeem past de lichte of donkere weergave aan met je apparaat."),
    h("button", { class: "text-button onboarding-replay", onclick: () => api.showWindow("onboarding", "welcome") }, "Introductie opnieuw openen"));
}

async function render(allowed: () => boolean = () => true) {
  if (!allowed()) return;
  const mine = ++generation;
  const route = navigation.current;
  const target: Mounted = { key: routeKey(route), cleanup: [], flush: [], leave: [], info: [], focus: [] };
  const ctx = contextFor(target);
  const view = mode === "onboarding" ? onboardingView : mode === "prompt" ? promptView
    : route.section === "settings" ? settingsView : route.section === "history" ? historyView
    : route.section === "recording" ? meetingView : route.section === "meetings" ? meetingsView : padView;
  try {
    const node = await view(ctx);
    if (mine !== generation || !allowed()) { target.cleanup.forEach((cleanup) => cleanup()); return; }
    const paneScroll = mounted?.key === target.key ? content.querySelector<HTMLElement>(".settings-pane")?.scrollTop : undefined;
    mounted?.cleanup.forEach((cleanup) => cleanup());
    mounted = target;
    const scroll = content.scrollTop;
    content.replaceChildren(node);
    content.scrollTop = scroll;
    if (paneScroll !== undefined) { const pane = content.querySelector<HTMLElement>(".settings-pane"); if (pane) pane.scrollTop = paneScroll; }
    if (ctx.jumpMs === route.jump_ms) navigation.current.jump_ms = null;
    needsRefresh = false;
    updateNavigation();
  } catch (error) {
    target.cleanup.forEach((cleanup) => cleanup());
    if (mine === generation && allowed()) report(error);
  }
}

function updateNavigation() {
  const section = navigation.current.section === "recording" ? "meetings" : navigation.current.section;
  navButtons.forEach((button, key) => { if (key === section) button.setAttribute("aria-current", "page"); else button.removeAttribute("aria-current"); });
  toolbarTitle.textContent = ({ dictation: "Dicteren", meetings: "Vergaderingen", history: "Geschiedenis", settings: "Instellingen", recording: "Opname" })[navigation.current.section];
}

function workspaceShell() {
  const wordmark = h("div", { class: "workspace-wordmark", "aria-label": "Pennel" }, "pennel", h("span", { "aria-hidden": "true" }));
  const nav = h("nav", { class: "workspace-navigation", "aria-label": "Hoofdnavigatie" });
  for (const [section, label, glyph] of [["dictation", "Dicteren", "mic"], ["meetings", "Vergaderingen", "notebook"], ["history", "Geschiedenis", "history"]] as [WorkspaceSection, string, IconName][]) {
    const button = h("button", { class: "workspace-nav-button", onclick: () => { void go(navigation.select(section)); } }, icon(glyph), h("span", {}, label));
    navButtons.set(section, button); nav.append(button);
  }
  const settings = h("button", { class: "workspace-nav-button", onclick: () => { void go(navigation.select("settings")); } }, icon("settings"), "Instellingen");
  navButtons.set("settings", settings);
  const shortcut = h("div", { class: "workspace-shortcut" }, h("div", {}, h("kbd", { "data-shortcut-label": true }, hotkeyLabel(info.settings.hotkey, info.platform)), h("strong", {}, "Hou in en spreek")), h("span", { class: "help" }, "Waar je cursor staat."));
  const sidebar = h("aside", { class: "workspace-sidebar" }, wordmark, nav, liveButton, recentList,
    h("div", { class: "workspace-sidebar-bottom" }, shortcut, settings));
  const theme = h("button", { class: "button-icon", "data-theme-toggle": true, "aria-label": "Weergave wijzigen", onclick: () => {
    pendingSettings.push((settings) => { settings.appearance = appearanceIsDark(settings.appearance, matchMedia("(prefers-color-scheme: dark)").matches) ? "light" : "dark"; });
    applyAppearance(); void settingsQueue.run(flushSettings).catch(report);
  } }, icon("moon"));
  const toolbar = h("header", { class: "workspace-toolbar" }, toolbarTitle, theme);
  app.replaceChildren(h("div", { class: "workspace-shell" }, sidebar, h("div", { class: "workspace-body" }, toolbar, banner, content)));
  applyAppearance();
}

async function refreshSidebar() {
  if (mode !== "workspace") return;
  const [meetings, status] = await Promise.all([api.meetings(""), api.meetingStatus()]);
  recentList.replaceChildren();
  if (meetings.length) recentList.append(h("p", { class: "help" }, "Recente meetings"), ...meetings.slice(0, 3).map((meeting) => h("button", {
    class: "text-button", onclick: () => { void go(workspaceRoute("meetings", null, meeting.id)); },
  }, meeting.title)));
  liveButton.hidden = !status.busy;
  liveButton.textContent = status.busy ? `● ${status.title ?? "Opname"}${status.phase === "finalizing" ? " · Afronden…" : ""}` : "";
}

function lockInterface() {
  app.inert = barrier.locked;
  if (barrier.locked) app.setAttribute("aria-busy", "true"); else app.removeAttribute("aria-busy");
}

function prepareFlush(requestId: number, kind: "quit" | "recording" | "update") {
  barrier.begin(requestId); barrierKind = kind; lockInterface();
  void (async () => {
    try { await flushAll(); await api.frontendFlushed(requestId, true); }
    catch (error) {
      if (barrier.isCurrent(requestId)) report(error);
      try { await api.frontendFlushed(requestId, false, errorText(error)); } catch (ackError) { if (barrier.isCurrent(requestId)) report(ackError); }
    }
  })();
}

function meetingFinished(event: MeetingFinished) {
  finishMeetingDrafts(event);
  void finishedRequests.run(api.meetingStatus, (status) => {
    const accepted = canShowFinished(status, event.session_id, displayedMeetingSession());
    const showing = navigation.current.section === "recording" || (navigation.current.section === "meetings" && navigation.current.meeting_id === event.id);
    if (navigation.current.section === "recording" && accepted && event.id) navigation.set(workspaceRoute("meetings", null, event.id));
    if (accepted && showing && (barrierKind === null || barrierKind === "recording")) {
      const warning = event.error ?? event.flush_warning; if (warning) report(warning);
      void render();
    }
    void refreshSidebar().catch(report);
  }, (error) => { if (!barrier.locked) report(error); });
}

function settingsFooter(ctx: Context) {
  const status = h("span", { class: "help", role: "status" }, info.preview ? "Updates uitgeschakeld in de preview." : "");
  const install = h("button", { class: "btn", hidden: true, disabled: info.preview }, "Update installeren");
  install.addEventListener("click", async () => {
    install.disabled = true; status.textContent = "Update downloaden…";
    try { await api.installUpdate(); status.textContent = "Update geïnstalleerd. Start Pennel opnieuw."; }
    catch (error) { status.textContent = errorText(error); install.disabled = false; }
  });
  const check = h("button", { class: "btn", disabled: info.preview, title: info.preview ? "Updates uitgeschakeld in de preview." : null, onclick: async () => {
    check.disabled = true; status.textContent = "Zoeken…";
    try { const update: UpdateInfo = await api.checkForUpdates(); status.textContent = update.available ? `Versie ${update.version} is beschikbaar.` : "Pennel is bijgewerkt."; install.hidden = !update.available; }
    catch (error) { status.textContent = errorText(error); }
    check.disabled = false;
  } }, "Zoek naar updates…");
  let disposed = false;
  ctx.onDispose(() => { disposed = true; });
  void listen<UpdateInfo>("update-available", (event) => {
    if (disposed || info.preview) return;
    install.hidden = !event.payload.available;
    if (event.payload.available) status.textContent = `Versie ${event.payload.version} is beschikbaar.`;
  }).then((unlisten) => { if (disposed) unlisten(); else ctx.onDispose(unlisten); }).catch(report);
  return h("footer", { class: "settings-footer row" }, h("span", { class: "help" }, `Pennel ${info.version}${info.preview ? " · Preview" : ""}`), h("span", { class: "grow" }), status, check, install);
}

function focusChanged(focused: boolean) {
  mounted?.focus.forEach((listener) => listener(focused));
  if (focused) void refresh(() => true, true).catch(report);
}

async function boot() {
  document.body.dataset.window = mode;
  info = await api.info(); applyAppearance();
  if (mode === "workspace") workspaceShell(); else app.replaceChildren(banner, content);
  await listen("capabilities-changed", () => { if (!booted) capabilitiesDirty = true; else void refresh().catch(report); });
  await listen<unknown>("workspace-navigate", (event) => { const route = parseWorkspaceRoute(event.payload); if (route) void go(route); });
  await listen<string>("navigate", (event) => {
    if (mode === "onboarding") { window.dispatchEvent(new CustomEvent("pennel-onboarding-route", { detail: event.payload })); return; }
    const route = legacyWorkspaceRoute("workspace", event.payload); if (route) void go(route);
  });
  await listen("settings-changed", () => { void refresh(() => true, true).catch(report); });
  await listen("history-changed", () => { if (navigation.current.section === "history") void refresh().catch(report); });
  await listen("meeting-updated", () => {
    void refreshSidebar().catch(report);
    if (navigation.current.section === "meetings") { if (editing()) needsRefresh = true; else void render(); }
  });
  await listen<MeetingFinished>("meeting-finished", (event) => { guardedFinishSeen = true; meetingFinished(event.payload); });
  await listen("meeting-stopped", () => { if (!guardedFinishSeen && ["recording", "meetings"].includes(navigation.current.section)) void render(); void refreshSidebar().catch(report); });
  if (mode !== "prompt") {
    for (const [event, kind] of [["prepare-exit", "quit"], ["prepare-recording-stop", "recording"], ["prepare-update", "update"]] as const) await listen<number>(event, (value) => prepareFlush(value.payload, kind));
    await listen<number>("recording-flush-complete", (event) => {
      const id = event.payload;
      void barrier.resume(id, () => refresh(() => barrier.isCurrent(id))).then((resumed) => { if (resumed) barrierKind = null; lockInterface(); }).catch((error) => { if (barrier.isCurrent(id)) { report(error); barrier.complete(id); barrierKind = null; lockInterface(); } });
    });
    for (const event of ["exit-cancelled", "update-cancelled"]) await listen<number>(event, (value) => { if (barrier.complete(value.payload)) { barrierKind = null; lockInterface(); } });
  }
  await render();
  if (mode !== "prompt") await api.frontendReady();
  booted = true;
  if (fixtureClient()) {
    window.addEventListener("focus", () => focusChanged(true)); window.addEventListener("blur", () => focusChanged(false));
  } else {
    try {
      const window = getCurrentWindow();
      void window.onFocusChanged((event) => focusChanged(event.payload));
      void window.onCloseRequested(() => { void flushAll().then(() => persistAll(mounted?.leave ?? [])).catch(report); mounted?.focus.forEach((listener) => listener(false)); });
    } catch { /* Window events are unavailable in a browser-only preview. */ }
  }
  if (capabilitiesDirty) { capabilitiesDirty = false; await refresh(); }
  if (mode === "workspace") {
    void refreshSidebar().catch(report);
    let reading = false;
    window.setInterval(() => { if (reading) return; reading = true; void refreshSidebar().catch(report).finally(() => { reading = false; }); }, 2000);
  }
}

content.addEventListener("focusout", () => { window.setTimeout(() => { if (needsRefresh && !editing() && !mounted?.info.length) void render(); }, 0); });
window.addEventListener("unhandledrejection", (event) => { event.preventDefault(); report(event.reason); });
window.addEventListener("beforeunload", () => { mounted?.cleanup.forEach((cleanup) => cleanup()); });
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (info) applyAppearance(); });
void boot().catch(report);
