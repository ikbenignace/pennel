import { bindLocalization, l, t, localizeBackendText } from "../i18n.ts";
import { api, type DictationSessionSnapshot, type SetupReadiness } from "../api";
import { SerialQueue } from "../async";
import { errorText, h } from "../dom";
import { OwnedDictationSession } from "../dictation-session";
import { PadDraftState } from "../dictation-session-state";
import { icon } from "../icons";
import type { Context } from "../main";
import { hotkeyLabel } from "../presentation";
import { shortcutAccessHint } from "../pad-shortcut-state";
import { processingSummary } from "../processing-state";
import { processingLabel } from "../recording-processing";

const draft = new PadDraftState();
const saves = new SerialQueue();

export async function padView(ctx: Context): Promise<HTMLElement> {
  if (ctx.param !== "__pad__") return dictationHome(ctx);
  if (!draft.dirty) draft.hydrate(await api.padText());
  let mounted = true;
  let saving: number | undefined;
  let readiness: SetupReadiness | null = null;
  let transition = false;
  let writeError: string | null = null;
  const back = h("button", { class: "text-button dictation-pad-back", onclick: () => ctx.navigate("dictation-home") }, l("dictation.pad.back"));
  const status = h("p", { class: "pad-status help", role: "status", "aria-live": "polite" });
  const target = h("p", { class: "help pad-target" });
  const textarea = h("textarea", { class: "dictation-pad", value: draft.value, lang: ctx.info.settings.language ?? "", "aria-label": l("dictation.pad.title"), placeholder: l("dictation.pad.placeholder"), spellcheck: true });
  const start = h("button", { class: "btn primary" }, icon("mic"), l("dictation.pad.start"));
  const cancel = h("button", { class: "text-button", hidden: true, onclick: () => { void session.cancel().catch(showError); } }, l("common.cancel"));
  const copy = h("button", { class: "btn", onclick: async () => {
    try { await api.copyText(textarea.value); status.textContent = t("dictation.pad.copied"); }
    catch (error) { showError(error); }
  } }, icon("copy"), l("common.copy"));
  const setup = h("div", { class: "pad-setup", hidden: true }, icon("warning"), h("span", {}, l("dictation.pad.not_ready")),
    h("button", { class: "btn", onclick: () => api.showWindow("onboarding", "resume") }, l("dictation.pad.setup")));
  const shortcutText = h("span");
  const shortcutRecovery = h("button", { class: "text-button", onclick: () => api.showWindow("onboarding", "permissions") }, l("dictation.access.check"));
  const shortcutHelp = h("div", { class: "pad-setup pad-shortcut-help", hidden: true, role: "status", "aria-live": "polite" }, icon("warning"), shortcutText, shortcutRecovery);
  const shortcutKey = h("kbd", {}, hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform));
  const shortcutInstruction = h("span", {}, l("dictation.pad.shortcut_help"));
  const processing = h("p", { class: "help processing-path" });
  const refreshProcessing = () => { processing.textContent = t("dictation.pad.next_processing", { path: processingLabel(processingSummary(ctx.info.settings, "dictation")) }); };
  refreshProcessing();

  const flush = async () => {
    window.clearTimeout(saving);
    await saves.drain();
    if (!draft.dirty) return;
    const value = draft.value;
    try { await saves.run(() => api.savePadText(value)); }
    catch (error) { writeError = errorText(error); throw error; }
    writeError = null;
    session.acceptLocalSavedText(value);
    draft.acknowledge(value);
  };
  const showError = (error: unknown) => { if (mounted) { status.textContent = errorText(error); status.classList.add("error"); start.disabled = false; } };
  const apply = (snapshot: DictationSessionSnapshot) => {
    if (!mounted) return;
    const busy = snapshot.phase === "listening" || snapshot.phase === "working";
    back.disabled = busy;
    textarea.disabled = busy;
    start.disabled = transition || snapshot.phase === "working" || readiness?.can_record === false;
    start.replaceChildren(icon(snapshot.phase === "listening" ? "square" : "mic"), snapshot.phase === "listening" ? t("dictation.pad.stop") : snapshot.phase === "working" ? t("dictation.pad.processing") : t("dictation.pad.start"));
    cancel.hidden = !busy;
    copy.disabled = busy || !textarea.value;
    if (draft.accept(snapshot)) {
      textarea.value = draft.value;
      copy.disabled = !draft.value;
    }
    status.textContent = writeError ?? (snapshot.error ? localizeBackendText(snapshot.error) : null) ?? (snapshot.phase === "listening" ? t("dictation.pad.listening") : snapshot.phase === "working" ? t("dictation.pad.transcribing") : "");
    status.classList.toggle("error", !!writeError || !!snapshot.error);
    target.textContent = ctx.info.settings.dictation_pad_auto_paste
      ? snapshot.external_paste_available && snapshot.target_name ? t("dictation.pad.external_target", { app: snapshot.target_name }) : snapshot.external_paste_reason ? localizeBackendText(snapshot.external_paste_reason) : t("dictation.pad.no_target")
      : t("dictation.pad.local_target");
  };
  const session = new OwnedDictationSession("pad", apply);
  const refreshReadiness = async () => {
    try {
      const result = await api.onboardingReadiness("dictation_pad");
      if (!mounted) return;
      readiness = result;
      start.disabled = transition || !result.can_record;
      setup.hidden = result.can_record;
      const detail = setup.querySelector("span"); if (detail) detail.textContent = result.reason ? localizeBackendText(result.reason) : t("dictation.pad.not_ready");
      const shortcut = shortcutAccessHint(result, ctx.info.capabilities.global_hotkey);
      shortcutHelp.hidden = shortcut === null;
      shortcutText.textContent = shortcut?.message ?? "";
      shortcutRecovery.hidden = !shortcut?.recoverable;
      shortcutKey.textContent = hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform);
      shortcutInstruction.textContent = result.can_use_shortcut ? t("dictation.pad.shortcut_help") : t("dictation.pad.button_help");
      if (session.state.snapshot) apply(session.state.snapshot);
    } catch (error) { showError(error); }
  };
  start.addEventListener("click", async () => {
    transition = true; start.disabled = true;
    try { await flush(); await session.toggle(); }
    catch (error) { showError(error); }
    finally { transition = false; if (session.state.snapshot) apply(session.state.snapshot); }
  });
  textarea.addEventListener("input", () => {
    session.invalidateRefresh(); draft.edit(textarea.value); copy.disabled = !textarea.value;
    window.clearTimeout(saving); saving = window.setTimeout(() => { void flush().catch(showError); }, 160);
  });
  textarea.addEventListener("blur", () => { void flush().catch(showError); });
  ctx.onFlush(flush);
  ctx.beforeLeave(async () => { await flush(); await session.close(); });
  ctx.onDispose(() => { mounted = false; window.clearTimeout(saving); void flush().catch(ctx.report); void session.close().catch(ctx.report); });
  ctx.onInfoChange(() => { textarea.lang = ctx.info.settings.language ?? ""; refreshProcessing(); void refreshReadiness(); if (session.state.snapshot) apply(session.state.snapshot); });
  const restoreSession = async () => {
    if (!mounted || !ctx.visible) return;
    try { await session.open(); await session.refresh(); }
    catch {
      await session.close();
      if (mounted && ctx.visible) await session.open();
    }
    if (mounted && ctx.visible) await refreshReadiness();
  };
  ctx.onFocusChange((focused) => { if (focused) void restoreSession().catch(showError); });
  ctx.onVisibilityChange((visible) => { if (visible) void restoreSession().catch(showError); });
  const root = h("div", { class: "pad-workspace" }, back, setup,
    h("div", { class: "page-heading" }, h("div", {}, h("h1", {}, l("dictation.pad.title")), h("p", { class: "muted" }, l("dictation.pad.intro"))), start),
    h("div", { class: "shortcut-instruction" }, shortcutKey, shortcutInstruction), shortcutHelp, processing,
    textarea, h("div", { class: "row pad-actions" }, copy, cancel, h("span", { class: "grow" }),
      h("button", { class: "text-button", onclick: () => ctx.navigate("dictation") }, icon("settings"), l("dictation.pad.settings"))), status, target);
  bindLocalization(root, () => {
    refreshProcessing();
    void refreshReadiness();
    if (session.state.snapshot) apply(session.state.snapshot);
  });
  try { if (ctx.visible) await session.open(); } catch (error) { showError(error); }
  await refreshReadiness();
  return root;
}

function dictationHome(ctx: Context): HTMLElement {
  const key = h("kbd", {}, hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform));
  const detail = h("p", { class: "help", hidden: true });
  const repair = h("button", { class: "btn dictation-home-repair", hidden: true, onclick: () => api.showWindow("onboarding", "resume") }, l("dictation.pad.setup"));
  const processing = h("p", { class: "help processing-path" });
  const root = h("div", { class: "pad-workspace dictation-home" },
    h("h1", {}, l("dictation.home.title")),
    h("p", { class: "dictation-home-intro muted" }, l("dictation.home.intro")),
    h("div", { class: "dictation-home-shortcut" }, key, h("span", { class: "muted" }, l("dictation.home.shortcut"))),
    detail, repair, processing,
    h("button", { class: "btn dictation-home-settings", onclick: () => ctx.navigate("dictation") }, l("dictation.pad.settings")),
    h("div", { class: "dictation-pad-option" }, h("button", { class: "text-button", onclick: () => ctx.navigate("dictation-pad") }, l("dictation.home.open_pad")),
      h("p", { class: "help" }, l("dictation.home.pad_help"))));
  let disposed = false;
  let revision = 0;
  ctx.onDispose(() => { disposed = true; });
  const refresh = async () => {
    const mine = ++revision;
    key.textContent = hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform);
    processing.textContent = t("dictation.pad.next_processing", { path: processingLabel(processingSummary(ctx.info.settings, "dictation")) });
    try {
      const ready = await api.onboardingReadiness("dictation_pad");
      if (disposed || mine !== revision) return;
      const shortcut = shortcutAccessHint(ready, ctx.info.capabilities.global_hotkey);
      detail.textContent = ready.can_record ? shortcut?.message ?? "" : ready.reason ? localizeBackendText(ready.reason) : t("dictation.home.speech_setup");
      detail.hidden = !detail.textContent;
      repair.hidden = ready.can_record && !shortcut?.recoverable;
      repair.textContent = ready.can_record ? t("dictation.access.check") : t("dictation.pad.setup");
    } catch (error) { if (!disposed && mine === revision) { detail.textContent = errorText(error); detail.hidden = false; } }
  };
  ctx.onInfoChange(() => { void refresh(); });
  bindLocalization(root, () => { void refresh(); });
  ctx.onFocusChange((focused) => { if (focused) void refresh(); });
  void refresh();
  return root;
}
