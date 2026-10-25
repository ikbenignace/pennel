import { api, type DictationSessionSnapshot, type SetupReadiness } from "../api";
import { SerialQueue } from "../async";
import { errorText, h } from "../dom";
import { OwnedDictationSession } from "../dictation-session";
import { PadDraftState } from "../dictation-session-state";
import { icon } from "../icons";
import type { Context } from "../main";
import { hotkeyLabel } from "../presentation";
import { shortcutAccessHint } from "../pad-shortcut-state";

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
  const back = h("button", { class: "text-button dictation-pad-back", onclick: () => ctx.navigate("dictation-home") }, "Terug naar overal dicteren");
  const status = h("p", { class: "pad-status help", role: "status", "aria-live": "polite" });
  const target = h("p", { class: "help pad-target" });
  const textarea = h("textarea", { class: "dictation-pad", value: draft.value, "aria-label": "Dicteerpad", placeholder: "Spreek of schrijf hier…", spellcheck: true });
  const start = h("button", { class: "btn primary" }, icon("mic"), "Start dicteren");
  const cancel = h("button", { class: "text-button", hidden: true, onclick: () => { void session.cancel().catch(showError); } }, "Annuleer");
  const copy = h("button", { class: "btn", onclick: async () => {
    try { await api.copyText(textarea.value); status.textContent = "Gekopieerd"; }
    catch (error) { showError(error); }
  } }, icon("copy"), "Kopiëren");
  const setup = h("div", { class: "pad-setup", hidden: true }, icon("warning"), h("span", {}, "Dicteren is nog niet klaar."),
    h("button", { class: "btn", onclick: () => api.showWindow("onboarding", "resume") }, "Maak het klaar"));
  const shortcutText = h("span");
  const shortcutRecovery = h("button", { class: "text-button", onclick: () => api.showWindow("onboarding", "permissions") }, "Controleer toegang");
  const shortcutHelp = h("div", { class: "pad-setup pad-shortcut-help", hidden: true, role: "status", "aria-live": "polite" }, icon("warning"), shortcutText, shortcutRecovery);
  const shortcutKey = h("kbd", {}, hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform));
  const shortcutInstruction = h("span", {}, "Hou in om te spreken. Twee keer tikken voor handsfree.");

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
    start.replaceChildren(icon(snapshot.phase === "listening" ? "square" : "mic"), snapshot.phase === "listening" ? "Stop dicteren" : snapshot.phase === "working" ? "Tekst verwerken…" : "Start dicteren");
    cancel.hidden = !busy;
    copy.disabled = busy || !textarea.value;
    if (draft.accept(snapshot)) {
      textarea.value = draft.value;
      copy.disabled = !draft.value;
    }
    status.textContent = writeError ?? snapshot.error ?? (snapshot.phase === "listening" ? "Luistert…" : snapshot.phase === "working" ? "Je woorden worden uitgeschreven…" : "");
    status.classList.toggle("error", !!writeError || !!snapshot.error);
    target.textContent = ctx.info.settings.dictation_pad_auto_paste
      ? snapshot.external_paste_available && snapshot.target_name ? `Plakt ook in ${snapshot.target_name}.` : snapshot.external_paste_reason ?? "Geen doelapp beschikbaar. Je tekst blijft hier."
      : "Je tekst blijft in Pennel. Kopieer wanneer je wilt.";
  };
  const session = new OwnedDictationSession("pad", apply);
  const refreshReadiness = async () => {
    try {
      const result = await api.onboardingReadiness("dictation_pad");
      if (!mounted) return;
      readiness = result;
      start.disabled = transition || !result.can_record;
      setup.hidden = result.can_record;
      const detail = setup.querySelector("span"); if (detail) detail.textContent = result.reason ?? "Dicteren is nog niet klaar.";
      const shortcut = shortcutAccessHint(result, ctx.info.capabilities.global_hotkey);
      shortcutHelp.hidden = shortcut === null;
      shortcutText.textContent = shortcut?.message ?? "";
      shortcutRecovery.hidden = !shortcut?.recoverable;
      shortcutKey.textContent = hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform);
      shortcutInstruction.textContent = result.can_use_shortcut ? "Hou in om te spreken. Twee keer tikken voor handsfree." : "Je kunt de knop gebruiken om hier te dicteren.";
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
  ctx.onInfoChange(() => { void refreshReadiness(); if (session.state.snapshot) apply(session.state.snapshot); });
  ctx.onFocusChange((focused) => { if (focused) void session.refresh().catch(async () => { await session.close(); await session.open(); }).catch(showError); });
  const root = h("div", { class: "pad-workspace" }, back, setup,
    h("div", { class: "page-heading" }, h("div", {}, h("h1", {}, "Dicteerpad"), h("p", { class: "muted" }, "Spreek, lees na en werk je tekst bij.")), start),
    h("div", { class: "shortcut-instruction" }, shortcutKey, shortcutInstruction), shortcutHelp,
    textarea, h("div", { class: "row pad-actions" }, copy, cancel, h("span", { class: "grow" }),
      h("button", { class: "text-button", onclick: () => ctx.navigate("dictation") }, icon("settings"), "Dicteren instellen")), status, target);
  try { await session.open(); } catch (error) { showError(error); }
  await refreshReadiness();
  return root;
}

function dictationHome(ctx: Context): HTMLElement {
  const key = h("kbd", {}, hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform));
  const detail = h("p", { class: "help", hidden: true });
  const repair = h("button", { class: "btn dictation-home-repair", hidden: true, onclick: () => api.showWindow("onboarding", "resume") }, "Maak het klaar");
  const root = h("div", { class: "pad-workspace dictation-home" },
    h("h1", {}, "Dicteren waar je typt."),
    h("p", { class: "dictation-home-intro muted" }, "Klik in een tekstveld in een andere app. Houd je sneltoets ingedrukt en spreek. Laat los om je tekst daar te plakken."),
    h("div", { class: "dictation-home-shortcut" }, key, h("span", { class: "muted" }, "Houd in om te spreken. Laat los om te plakken.")),
    detail, repair,
    h("button", { class: "btn dictation-home-settings", onclick: () => ctx.navigate("dictation") }, "Dicteren instellen"),
    h("div", { class: "dictation-pad-option" }, h("button", { class: "text-button", onclick: () => ctx.navigate("dictation-pad") }, "Open dicteerpad"),
      h("p", { class: "help" }, "Alleen als je tekst eerst in Pennel wilt bewaren of bewerken.")));
  let disposed = false;
  let revision = 0;
  ctx.onDispose(() => { disposed = true; });
  const refresh = async () => {
    const mine = ++revision;
    key.textContent = hotkeyLabel(ctx.info.settings.hotkey, ctx.info.platform);
    try {
      const ready = await api.onboardingReadiness("dictation_pad");
      if (disposed || mine !== revision) return;
      const shortcut = shortcutAccessHint(ready, ctx.info.capabilities.global_hotkey);
      detail.textContent = ready.can_record ? shortcut?.message ?? "" : ready.reason ?? "Stel eerst spraakherkenning in.";
      detail.hidden = !detail.textContent;
      repair.hidden = ready.can_record && !shortcut?.recoverable;
      repair.textContent = ready.can_record ? "Controleer toegang" : "Maak het klaar";
    } catch (error) { if (!disposed && mine === revision) { detail.textContent = errorText(error); detail.hidden = false; } }
  };
  ctx.onInfoChange(() => { void refresh(); });
  ctx.onFocusChange((focused) => { if (focused) void refresh(); });
  void refresh();
  return root;
}
