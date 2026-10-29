import "./styles.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, fixtureClient, listen, listenToWindow, type AppInfo, type MeetingFinished, type MeetingRuntimeStatus, type Settings, type UpdateInfo } from "./api";
import { LatestRequest, SerialQueue, afterPaint, fillWhenReady, persistAll } from "./async";
import { controlButton, errorText, h, section, select } from "./dom";
import { dictationView, dictionaryView, stylesView } from "./views/dictation";
import { historyView } from "./views/history";
import { displayedMeetingSession, finishMeetingDrafts, flushMeetingDrafts, meetingView, meetingsView } from "./views/meetings";
import { FrontendBarrier, canShowFinished, type FrontendBarrierKind } from "./session-events";
import { deliveredRouteApplies, historyEntryId, historyRoute, legacyWorkspaceRoute, parseWorkspaceRoute, routeKey, samePadRoute, settingTabs, WorkspaceNavigation, windowForSection, windowMode, windowTitles, workspaceRoute, type WorkspaceRoute, type MeetingSourceTarget } from "./workspace-state";
import { LibrarySidebar } from "./views/library";
import { recordingEntry } from "./voice-flow-state";
import { PadNavigationBlocked } from "./dictation-session-state";
import { icon } from "./icons";
import { modelsView } from "./views/models";
import { accountView } from "./views/account";
import { providersView } from "./views/providers";
import { meetingSettingsView, recordingSettingsView } from "./views/settings";
import { promptView } from "./views/welcome";
import { onboardingView } from "./views/onboarding";
import { padView } from "./views/pad";
import { openTaskSetupDialog, settingsOverview, type TaskSetupDialog } from "./views/setup";
import type { SetupTask } from "./task-setup-state";
import { appInfoContentEqual } from "./presentation";
import { VisibilityPoll } from "./window-lifecycle";
import { bindLocalization, l, liveText, localizeBackendText, refreshLocalization, setInterfaceLanguage, t, type InterfaceLanguagePreference, type TranslationKey } from "./i18n.ts";

export interface Context {
  info: AppInfo;
  save: (change: (settings: Settings) => void) => Promise<void>;
  mutate: (work: () => Promise<AppInfo>) => Promise<void>;
  update: (info: AppInfo) => void;
  refresh: () => Promise<void>;
  rerender: () => void;
  navigate: (tab: string, param?: string, jumpMs?: number, sourceTarget?: MeetingSourceTarget) => void;
  openSetup: (task: SetupTask) => void;
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
  scrollKey: string;
  cleanup: (() => void)[];
  flush: (() => Promise<void>)[];
  leave: (() => Promise<void>)[];
  info: ((info: AppInfo) => void)[];
  focus: ((focused: boolean) => void)[];
  visibility: ((visible: boolean) => void)[];
}

const tabs: [string, TranslationKey, View][] = [
  ["overview", "navigation.overview", settingsOverview],
  ["providers", "navigation.providers", providersView], ["models", "navigation.models", modelsView],
  ["dictation", "navigation.dictation", dictationView], ["meeting-settings", "navigation.meetings", meetingSettingsView],
  ["recording", "navigation.recording", recordingSettingsView], ["dictionary", "navigation.dictionary", dictionaryView],
  ["styles", "navigation.styles", stylesView], ["appearance", "navigation.appearance", appearanceView],
  ["account", "navigation.account", accountView],
];
const query = new URLSearchParams(location.search);
const requestedWindow = query.get("window") ?? "workspace";
const mode = requestedWindow === "onboarding" ? "onboarding" : ["prompt", "record-prompt"].includes(requestedWindow) ? "prompt" : windowMode(requestedWindow);
const windowKind = mode === "onboarding" || mode === "prompt" ? "workspace" : mode;
const legacyWindow = ["settings", "dictation", "history"].includes(requestedWindow) ? requestedWindow : windowKind;
const navigation = new WorkspaceNavigation(legacyWorkspaceRoute(legacyWindow, query.get("tab")) ?? workspaceRoute("meetings"));
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
let runtimePoll: VisibilityPoll | null = null;
let pendingRoute: WorkspaceRoute | null = null;
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
let currentMeetingStatus: MeetingRuntimeStatus | null = null;
const navigationStatus = h("p", { class: "navigation-progress settings-loading help", role: "status", hidden: true }, l("common.loading"));
const contentPositions = new Map<string, number>();
const settingsPositions = new Map<string, number>();

function scrollKey(route: WorkspaceRoute): string {
  return JSON.stringify([route.section, route.section === "history" ? null : route.tab, route.meeting_id]);
}
let taskSetup: TaskSetupDialog | null = null;
let setupOpening = false;
let deferredRender = false;
let settingsFocusRequest: string | null = null;

function showRecovery() {
  const unsaved = (info?.unsaved_history_count ?? 0) > 0;
  if (!unsaved && recoveryNotice?.unsaved) recoveryNotice = null;
  recoveryBanner.hidden = !unsaved && !recoveryNotice;
  if (recoveryBanner.hidden) return;
  const notice = recoveryNotice;
  recoveryBanner.replaceChildren(
    h("span", { class: "grow" }, notice ? liveText(() => localizeBackendText(notice.reason)) : l("recovery.unsaved")),
    ...(notice ? [h("button", { class: "text-button", onclick: async () => {
      try {
        await api.copyHistory(notice.id);
        if (!notice.unsaved) { recoveryNotice = null; showRecovery(); }
      } catch (error) { report(error); }
    } }, l("common.copy"))] : []),
    h("button", { class: "text-button", onclick: () => navigate("history", notice?.id) }, l("common.open_history")),
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
  setInterfaceLanguage(info.interface_language);
  showRecovery();
  applyAppearance();
  mounted?.info.forEach((listener) => listener(visibleInfo()));
  taskSetup?.notify(visibleInfo());
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
    ...(taskSetup ? [taskSetup.flush] : []),
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
  if (taskSetup || setupOpening) { needsRefresh = true; return; }
  if (mounted?.info.length || navigation.current.section === "recording") return;
  if (editing()) { needsRefresh = true; return; }
  await render(allowed);
}

function navigate(tab: string, param?: string, jumpMs?: number, sourceTarget?: MeetingSourceTarget) {
  if (tab === "settings-back") { void go(navigation.settingsBack()); return; }
  if (tab === "onboarding") { void api.showWindow("onboarding").catch(report); return; }
  if (tab === "meetings" || tab === "library" || tab === "chat") {
    void go(workspaceRoute("meetings", tab === "chat" ? "chat" : null, param ?? null, jumpMs ?? null, sourceTarget)); return;
  }
  if (tab === "meeting" || tab === "live") { void go(workspaceRoute("recording", null, param ?? null)); return; }
  if (tab === "history") { void go(historyRoute(param)); return; }
  if (tab === "settings") { void go(workspaceRoute("settings", null)); return; }
  if (tab === "dictation-home") { void go(workspaceRoute("dictation", "home")); return; }
  if (tab === "dictation-pad") { void go(workspaceRoute("dictation", "pad")); return; }
  if (settingTabs.includes(tab as typeof settingTabs[number]) || tab === "local") void go(workspaceRoute("settings", tab));
}

async function go(route: WorkspaceRoute) {
  if (mode === "onboarding" || mode === "prompt" || windowForSection(route.section) !== mode) {
    await api.showWorkspace(route).catch(report); return;
  }
  if (route.section === "settings" && route.tab === null) route = navigation.select("settings");
  if (route.section === "dictation" && route.tab === null) route = navigation.select("dictation");
  if (mounted?.key === routeKey(route) && ((route.section === "settings" && routeKey(navigation.current) === routeKey(route))
      || samePadRoute(navigation.current, route) || (route.section === "history" && routeKey(navigation.current) === routeKey(route)))) {
    ++navigationGeneration; ++generation; content.inert = false; showPendingNavigation(null); restoreSettingsFocus(); return;
  }
  const mine = ++navigationGeneration;
  ++generation;
  showPendingNavigation(route);
  try {
    content.inert = true;
    await afterPaint();
    if (mine !== navigationGeneration) return;
    await flushAll();
    if (mine !== navigationGeneration) return;
    await persistAll(mounted?.leave ?? []);
    if (mine !== navigationGeneration) return;
    navigation.set(route);
    await render(() => mine === navigationGeneration, mine);
  } catch (error) { if (!(error instanceof PadNavigationBlocked)) report(error); }
  finally { if (mine === navigationGeneration) { content.inert = false; showPendingNavigation(null); restoreSettingsFocus(); } }
}

function showPendingNavigation(route: WorkspaceRoute | null) {
  pendingRoute = route;
  library?.setPendingRoute(route);
  navigationStatus.hidden = !route;
  if (route) content.setAttribute("aria-busy", "true"); else content.removeAttribute("aria-busy");
}

function restoreSettingsFocus() {
  if (!settingsFocusRequest || content.inert) return;
  const selected = content.querySelector<HTMLButtonElement>('.settings-categories [aria-current="page"]');
  if (selected?.dataset.settingsCategory === settingsFocusRequest) selected.focus({ preventScroll: true });
  settingsFocusRequest = null;
}

function contextFor(target: Mounted, reportError: (error: unknown) => void = report): Context {
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
      }).catch((error: unknown) => { reportError(error); throw error; });
    },
    mutate: (work) => settingsQueue.run(async () => { await flushSettings(); info = await work(); notifyInfo(); banner.hidden = true; }).then(async () => {
      if (!target.info.length && mounted === target) await render();
    }).catch((error: unknown) => { reportError(error); throw error; }),
    update: (next) => { info = next; notifyInfo(); },
    refresh,
    rerender: () => { if (mounted === target) void render().catch(report); },
    navigate,
    openSetup: (task) => { void openSetup(task, target).catch(report); },
    report: reportError,
    onDispose: (cleanup) => target.cleanup.push(cleanup),
    beforeLeave: (flush) => target.leave.push(flush),
    onFlush: (flush) => target.flush.push(flush),
    onInfoChange: (listener) => target.info.push(listener),
    onFocusChange: (listener) => target.focus.push(listener),
    get visible() { return presentationVisible(); },
    onVisibilityChange: (listener) => target.visibility.push(listener),
    param: route.section === "dictation" && route.tab === "pad" ? "__pad__" : route.section === "settings" && route.tab === "local" ? "__local__" : route.section === "history" ? historyEntryId(route.tab) ?? undefined : route.tab === "chat" ? "__chat__" : route.meeting_id ?? undefined,
    jumpMs: route.jump_ms ?? undefined,
    sourceTarget: route.source_target ?? undefined,
  };
}

async function openSetup(task: SetupTask, origin: Mounted) {
  if (taskSetup) { taskSetup.focus(); return; }
  if (setupOpening || mounted !== origin) return;
  setupOpening = true;
  try {
    await flushAll();
    if (mounted !== origin) return;
    const scope: Mounted = { key: `setup:${task}`, scrollKey: origin.scrollKey, cleanup: [], flush: [], leave: [], info: [], focus: [], visibility: [] };
    // The setup owns error presentation; a repaired task must not leave an error behind its dialog.
    taskSetup = openTaskSetupDialog(contextFor(scope, () => undefined), task, flushAll, () => {
      taskSetup = null;
      const refreshNeeded = deferredRender || needsRefresh;
      deferredRender = false; needsRefresh = false;
      if (refreshNeeded && (!mounted?.info.length || mounted.key !== routeKey(navigation.current))) void render().catch(report);
    });
  } finally { setupOpening = false; }
}

async function settingsView(ctx: Context): Promise<HTMLElement> {
  const current = navigation.current.tab === "local" ? "providers" : navigation.current.tab ?? "overview";
  const tabButton = ([id, label]: [string, TranslationKey, View]) => h("button", {
    "aria-current": id === current ? "page" : null, "data-settings-category": id, onclick: (event: Event) => {
      settingsFocusRequest = id;
      nav.querySelectorAll("button").forEach((button) => button.removeAttribute("aria-current"));
      (event.currentTarget as HTMLElement).setAttribute("aria-current", "page");
      navigate(id);
    },
  }, l(label));
  const group = (label: TranslationKey, ids: string[]) => [h("p", { class: "settings-nav-group" }, l(label)), ...ids.map((id) => tabButton(tabs.find((tab) => tab[0] === id)!))];
  const nav = h("nav", { class: "settings-categories", "aria-label": l("navigation.settings_categories") },
    h("button", { class: "settings-back", "data-action": "back-to-work", onclick: () => navigate("settings-back") }, icon("arrowLeft"), l("navigation.back_to_work")),
    h("button", { class: "settings-recording", hidden: true, onclick: async () => {
      try { const status = await api.meetingRuntimeStatus(); currentMeetingStatus = status; const entry = recordingEntry(status); await go(entry.kind === "new" ? navigation.settingsBack() : workspaceRoute("recording", null, entry.meetingId)); }
      catch (error) { report(error); }
    } }),
    tabButton(tabs[0]),
    ...group("navigation.your_work", ["dictation", "meeting-settings", "recording", "dictionary", "styles"]),
    ...group("navigation.app", ["appearance", "account"]),
    ...group("navigation.advanced", ["providers", "models"]));
  const selected = tabs.find(([id]) => id === current) ?? tabs[0];
  const pane = h("section", { class: "settings-pane" });
  const positionKey = scrollKey(navigation.current);
  const content = async () => [...(current === "overview" ? [] : [h("h2", { class: "settings-title" }, l(selected[1]))]), await selected[2](ctx), settingsFooter(ctx)];
  // A re-render of the same category keeps its old content until the new one is ready, so the
  // scroll position survives; a new category appears at once and fills in as its data arrives.
  if (rerendering) pane.append(...await content());
  else {
    let live = true;
    ctx.onDispose(() => { live = false; });
    const loading = h("p", { class: "help settings-loading", role: "status" }, l("common.loading"));
    void fillWhenReady(pane, loading, content, () => live).then(() => { refreshLocalization(pane); pane.scrollTop = settingsPositions.get(positionKey) ?? 0; }).catch((error: unknown) => {
      if (live) pane.replaceChildren(h("p", { class: "error", role: "status" }, errorText(error)));
      report(error);
    });
  }
  return h("div", { class: "settings-workspace" }, h("h1", { class: "visually-hidden" }, l("window.settings")), h("div", { class: "settings-layout" }, nav, pane));
}

function appearanceView(ctx: Context): HTMLElement {
  const picker = select([["system", l("appearance.system")], ["light", l("appearance.light")], ["dark", l("appearance.dark")]], ctx.info.settings.appearance ?? "system", (value) => {
    void ctx.save((settings) => { settings.appearance = value; }).catch(ctx.report);
  });
  picker.setAttribute("aria-label", t("appearance.title"));
  const systemLabel = l("interface.system", () => ({ language: ctx.info.system_interface_language === "nl" ? "Nederlands" : "English" }));
  const interfacePicker = select<InterfaceLanguagePreference>([["system", systemLabel], ["en", "English"], ["nl", "Nederlands"]], ctx.info.settings.ui_language ?? "system", (value) => {
    void ctx.save((settings) => { settings.ui_language = value; }).catch(ctx.report);
  }, "menu");
  const updateLabels = () => {
    picker.setAttribute("aria-label", t("appearance.title"));
    interfacePicker.setAttribute("aria-label", t("interface.language"));
  };
  updateLabels();
  bindLocalization(interfacePicker, updateLabels);
  ctx.onInfoChange((next) => { picker.value = next.settings.appearance; interfacePicker.value = next.settings.ui_language ?? "system"; });
  return h("div", {}, section(l("interface.language"), l("interface.language_help"), interfacePicker),
    section(l("appearance.title"), l("appearance.help"), h("div", { class: "preference-row" }, h("span", {}, l("appearance.title")), picker)),
    section(l("intro.title"), l("intro.help"),
      controlButton({ class: "btn onboarding-replay", onclick: () => api.showWindow("onboarding", "language") }, l("intro.open"))));
}

/** As in the original library, the main window opens on the newest meeting when none is chosen. */
async function chooseMeeting() {
  const route = navigation.current;
  if (mode !== "workspace" || route.section !== "meetings" || route.meeting_id || route.tab === "chat") return;
  const first = (await api.meetings(""))[0];
  if (first && navigation.current === route) navigation.set(workspaceRoute("meetings", null, first.id));
}

async function render(allowed: () => boolean = () => true, request?: number) {
  if (!allowed()) return;
  if (pendingRoute && request !== navigationGeneration) { needsRefresh = true; return; }
  if (taskSetup || setupOpening) { deferredRender = true; return; }
  const mine = ++generation;
  await chooseMeeting().catch(report);
  if (mine !== generation) return;
  const route = navigation.current;
  const target: Mounted = { key: routeKey(route), scrollKey: scrollKey(route), cleanup: [], flush: [], leave: [], info: [], focus: [], visibility: [] };
  rerendering = mounted?.key === target.key;
  const ctx = contextFor(target);
  const view = mode === "onboarding" ? onboardingView : mode === "prompt" ? promptView
    : route.section === "settings" ? settingsView : route.section === "history" ? historyView
    : route.section === "recording" ? meetingView : route.section === "meetings" ? meetingsView : padView;
  try {
    const node = await view(ctx);
    if (mine !== generation || !allowed()) { target.cleanup.forEach((cleanup) => cleanup()); return; }
    if (mounted) {
      contentPositions.set(mounted.scrollKey, content.scrollTop);
      const previousPane = content.querySelector<HTMLElement>(".settings-pane");
      if (previousPane) settingsPositions.set(mounted.scrollKey, previousPane.scrollTop);
    }
    const paneScroll = settingsPositions.get(target.scrollKey);
    mounted?.cleanup.forEach((cleanup) => cleanup());
    mounted = target;
    target.visibility.forEach((listener) => listener(presentationVisible()));
    content.replaceChildren(node);
    refreshLocalization(node);
    content.scrollTop = contentPositions.get(target.scrollKey) ?? 0;
    if (paneScroll !== undefined) { const pane = content.querySelector<HTMLElement>(".settings-pane"); if (pane) pane.scrollTop = paneScroll; }
    const selectedCategory = node.querySelector<HTMLButtonElement>('.settings-categories [aria-current="page"]');
    selectedCategory?.scrollIntoView({ block: "nearest", inline: "nearest" });
    restoreSettingsFocus();
    if (ctx.jumpMs === route.jump_ms) navigation.current.jump_ms = null;
    if (ctx.sourceTarget === route.source_target) delete navigation.current.source_target;
    if (route.section === "history" && ctx.param === historyEntryId(route.tab)) navigation.current.tab = null;
    needsRefresh = false;
    updateNavigation();
  } catch (error) {
    target.cleanup.forEach((cleanup) => cleanup());
    if (mine === generation && allowed()) report(error);
  }
}

function updateNavigation() {
  if (library) library.element.hidden = navigation.current.section === "settings";
  const visible = presentationVisible() && navigation.current.section !== "settings";
  library?.setVisible(visible);
  sidebarPoll?.setVisible(visible);
  library?.highlight();
  updateSettingsRecording();
  if (navigation.current.section === "settings" && !currentMeetingStatus) void library?.refreshLive().catch(report);
}

function updateSettingsRecording() {
  const button = content.querySelector<HTMLButtonElement>(".settings-recording");
  if (!button || !currentMeetingStatus) return;
  const entry = recordingEntry(currentMeetingStatus);
  button.hidden = entry.kind === "new";
  button.replaceChildren(entry.kind === "recovery" ? icon("warning") : h("span", { class: "status-dot" }),
    h("span", {}, l(entry.kind === "recovery" ? "meetings.recording_needs_attention" : "meetings.return_recording")));
}

/** On macOS the transparent system title bar overlays the webview; this strip keeps it draggable. */
function titlebarDragRegion(): HTMLElement {
  return h("div", { class: "titlebar-drag", "data-tauri-drag-region": true, "aria-hidden": "true" });
}

function workspaceShell() {
  library = new LibrarySidebar({ info: visibleInfo, current: () => navigation.current, go: (route) => { void go(route); }, report,
    select: (section) => { void go(navigation.select(section)); },
    statusChanged: (status) => { currentMeetingStatus = status; updateSettingsRecording(); } });
  app.replaceChildren(titlebarDragRegion(), h("div", { class: "workspace-shell" }, library.element, h("div", { class: "workspace-body" }, recoveryBanner, banner, content)));
}

async function refreshSidebar() {
  if (mode !== "workspace") return;
  await library?.invalidate();
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
      await api.frontendFlushed(requestId, success, success ? null : t("common.transition_busy"));
    }
    catch (error) {
      if (barrier.isCurrent(requestId)) report(error);
      try { await api.frontendFlushed(requestId, false, errorText(error)); } catch (ackError) { if (barrier.isCurrent(requestId)) report(ackError); }
    }
  })();
}

function meetingFinished(event: MeetingFinished) {
  finishMeetingDrafts(event);
  void finishedRequests.run(api.meetingRuntimeStatus, (status) => {
    const accepted = canShowFinished(status, event.session_id, displayedMeetingSession());
    if (accepted && event.id) navigation.recordingFinished(event.id);
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
  let message = () => info.preview ? t("updates.preview_disabled") : "";
  const status = h("span", { class: "help", role: "status" }, liveText(() => message()));
  const showStatus = (next: () => string) => { message = next; status.textContent = message(); };
  bindLocalization(status, () => { status.textContent = message(); });
  const install = controlButton({ class: "btn", hidden: true, disabled: info.preview }, l("updates.install"));
  install.addEventListener("click", async () => {
    install.disabled = true; showStatus(() => t("updates.downloading"));
    try { await api.installUpdate(); showStatus(() => t("updates.installed")); }
    catch (error) { showStatus(() => errorText(error)); install.disabled = false; }
  });
  const check = controlButton({ class: "btn", disabled: info.preview, title: info.preview ? l("updates.preview_disabled") : null, onclick: async () => {
    check.disabled = true; showStatus(() => t("updates.checking"));
    try { const update: UpdateInfo = await api.checkForUpdates(); showStatus(() => update.available ? t("updates.available", { version: update.version ?? "" }) : t("updates.current")); install.hidden = !update.available; }
    catch (error) { showStatus(() => errorText(error)); }
    check.disabled = false;
  } }, l("updates.check"));
  let disposed = false;
  ctx.onDispose(() => { disposed = true; });
  void listen<UpdateInfo>("update-available", (event) => {
    if (disposed || info.preview) return;
    install.hidden = !event.payload.available;
    if (event.payload.available) showStatus(() => t("updates.available", { version: event.payload.version ?? "" }));
  }).then((unlisten) => { if (disposed) unlisten(); else ctx.onDispose(unlisten); }).catch(report);
  return h("footer", { class: "settings-footer row" }, h("span", { class: "help" }, `Pennel ${info.version}${info.preview ? " · Preview" : ""}`), h("span", { class: "grow" }), status, check, install);
}

function focusChanged(focused: boolean) {
  document.documentElement.dataset.windowFocused = String(focused);
  if (!presentationVisible() || barrier.locked) return;
  mounted?.focus.forEach((listener) => listener(focused));
  taskSetup?.focusChanged(focused);
  if (focused) {
    void refresh(() => true, true).catch(report);
    void refreshSidebar().catch(report);
    void library?.refreshLive().catch(report);
  }
}

function windowShortcuts() {
  window.addEventListener("keydown", (event) => {
    if (!(info.platform === "macos" ? event.metaKey : event.ctrlKey) || event.altKey || event.shiftKey) return;
    const key = event.key.toLowerCase();
    if (key === ",") { event.preventDefault(); navigate("settings"); }
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
  const sidebarVisible = visible && navigation.current.section !== "settings";
  library?.setVisible(sidebarVisible);
  sidebarPoll?.setVisible(sidebarVisible);
  runtimePoll?.setVisible(visible);
  mounted?.visibility.forEach((listener) => listener(visible));
  taskSetup?.visibilityChanged(visible);
}

async function boot() {
  document.body.dataset.window = mode;
  info = await api.info(); applyAppearance();
  setInterfaceLanguage(info.interface_language ?? "en");
  const updateTitle = () => { document.title = mode === "onboarding" ? t("window.onboarding") : mode === "prompt" ? "Pennel" : windowTitles[mode]; };
  updateTitle();
  bindLocalization(document.documentElement, updateTitle);
  bindLocalization(banner, () => { if (banner.textContent) banner.textContent = localizeBackendText(banner.textContent); });
  document.documentElement.dataset.platform = info.platform;
  windowVisible = !!fixtureClient() || info.platform !== "macos" || mode === "prompt";
  if (mode === "workspace") workspaceShell(); else app.replaceChildren(...(mode === "prompt" ? [] : [titlebarDragRegion(), recoveryBanner]), banner, content);
  if (mode !== "prompt") app.append(navigationStatus);
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
  await listen("library-changed", () => { void refreshSidebar().catch(report); });
  await listen<string>("meeting-updated", (event) => {
    void refreshSidebar().catch(report);
    if (navigation.current.section === "meetings" && (typeof event.payload !== "string" || event.payload === navigation.current.meeting_id)) {
      if (editing()) needsRefresh = true; else void render();
    }
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
    for (const event of ["exit-cancelled", "update-cancelled"]) await listen<number>(event, (value) => {
      if (barrier.complete(value.payload)) {
        lockInterface();
        if (event === "exit-cancelled") window.dispatchEvent(new Event("pennel-exit-cancelled"));
      }
    });
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
    sidebarPoll = new VisibilityPoll(30_000, async () => {
      if (!started) { await sidebar.start(); started = true; }
      else await refreshSidebar();
    }, report);
    runtimePoll = new VisibilityPoll(2000, () => sidebar.refreshLive(), report);
    notifyVisibility();
  }
}

content.addEventListener("focusout", () => { window.setTimeout(() => { if (needsRefresh && !editing() && !mounted?.info.length) void render(); }, 0); });
window.addEventListener("unhandledrejection", (event) => { event.preventDefault(); report(event.reason); });
window.addEventListener("beforeunload", () => { sidebarPoll?.dispose(); runtimePoll?.dispose(); library?.dispose(); mounted?.cleanup.forEach((cleanup) => cleanup()); });
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { if (info) applyAppearance(); });
void boot().catch(report);
