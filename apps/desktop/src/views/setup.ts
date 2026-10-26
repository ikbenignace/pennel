import { api, type AppInfo, type PermissionInfo, type SetupReadiness } from "../api";
import { errorText, h, menuButton } from "../dom";
import { bindLocalization, l, localizeBackendText, refreshLocalization, t, type TranslationKey } from "../i18n.ts";
import type { Context } from "../main";
import { providerDestination } from "../processing-state";
import { taskFingerprint } from "../provider-verification";
import { TaskSetupScope, taskConfiguration, type SetupTask, type TaskConfiguration } from "../task-setup-state";
import { accountView } from "./account";
import { taskModelView } from "./models";
import { providerDetails } from "./providers";

const tasks: SetupTask[] = ["dictation_stt", "meeting_stt", "llm"];
const titleKeys: Record<SetupTask, TranslationKey> = {
  dictation_stt: "settings.setup.dictation_title", meeting_stt: "settings.setup.meeting_title", llm: "settings.setup.text_title",
};
const helpKeys: Record<SetupTask, TranslationKey> = {
  dictation_stt: "settings.setup.dictation_help", meeting_stt: "settings.setup.meeting_help", llm: "settings.setup.text_help",
};

function configurationLabel(configuration: TaskConfiguration): string {
  return configuration.reason ? localizeBackendText(configuration.reason) : t(`settings.setup.state.${configuration.state}`);
}

async function taskState(info: AppInfo, task: SetupTask): Promise<{ configuration: TaskConfiguration; speech: SetupReadiness | null }> {
  const local = info.settings.providers.find((provider) => provider.id === info.settings[task].provider_id)?.kind === "local";
  const downloaded = local ? (await api.localModels(null, false)).filter((model) => model.state.kind === "downloaded").map((model) => model.id) : [];
  const configuration = taskConfiguration(info, task, downloaded);
  const speech = task === "llm" ? null : await api.onboardingReadiness(task === "meeting_stt" ? "meeting" : "dictation_pad");
  return { configuration, speech };
}

export function settingsOverview(ctx: Context): HTMLElement {
  let disposed = false;
  let revision = 0;
  const rows = new Map<SetupTask, { status: HTMLElement; action: HTMLButtonElement }>();
  const root = h("div", { class: "settings-overview" },
    h("h2", {}, l("settings.setup.overview_title")), h("p", { class: "help" }, l("settings.setup.overview_help")),
    ...tasks.map((task) => {
      const status = h("p", { class: "settings-task-status help", role: "status" }, l("common.loading"));
      const action = h("button", { class: "btn", "data-setup-task": task, onclick: () => ctx.openSetup(task) }, l("settings.setup.finish"));
      rows.set(task, { status, action });
      return h("section", { class: "settings-task-row" },
        h("div", { class: "settings-task-copy" }, h("h3", {}, l(titleKeys[task])), h("p", { class: "help" }, l(helpKeys[task])), status), action);
    }));
  const refresh = async () => {
    const mine = ++revision;
    await Promise.all(tasks.map(async (task) => {
      const row = rows.get(task)!;
      try {
        const { configuration, speech } = await taskState(ctx.info, task);
        if (disposed || mine !== revision) return;
        const ready = configuration.state === "ready" && (!speech || speech.can_record);
        row.status.textContent = ready ? t("settings.setup.ready") : speech?.reason ? localizeBackendText(speech.reason) : configurationLabel(configuration);
        row.status.dataset.ready = String(ready);
        row.action.textContent = t(ready ? "settings.setup.change" : "settings.setup.finish");
      } catch (error) {
        if (!disposed && mine === revision) row.status.textContent = errorText(error);
      }
    }));
  };
  ctx.onDispose(() => { disposed = true; revision++; });
  ctx.onInfoChange(() => { void refresh(); });
  ctx.onFocusChange((focused) => { if (focused) void refresh(); });
  bindLocalization(root, () => { void refresh(); });
  void refresh();
  return root;
}

export interface TaskSetupDialog {
  notify: (info: AppInfo) => void;
  focusChanged: (focused: boolean) => void;
  visibilityChanged: (visible: boolean) => void;
  flush: () => Promise<void>;
  dispose: () => void;
  focus: () => void;
}

let dialogId = 0;
interface SetupViewScope {
  listeners: ((info: AppInfo) => void)[];
  focus: ((focused: boolean) => void)[];
  visibility: ((visible: boolean) => void)[];
  cleanup: (() => void)[];
  flushes: (() => Promise<void>)[];
}
const viewScope = (): SetupViewScope => ({ listeners: [], focus: [], visibility: [], cleanup: [], flushes: [] });

/** The origin stays mounted behind this task-specific setup, including its draft and reading position. */
export function openTaskSetupDialog(base: Context, task: SetupTask, beforeClose: () => Promise<void>, onClose: () => void): TaskSetupDialog {
  const id = `task-setup-${++dialogId}`;
  const invoker = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const state = new TaskSetupScope();
  const dialog = h("dialog", { class: "task-setup-dialog", "aria-modal": "true", "aria-labelledby": `${id}-title`, "aria-describedby": `${id}-help`, "data-setup-task": task });
  const body = h("div", { class: "task-setup-body" });
  const message = h("p", { class: "task-setup-message help", role: "status", "aria-live": "polite" });
  const status = h("p", { class: "settings-task-status help", role: "status" });
  const back = h("button", { class: "btn", onclick: () => { void close(false); } }, l("common.back"));
  const done = h("button", { class: "btn primary", onclick: () => { void close(true); } }, l("common.done"));
  let closed = false;
  let closing = false;
  let accountPage = false;
  let providerOpen = false;
  let deferred = false;
  let pointerInFlight = false;
  let pointerGeneration = 0;
  let signature = "";
  let statusText = () => "";
  let currentView = viewScope();
  const report = (error: unknown) => { if (!closed) { message.textContent = errorText(error); message.className = "task-setup-message error"; } };
  const saved = () => {
    if (closed) return;
    if (state.failure) report(state.failure);
    else { message.textContent = ""; message.className = "task-setup-message help"; }
    requestRender();
  };
  const typing = () => body.contains(document.activeElement) && !!document.activeElement?.matches("input,textarea,[contenteditable=true]");
  const enteredKey = () => [...body.querySelectorAll<HTMLInputElement>('input[type="password"]')].some((input) => !!input.value);
  const preservingInput = () => typing() || enteredKey() || pointerInFlight;
  const currentSignature = (info: AppInfo) => JSON.stringify([taskFingerprint(info, task), info.interface_language, info.active_model_downloads ?? [], info.kinds.map((kind) => [kind.kind, kind.availability])]);
  const disposeView = (view: SetupViewScope) => { view.cleanup.splice(0).forEach((action) => action()); };

  const child = (view: SetupViewScope): Context => ({
    ...base,
    get info() { return base.info; },
    get visible() { return base.visible; },
    param: undefined, jumpMs: undefined, sourceTarget: undefined,
    save: (change) => state.track(base.save(change), "settings").then(saved).catch((error: unknown) => { report(error); throw error; }),
    mutate: (work) => state.track(base.mutate(work)).then(saved).catch((error: unknown) => { report(error); throw error; }),
    refresh: () => base.refresh().then(() => { requestRender(); }),
    update: (info) => { base.update(info); requestRender(); },
    rerender: () => requestRender(),
    navigate: (tab) => {
      if (tab === "account") { accountPage = true; requestRender(); return; }
      if (["providers", "local", "models"].includes(tab)) {
        providerOpen = true;
        const details = body.querySelector<HTMLDetailsElement>(".task-setup-provider");
        if (details) { details.open = true; details.scrollIntoView({ block: "nearest" }); details.querySelector<HTMLElement>("input,button")?.focus(); }
        else requestRender();
        return;
      }
      report(new Error(t("settings.setup.stay_here")));
    },
    openSetup: () => undefined,
    report,
    onDispose: (action) => view.cleanup.push(action),
    beforeLeave: (action) => view.flushes.push(action),
    onFlush: (action) => view.flushes.push(action),
    onInfoChange: (action) => view.listeners.push(action),
    onFocusChange: (action) => view.focus.push(action),
    onVisibilityChange: (action) => view.visibility.push(action),
  });

  function requestRender() {
    if (closed) return;
    if (preservingInput()) { deferred = true; return; }
    void render().catch(report);
  }

  async function render() {
    const mine = state.next();
    const nextView = viewScope();
    const ctx = child(nextView);
    try {
      let node: HTMLElement;
      if (accountPage) {
        node = h("div", {}, h("button", { class: "text-button", onclick: () => { accountPage = false; requestRender(); } }, l("common.back")), await accountView(ctx));
      } else {
        const info = ctx.info;
        const choice = info.settings[task];
        const account = info.settings.providers.find((provider) => provider.id === choice.provider_id);
        const { configuration, speech } = await taskState(info, task);
        const ready = configuration.state === "ready" && (!speech || speech.can_record);
        if (state.current(mine)) {
          statusText = () => ready ? t("settings.setup.ready") : speech?.reason ? localizeBackendText(speech.reason) : configurationLabel(configuration);
          status.textContent = statusText();
        }
        const details = h("details", { class: "task-setup-provider", open: providerOpen || ["key", "address", "permission"].includes(configuration.state) },
          h("summary", {}, l("settings.setup.connection_details")),
          account ? await providerDetails(ctx, account, task) : h("p", { class: "help" }, l("settings.setup.choose_option")));
        details.addEventListener("toggle", () => { providerOpen = details.open; });
        const addable = info.kinds.filter((kind) => (task === "llm" ? kind.llm : kind.stt)
          && !(["local", "apple_speech", "apple_intelligence"].includes(kind.kind) && info.settings.providers.some((provider) => provider.kind === kind.kind)));
        const add = menuButton(l("settings.setup.add_option"), addable.map((kind) => ({
          label: kind.availability.status === "unsupported" && kind.availability.reason ? `${localizeBackendText(kind.label)} — ${localizeBackendText(kind.availability.reason)}` : localizeBackendText(kind.label),
          disabled: kind.availability.status === "unsupported",
          run: async () => {
            let created: string | undefined;
            try {
              await ctx.mutate(async () => {
                const next = await api.addProvider(kind.kind);
                created = next.settings.providers.at(-1)?.id;
                return next;
              });
              if (created) await ctx.save((settings) => { settings[task] = { provider_id: created!, model: task === "llm" ? kind.default_llm_model : kind.default_stt_model }; });
            } catch (error) { report(error); }
          },
        })), "task-setup-add");
        node = h("div", {}, await taskModelView(ctx, task), details, h("div", { class: "row" }, add));
        if (speech && configuration.state === "ready" && !speech.can_record) {
          node.append(h("button", { class: "btn", onclick: async () => {
            try {
              const permissions = await api.permissions();
              const required = permissions.find((permission: PermissionInfo) => ["microphone", ...(account?.kind === "apple_speech" ? ["speech"] : [])].includes(permission.kind)
                && !["granted", "not_required", "unknown"].includes(permission.status));
              if (required) await api.requestPermission(required.kind);
              await ctx.refresh();
            } catch (error) { report(error); }
          } }, l("settings.setup.check_access")));
        }
        node.append(h("p", { class: "help" }, l("settings.setup.destination", () => ({ destination: providerDestination(account) }))));
      }
      if (!state.current(mine)) { disposeView(nextView); return; }
      if (preservingInput()) { disposeView(nextView); deferred = true; return; }
      disposeView(currentView); currentView = nextView;
      const scroll = body.scrollTop;
      body.replaceChildren(node); refreshLocalization(node); body.scrollTop = scroll;
      signature = currentSignature(base.info);
      deferred = false;
      if (!state.failure) { message.textContent = ""; message.className = "task-setup-message help"; }
    } catch (error) {
      disposeView(nextView);
      if (state.current(mine)) throw error;
    }
  }

  async function flush() {
    await state.settle();
    await Promise.all(currentView.flushes.map((action) => action()));
  }

  async function close(finish: boolean) {
    if (closed || closing) return;
    closing = true; back.disabled = true; done.disabled = true;
    try {
      if (finish) {
        dialog.querySelectorAll<HTMLButtonElement>("button[data-setup-save]:not(:disabled)").forEach((button) => button.click());
      }
      if (document.activeElement instanceof HTMLElement && dialog.contains(document.activeElement)) document.activeElement.blur();
      try { await flush(); } catch (error) { if (finish) throw error; }
      await beforeClose();
      state.settingsSaved();
      if (finish && state.failure) throw state.failure;
      dispose();
      onClose();
    } catch (error) { report(error); }
    finally { closing = false; if (!closed) { back.disabled = false; done.disabled = false; } }
  }

  function dispose() {
    if (closed) return;
    closed = true; state.dispose(); disposeView(currentView);
    dialog.close(); dialog.remove();
    if (invoker?.isConnected) invoker.focus({ preventScroll: true });
  }

  body.addEventListener("focusout", () => { queueMicrotask(() => { if (deferred && !preservingInput()) requestRender(); }); });
  dialog.addEventListener("pointerdown", () => { pointerGeneration++; pointerInFlight = true; }, true);
  dialog.addEventListener("pointerup", () => {
    const gesture = pointerGeneration;
    requestAnimationFrame(() => {
      if (closed || gesture !== pointerGeneration) return;
      pointerInFlight = false;
      if (deferred) requestRender();
    });
  });
  dialog.addEventListener("pointercancel", () => { pointerInFlight = false; if (deferred) requestRender(); });
  dialog.addEventListener("click", () => { pointerInFlight = false; if (deferred) queueMicrotask(requestRender); });
  bindLocalization(status, () => { status.textContent = statusText(); });
  dialog.addEventListener("cancel", (event) => { event.preventDefault(); void close(false); });
  dialog.append(h("header", { class: "task-setup-header" }, h("h2", { id: `${id}-title` }, l(titleKeys[task])), h("p", { id: `${id}-help`, class: "help" }, l(helpKeys[task])), status),
    body, message, h("footer", { class: "task-setup-footer row" }, back, h("span", { class: "grow" }), h("span", { class: "help" }, l("settings.setup.saved_help")), done));
  document.body.append(dialog); dialog.showModal();
  requestRender();
  return {
    notify: (info) => {
      currentView.listeners.forEach((listener) => listener(info));
      if (currentSignature(info) !== signature) requestRender();
    },
    focusChanged: (focused) => { currentView.focus.forEach((listener) => listener(focused)); },
    visibilityChanged: (visible) => { currentView.visibility.forEach((listener) => listener(visible)); },
    flush, dispose, focus: () => { (dialog.querySelector<HTMLElement>("button,input,select") ?? done).focus(); },
  };
}
