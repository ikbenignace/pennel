import { listen } from "@tauri-apps/api/event";
import { api, type PermissionInfo, type PermissionKind, type RecordPrompt } from "../api";
import { errorText, h } from "../dom";
import type { Context } from "../main";
import wordmark from "../../../../assets/brand/wordmark.svg";

function granted(permissions: PermissionInfo[], kind: PermissionKind): boolean {
  const status = permissions.find((item) => item.kind === kind)?.status;
  return status === "granted" || status === "not_required";
}

export async function onboardingView(ctx: Context): Promise<HTMLElement> {
  const permissions = await api.permissions();
  const { settings, kinds, keys, platform } = ctx.info;
  const account = settings.providers.find((item) => item.id === settings.dictation_stt.provider_id);
  const kind = kinds.find((item) => item.kind === account?.kind);
  let providerReady = !!account && !!settings.dictation_stt.model && !!kind
    && kind.availability.status === "available" && (!kind.requires_key || !!keys[account.id]);
  if (account?.kind === "local") {
    providerReady = (await api.localModels(null, false)).some((model) => model.id === settings.dictation_stt.model && model.state.kind === "downloaded");
  }
  const step = (title: string, description: string, ready: boolean, action?: () => Promise<unknown>, actionTitle = "Geef toegang") => {
    const status = h("span", { class: "help", role: "status" }, ready ? "In orde" : "");
    const button = h("button", { class: "btn", hidden: !action || ready }, actionTitle);
    button.addEventListener("click", async () => {
      button.disabled = true;
      try { await action?.(); await ctx.refresh(); }
      catch (error) { status.textContent = errorText(error); button.disabled = false; }
    });
    return h("section", { class: "onboarding-step row" },
      h("span", { class: `status-dot${ready ? " ready" : ""}`, "aria-label": ready ? "In orde" : "Nog instellen" }),
      h("div", { class: "grow" }, h("strong", {}, title), h("p", { class: "help" }, description)), status, button);
  };
  const timer = window.setInterval(() => { void ctx.refresh().catch(ctx.report); }, 3000);
  ctx.onDispose(() => window.clearInterval(timer));
  return h("div", { class: "onboarding" },
    h("img", { class: "wordmark-image", src: wordmark, alt: "Pennel" }),
    h("h1", {}, "Welkom bij Pennel"),
    h("p", { class: "muted" }, "Drie dingen en je kan dicteren in elke app."),
    step("Microfoon", "Zodat Pennel kan horen wat je dicteert.", granted(permissions, "microphone"),
      platform === "linux" ? undefined : () => api.requestPermission("microphone"), platform === "windows" ? "Open instellingen" : "Geef toegang"),
    step("Toegankelijkheid", "Nodig voor de sneltoets en om tekst in andere apps te plakken.", granted(permissions, "accessibility"),
      platform === "macos" ? () => api.requestPermission("accessibility") : undefined, "Open instellingen"),
    ctx.info.capabilities.global_hotkey.reason ? h("p", { class: "help" }, ctx.info.capabilities.global_hotkey.reason) : null,
    step("Provider", platform === "macos"
      ? "Kies Apple-dicteren bij Instellingen → Providers, voeg een eigen provider toe of download daar een lokaal model."
      : "Plak een API-sleutel bij Instellingen → Providers, of download daar een lokaal model.", providerReady,
      () => api.showWindow("settings", "providers"), "Kies provider"),
    platform === "macos" ? h("p", { class: "help" }, "Tip: zet in Systeeminstellingen → Toetsenbord de 🌐/fn-toets op ‘Doe niets’, anders opent macOS de emoji-kiezer.") : null,
    ctx.info.preview ? h("p", { class: "help preview-note" }, "Je gebruikt de preview. Instellingen en meetings worden in een aparte map bewaard.") : null,
    h("div", { class: "row" },
      h("button", { class: "btn", onclick: () => api.showWindow("settings", "dictation") }, "Dicteren instellen"),
      h("span", { class: "grow" }), h("button", { class: "btn primary", onclick: () => api.showWindow("library") }, "Open Pennel")));
}

export async function promptView(ctx: Context): Promise<HTMLElement> {
  const node = h("div", { class: "record-prompt" });
  let current: RecordPrompt | null = null;
  let pending = false;
  const draw = (prompt: RecordPrompt | null) => {
    current = prompt;
    pending = false;
    if (!prompt || prompt.expires_at_ms <= Date.now()) { node.replaceChildren(); return; }
    const status = h("span", { class: "help", role: "status" });
    const respond = async (answer: "yes" | "no" | "never") => {
      if (!current || pending || current.id !== prompt.id) return;
      const id = prompt.id;
      pending = true;
      node.querySelectorAll("button").forEach((button) => { button.disabled = true; });
      try { await api.answerRecordPrompt(answer, id); }
      catch (error) { status.textContent = errorText(error); }
      if (current?.id === id) node.replaceChildren();
    };
    node.replaceChildren(
      h("div", { class: "row" }, h("span", { class: "status-dot" }), h("strong", {}, "Opnemen?")),
      h("p", { class: "help prompt-description" }, [prompt.app_name, prompt.title].filter(Boolean).join(" · ")),
      h("div", { class: "row" }, h("button", { class: "btn", onclick: () => respond("yes") }, "Ja"),
        h("button", { class: "btn", onclick: () => respond("no") }, "Nee"), h("span", { class: "grow" }),
        h("button", { class: "text-button", onclick: () => respond("never") }, `Nooit voor ${prompt.app_name}`)), status);
  };
  let eventReceived = false;
  const dispose = await listen<RecordPrompt | null>("record-prompt", (event) => { eventReceived = true; draw(event.payload); });
  ctx.onDispose(dispose);
  const snapshot = await api.recordPrompt();
  if (!eventReceived) draw(snapshot);
  const timer = window.setInterval(() => { if (current && current.expires_at_ms <= Date.now()) draw(null); }, 1000);
  ctx.onDispose(() => window.clearInterval(timer));
  return node;
}
