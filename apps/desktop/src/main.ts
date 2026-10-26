import "./styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, fixtureClient, listen, listenToWindow, type AppInfo, type MeetingFinished, type Settings, type UpdateInfo } from "./api";
import { LatestRequest, SerialQueue, fillWhenReady, persistAll } from "./async";
import { errorText, h, section, select } from "./dom";
import { dictationView, dictionaryView, stylesView } from "./views/dictation";
import { historyView } from "./views/history";
import { displayedMeetingSession, finishMeetingDrafts, flushMeetingDrafts, meetingView, meetingsView } from "./views/meetings";
import { FrontendBarrier, canShowFinished, type FrontendBarrierKind } from "./session-events";
import { deliveredRouteApplies, legacyWorkspaceRoute, parseWorkspaceRoute, routeKey, samePadRoute, settingTabs, WorkspaceNavigation, windowForSection, windowMode, windowTitles, workspaceRoute, type WorkspaceRoute, type MeetingSourceTarget } from "./workspace-state";
import { LibrarySidebar } from "./views/library";
import { modelsView } from "./views/models";
import { accountView } from "./views/account";
import { providersView } from "./views/providers";
import { meetingSettingsView, recordingSettingsView } from "./views/settings";
import { promptView } from "./views/welcome";
import { onboardingView } from "./views/onboarding";
import { padView } from "./views/pad";
import { appInfoContentEqual } from "./presentation";
import { VisibilityPoll } from "./window-lifecycle";

export interface Context {
  info: AppInfo;
  save: (change: (settings: Settings) => void) => Promise<void>;
  mutate: (work: () => Promise<AppInfo>) => Promise<void>;
  update: (info: AppInfo) => void;
  refresh: () => Promise<void>;
  rerender: () => void;
  navigate: (tab: string, param?: string, jumpMs?: number, sourceTarget?: MeetingSourceTarget) => void;
  report: (error: unknown) => void;
  onDispose: (cleanup: () => void) => void;
  beforeLeave: (flush: () => Promise<void>) => void;
  onFlush: (flush: () => Promise<void>) => void;
  onInfoChange: (listener: (info: AppInfo) => void) => void;
  onFocusChange: (listener: (focused: boolean) => void) => void;
  readonly visible: boolean;
  onVisibilityChange: (listener: (visible: boolean) => void) => void;
  param?: string;
  jumpMs?: number;
  sourceTarget?: MeetingSourceTarget;
}

type View = (ctx: Context) => HTMLElement | Promise<HTMLElement>;
interface Mounted {
  key: string;
  cleanup: (() => void)[];
  flush: (() => Promise<void>)[];
  leave: (() => Promise<void>)[];
  info: ((info: AppInfo) => void)[];
  focus: ((focused: boolean) => void)[];
  visibility: ((visible: boolean) => void)[];
}

const tabs: [string, string, View][] = [
  ["providers", "Providers", providersView], ["models", "Modellen", modelsView],
  ["dictation", "Dicteren", dictationView], ["meeting-settings", "Meetings", meetingSettingsView],
  ["recording", "Opnemen", recordingSettingsView], ["dictionary", "Woordenlijst", dictionaryView],
  ["styles", "Stijlen", stylesView], ["appearance", "Weergave", appearanceView],
  ["account", "Account", accountView],
];
const query = new URLSearchParams(location.search);
const requestedWindow = query.get("window") ?? "workspace";
const mode = requestedWindow === "onboarding" ? "onboarding" : ["prompt", "record-prompt"].includes(requestedWindow) ? "prompt" : windowMode(requestedWindow);
const windowKind = mode === "onboarding" || mode === "prompt" ? "workspace" : mode;
const navigation = new WorkspaceNavigation(legacyWorkspaceRoute(windowKind, query.get("tab")) ?? workspaceRoute("meetings"));
if (fixtureClient() && query.has("section")) {
  const route = parseWorkspaceRoute({ section: query.get("section"), tab: query.get("tab"), meeting_id: query.get("meeting_id"), jump_ms: query.has("jump_ms") ? Number(query.get("jump_ms")) : null });
  if (route && windowForSection(route.section) === windowKind) navigation.set(route);
}
let info: AppInfo;
let mounted: Mounted | null = null;
let generation = 0;
let rerendering = false;
let navigationGeneration = 0;
let booted = false;
let capabilitiesDirty = false;
let needsRefresh = false;
let guardedFinishSeen = false;
let windowVisible = true;
let sidebarPoll: VisibilityPoll | null = null;
const settingsQueue = new SerialQueue();
const pendingSettings: ((settings: Settings) => void)[] = [];
const barrier = new FrontendBarrier();
const finishedRequests = new LatestRequest();
const app = document.getElementById("app")!;
const content = h("main", { id: "content" });
const banner = h("div", { class: "notice-banner", role: "status", hidden: true });
const recoveryBanner = h("div", { class: "notice-banner row", "data-recovery-banner": true, role: "status", "aria-live": "polite", hidden: true });
type RecoveryNotice = { id: string; unsaved: boolean; reason: string };
let recoveryNotice: RecoveryNotice | null = null;
let library: LibrarySidebar | null = null;

function showRecovery() {
  const unsaved = (info?.unsaved_history_count ?? 0) > 0;
  if (!unsaved && recoveryNotice?.unsaved) recoveryNotice = null;
  recoveryBanner.hidden = !unsaved && !recoveryNotice;
  if (recoveryBanner.hidden) return;
  const notice = recoveryNotice;
  recoveryBanner.replaceChildren(
    h("span", { class: "grow" }, notice?.reason ?? "Je hebt dictaten die nog niet bewaard zijn. Ze blijven beschikbaar zolang Pennel open is."),
    ...(notice ? [h("button", { class: "text-button", onclick: async () => {
      try {
        await api.copyHistory(notice.id);
        if (!notice.unsaved) { recoveryNotice = null; showRecovery(); }
      } catch (error) { report(error); }
    } }, "Kopiëren")] : []),
    h("button", { class: "text-button", onclick: () => { void api.showWindow("history").catch(report); } }, "Open geschiedenis"),
  );
}

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
}

function notifyInfo() {
  showRecovery();
  applyAppearance();
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

function navigate(tab: string, param?: string, jumpMs?: number, sourceTarget?: MeetingSourceTarget) {
  if (tab === "onboarding") { void api.showWindow("onboarding").catch(report); return; }
  if (tab === "meetings" || tab === "library" || tab === "chat") {
    void go(workspaceRoute("meetings", tab === "chat" ? "chat" : null, param ?? null, jumpMs ?? null, sourceTarget)); return;
  }
  if (tab === "meeting" || tab === "live") { void go(workspaceRoute("recording", null, param ?? null)); return; }
  if (tab === "history") { void go(workspaceRoute("history")); return; }
  if (tab === "settings") { void go(workspaceRoute("settings", null)); return; }
  if (tab === "dictation-home") { void go(workspaceRoute("dictation")); return; }
  if (tab === "dictation-pad") { void go(workspaceRoute("dictation", "pad")); return; }
  if (settingTabs.includes(tab as typeof settingTabs[number]) || tab === "local") void go(workspaceRoute("settings", tab));
}

async function go(route: WorkspaceRoute) {
  if (mode === "onboarding" || mode === "prompt" || windowForSection(route.section) !== mode) {
    // Settings opened without a category keeps the category its window already shows.
    if (route.section === "settings" && route.tab === null) { await api.showWindow("settings").catch(report); return; }
    await api.showWorkspace(route).catch(report); return;
  }
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
    get visible() { return presentationVisible(); },
    onVisibilityChange: (listener) => target.visibility.push(listener),
    param: route.section === "dictation" && route.tab === "pad" ? "__pad__" : route.section === "settings" && route.tab === "local" ? "__local__" : route.tab === "chat" ? "__chat__" : route.meeting_id ?? undefined,
    jumpMs: route.jump_ms ?? undefined,
    sourceTarget: route.source_target ?? undefined,
  };
}

async function settingsView(ctx: Context): Promise<HTMLElement> {
  const current = navigation.current.tab === "local" ? "providers" : navigation.current.tab ?? "providers";
  const nav = h("nav", { class: "settings-categories segmented", "aria-label": "Instellingencategorieën" }, tabs.map(([id, label]) => h("button", {
    "aria-current": id === current ? "page" : null, onclick: (event: Event) => {
      nav.querySelectorAll("button").forEach((button) => button.removeAttribute("aria-current"));
      (event.currentTarget as HTMLElement).setAttribute("aria-current", "page");
      navigate(id);
    },
  }, label)));
  const selected = tabs.find(([id]) => id === current) ?? tabs[0];
  const pane = h("section", { class: "settings-pane" });
  const content = async () => [await selected[2](ctx), settingsFooter(ctx)];
  // A re-render of the same category keeps its old content until the new one is ready, so the
  // scroll position survives; a new category appears at once and fills in as its data arrives.
  if (rerendering) pane.append(...await content());
  else {
    let live = true;
    ctx.onDispose(() => { live = false; });
    const loading = h("p", { class: "help settings-loading", role: "status" }, "Laden…");
    void fillWhenReady(pane, loading, content, () => live).catch((error: unknown) => {
      if (live) pane.replaceChildren(h("p", { class: "error", role: "status" }, errorText(error)));
      report(error);
    });
  }
  return h("div", { class: "settings-workspace" }, h("h1", { class: "visually-hidden" }, "Instellingen"), h("div", { class: "settings-layout" }, nav, pane));
}

function appearanceView(ctx: Context): HTMLElement {
  const picker = select([["system", "Volg systeem"], ["light", "Licht"], ["dark", "Donker"]], ctx.info.settings.appearance ?? "system", (value) => {
    void ctx.save((settings) => { settings.appearance = value; });
  });
  picker.setAttribute("aria-label", "Weergave");
  ctx.onInfoChange((next) => { picker.value = next.settings.appearance; });
  return h("div", {}, section("Weergave", "Volg systeem past de lichte of donkere weergave aan met je apparaat.", h("div", { class: "preference-row" }, h("span", {}, "Weergave"), picker)),
    section("Introductie", "Toon de eerste stappen opnieuw: spraakherkenning, toegang en een oefendictaat.",
      h("button", { class: "btn onboarding-replay", onclick: () => api.showWindow("onboarding", "welcome") }, "Introductie opnieuw openen")));
}

/** As in the original library, the main window opens on the newest meeting when none is chosen. */
async function chooseMeeting() {
  const route = navigation.current;
  if (mode !== "workspace" || route.section !== "meetings" || route.meeting_id || route.tab === "chat") return;
  const first = (await api.meetings(""))[0];
  if (first && navigation.current === route) navigation.set(workspaceRoute("meetings", null, first.id));
}

async function render(allowed: () => boolean = () => true) {
  if (!allowed()) return;
  const mine = ++generation;
  await chooseMeeting().catch(report);
  if (mine !== generation) return;
  const route = navigation.current;
  const target: Mounted = { key: routeKey(route), cleanup: [], flush: [], leave: [], info: [], focus: [], visibility: [] };
  rerendering = mounted?.key === target.key;
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
    target.visibility.forEach((listener) => listener(presentationVisible()));
    const scroll = content.scrollTop;
    content.replaceChildren(node);
    content.scrollTop = scroll;
    if (paneScroll !== undefined) { const pane = content.querySelector<HTMLElement>(".settings-pane"); if (pane) pane.scrollTop = paneScroll; }
    if (ctx.jumpMs === route.jump_ms) navigation.current.jump_ms = null;
    if (ctx.sourceTarget === route.source_target) delete navigation.current.source_target;
    needsRefresh = false;
    updateNavigation();
  } catch (error) {
    target.cleanup.forEach((cleanup) => cleanup());
    if (mine === generation && allowed()) report(error);
  }
}

function updateNavigation() {
  library?.highlight();
}

/** On macOS the transparent system title bar overlays the webview; this strip keeps it draggable. */
function titlebarDragRegion(): HTMLElement {
  return h("div", { class: "titlebar-drag", "data-tauri-drag-region": true, "aria-hidden": "true" });
}

function workspaceShell() {
  library = new LibrarySidebar({ info: visibleInfo, current: () => navigation.current, go: (route) => { void go(route); }, report });
  app.replaceChildren(titlebarDragRegion(), h("div", { class: "workspace-shell" }, library.element, h("div", { class: "workspace-body" }, recoveryBanner, banner, content)));
}

async function refreshSidebar() {
  if (mode !== "workspace" || !presentationVisible()) return;
  await library?.refresh();
}

function lockInterface() {
  app.inert = barrier.locked;
  if (barrier.locked) app.setAttribute("aria-busy", "true"); else app.removeAttribute("aria-busy");
}

function prepareFlush(requestId: number, kind: FrontendBarrierKind) {
  const target = mounted;
  const preparing = barrier.prepare(requestId, flushAll, kind === "hide" ? () => persistAll(target?.leave ?? []) : undefined, kind);
  if (barrier.isCurrent(requestId)) {
    lockInterface();
    if (kind === "hide") notifyVisibility();
  }
  void (async () => {
    try {
      const success = await preparing;
      await api.frontendFlushed(requestId, success, success ? null : "Een andere vensteractie is al bezig.");
    }
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
    if (accepted && showing && (barrier.kind === null || barrier.kind === "recording")) {
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
  if (!presentationVisible() || barrier.locked) return;
  mounted?.focus.forEach((listener) => listener(focused));
  if (focused) void refresh(() => true, true).catch(report);
}

function windowShortcuts() {
  window.addEventListener("keydown", (event) => {
    if (!(info.platform === "macos" ? event.metaKey : event.ctrlKey) || event.altKey || event.shiftKey) return;
    const key = event.key.toLowerCase();
    if (key === ",") { event.preventDefault(); void api.showWindow("settings").catch(report); }
    if (key === "w" && !fixtureClient()) {
      event.preventDefault();
      try { void getCurrentWindow().close().catch(report); } catch { /* Window APIs are unavailable in a browser-only preview. */ }
    }
  });
}

function presentationVisible(): boolean {
  return windowVisible && barrier.kind !== "hide";
}

function notifyVisibility() {
  const visible = presentationVisible();
  sidebarPoll?.setVisible(visible);
  mounted?.visibility.forEach((listener) => listener(visible));
}

async function boot() {
  document.body.dataset.window = mode;
  document.title = mode === "onboarding" ? "Welkom bij Pennel" : mode === "prompt" ? "Pennel" : windowTitles[mode];
  info = await api.info(); applyAppearance();
  document.documentElement.dataset.platform = info.platform;
  windowVisible = !!fixtureClient() || info.platform !== "macos" || mode === "prompt";
  if (mode === "workspace") workspaceShell(); else app.replaceChildren(...(mode === "prompt" ? [] : [titlebarDragRegion(), recoveryBanner]), banner, content);
  if (mode !== "prompt") showRecovery();
  if (mode !== "prompt") windowShortcuts();
  await listen("capabilities-changed", () => { if (!booted) capabilitiesDirty = true; else void refresh().catch(report); });
  await listenToWindow<unknown>("workspace-navigate", (event) => { const route = parseWorkspaceRoute(event.payload); if (route && deliveredRouteApplies(mode, route)) void go(route); });
  await listenToWindow<string>("navigate", (event) => {
    if (mode === "onboarding") { window.dispatchEvent(new CustomEvent("pennel-onboarding-route", { detail: event.payload })); return; }
    const route = legacyWorkspaceRoute("workspace", event.payload); if (route && deliveredRouteApplies(mode, route)) void go(route);
  });
  await listen("settings-changed", () => { void refresh(() => true, true).catch(report); });
  await listenToWindow<{ visible: boolean }>("workspace-visibility-changed", (event) => {
    windowVisible = event.payload.visible;
    notifyVisibility();
    if (presentationVisible()) void refresh(() => presentationVisible(), true).catch(report);
  });
  await listenToWindow<string>("window-hide-error", (event) => report(event.payload));
  await listen<RecoveryNotice>("dictation-recovery", (event) => {
    recoveryNotice = event.payload;
    if (event.payload.unsaved) info.unsaved_history_count = Math.max(1, info.unsaved_history_count ?? 0);
    showRecovery();
  });
  await listen("history-changed", () => { void refresh(() => true, true).catch(report); });
  await listen("meeting-updated", () => {
    void refreshSidebar().catch(report);
    if (navigation.current.section === "meetings") { if (editing()) needsRefresh = true; else void render(); }
  });
  await listen<MeetingFinished>("meeting-finished", (event) => { guardedFinishSeen = true; meetingFinished(event.payload); });
  await listen("meeting-stopped", () => { if (!guardedFinishSeen && ["recording", "meetings"].includes(navigation.current.section)) void render(); void refreshSidebar().catch(report); });
  if (mode !== "prompt") {
    for (const [event, kind] of [["prepare-exit", "quit"], ["prepare-recording-stop", "recording"], ["prepare-update", "update"], ["prepare-hide", "hide"]] as const) await listenToWindow<number>(event, (value) => prepareFlush(value.payload, kind));
    await listenToWindow<number>("hide-flush-complete", (event) => {
      if (barrier.complete(event.payload, "hide")) { lockInterface(); notifyVisibility(); }
    });
    await listen<number>("recording-flush-complete", (event) => {
      const id = event.payload;
      void barrier.resume(id, () => refresh(() => barrier.isCurrent(id))).then(() => { lockInterface(); }).catch((error) => { if (barrier.isCurrent(id)) { report(error); barrier.complete(id); lockInterface(); } });
    });
    for (const event of ["exit-cancelled", "update-cancelled"]) await listen<number>(event, (value) => { if (barrier.complete(value.payload)) lockInterface(); });
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
      if (info.platform !== "macos") void window.onCloseRequested(() => { void flushAll().then(() => persistAll(mounted?.leave ?? [])).catch(report); mounted?.focus.forEach((listener) => listener(false)); });
    } catch { /* Window events are unavailable in a browser-only preview. */ }
  }
  if (capabilitiesDirty) { capabilitiesDirty = false; await refresh(); }
  if (mode === "workspace" && library) {
    const sidebar = library;
    let started = false;
    sidebarPoll = new VisibilityPoll(2000, async () => {
      if (!started) { await sidebar.start(); started = true; }
      else await refreshSidebar();
    }, report);
    sidebarPoll.setVisible(presentationVisible());
  }
}

content.addEventListener("focusout", () => { window.setTimeout(() => { if (needsRefresh && !editing() && !mounted?.info.length) void render(); }, 0); });
window.addEventListener("unhandledrejection", (event) => { event.preventDefault(); report(event.reason); });
window.addEventListener("beforeunload", () => { sidebarPoll?.dispose(); mounted?.cleanup.forEach((cleanup) => cleanup()); });
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (info) applyAppearance(); });
void boot().catch(report);
