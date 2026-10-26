import { liveText, bindLocalization, l, t, uiLocale, localizeBackendText, type LocalizedText } from "../i18n.ts";
import { api, type ModelChoice, type RemoteModel, type Settings } from "../api";
import { errorText, field, h, section, select } from "../dom";
import type { Context } from "../main";
import { processingSummary, providerDestination } from "../processing-state";
import { ModelCatalogCache, providerFingerprint, providerVerification, taskFingerprint, type ProviderSlot } from "../provider-verification";

type Slot = ProviderSlot;

const remoteCache = new ModelCatalogCache<RemoteModel>();

export async function modelsView(ctx: Context): Promise<HTMLElement> {
  return h(
    "div",
    { class: "models-settings" },
    ...await Promise.all([
      slot(ctx, "dictation_stt", l("providers.models.dictation"), l("providers.models.dictation_help"), false),
      slot(ctx, "llm", l("providers.models.text"), l("providers.models.text_help"), true),
      slot(ctx, "meeting_stt", l("providers.models.meetings"), l("providers.models.meetings_help"), false),
    ]),
  );
}

async function slot(ctx: Context, key: Slot, title: string | LocalizedText, subtitle: string | LocalizedText, llm: boolean) {
  const { settings, kinds } = ctx.info;
  const choice: ModelChoice = settings[key];
  const eligible = settings.providers.filter((p) => {
    const kind = kinds.find((k) => k.kind === p.kind);
    return llm ? kind?.llm : kind?.stt;
  });
  const account = settings.providers.find((p) => p.id === choice.provider_id);
  const accountKind = kinds.find((kind) => kind.kind === account?.kind);
  const set = (change: (c: ModelChoice) => void) =>
    ctx.save((s: Settings) => {
      change(s[key]);
    });

  const provider = select(
    eligible.map((p) => [p.id, p.name] as [string, string]),
    choice.provider_id,
    (id) =>
      set((c) => {
        const kind = kinds.find((k) => k.kind === settings.providers.find((p) => p.id === id)?.kind);
        c.provider_id = id;
        c.model = (llm ? kind?.default_llm_model : kind?.default_stt_model) ?? "";
      }),
    "menu",
  );
  const providerLabel = () => provider.setAttribute("aria-label", t("providers.provider_for", { task: String(title).toLocaleLowerCase(uiLocale()) }));
  providerLabel(); bindLocalization(provider, providerLabel);

  for (const option of provider.options) {
    const candidate = eligible.find((item) => item.id === option.value);
    option.disabled = kinds.find((kind) => kind.kind === candidate?.kind)?.availability.status === "unsupported";
  }

  let model: HTMLElement;
  let repair = () => t("providers.choose_provider");
  let repairTarget = "providers";
  const catalogStatus = h("div", { class: "row", hidden: true });
  let ready = accountKind?.availability.status === "available" && (!accountKind.requires_key || !!ctx.info.keys[choice.provider_id]);
  if (account) {
    repair = () => accountKind?.availability.status !== "available" ? localizeBackendText(accountKind?.availability.reason ?? "") || t("providers.unavailable")
      : accountKind.requires_key && !ctx.info.keys[account.id] ? t("providers.add_key", { provider: account.name })
      : t("providers.choose_model");
  }
  if (accountKind?.is_cli_agent) {
    ready = ready && !!account?.base_url;
    if (!account?.base_url) repair = () => t("providers.choose_cli");
  } else ready = ready && !!choice.model.trim();
  if (account?.kind === "local") {
    const downloaded = (await api.localModels(null, false)).filter((m) => m.state.kind === "downloaded");
    const downloadedChoice = downloaded.some((item) => item.id === choice.model);
    ready = ready && downloadedChoice;
    if (!downloadedChoice) { repair = () => t("providers.download_first"); repairTarget = "local"; }
    model = downloaded.length
      ? select(
          downloaded.map((m) => [m.id, m.name] as [string, string]),
          choice.model,
          (id) => set((c) => (c.model = id)),
          "menu",
        )
      : h("span", { class: "muted" }, l("providers.no_downloaded_model"));
  } else if (account?.kind === "apple_speech") {
    model = h("span", { class: "help" }, liveText(() => localizeBackendText(accountKind?.availability.reason ?? "")) || l("providers.apple_speech_model"));
  } else if (account?.kind === "apple_intelligence") {
    model = h("span", { class: "help" }, l("providers.apple_text_model"));
  } else {
    const listId = `models-${key}`;
    const suggestions = h("datalist", { id: listId });
    // The provider's model list is only a suggestion; the field works before it arrives.
    if (account && !accountKind?.is_cli_agent && (!accountKind?.requires_key || ctx.info.keys[account.id])) {
      remoteCache.get(account.id, providerFingerprint(ctx.info, account), () => api.providerModels(account.id))
        .then((known: RemoteModel[]) => suggestions.replaceChildren(...known.map((m) => h("option", { value: m.id }, m.name ?? m.id))))
        .catch((error: unknown) => {
          catalogStatus.replaceChildren(h("p", { class: "help" }, l("providers.catalog_unavailable", () => ({ error: errorText(error) }))),
            h("button", { class: "text-button", onclick: () => { remoteCache.invalidate(account.id); ctx.rerender(); } }, l("providers.reload")));
          catalogStatus.hidden = false;
        });
    }
    const input = h("input", {
      type: "text",
      value: choice.model,
      list: listId,
      placeholder: accountKind?.is_cli_agent ? l("providers.cli_default_model") : l("providers.model_name"),
      onchange: () => set((c) => (c.model = input.value.trim())),
    });
    model = h("div", { class: "row" }, input, suggestions);
  }
  const configured = ready;
  const readiness = h("span", { class: "help", role: "status" });
  const outcome = h("p", { class: "help", role: "status" });
  let testing = false;
  let disposed = false;
  ctx.onDispose(() => { disposed = true; });
  const test = h("button", { class: "btn", disabled: !configured, onclick: async () => {
    if (testing) return;
    const fingerprint = taskFingerprint(ctx.info, key);
    testing = true; draw();
    try {
      const result = await api.testProviderTask(key);
      if (!disposed && taskFingerprint(ctx.info, key) === fingerprint) providerVerification.set(key, fingerprint, result);
    } catch (error) {
      if (!disposed && taskFingerprint(ctx.info, key) === fingerprint) providerVerification.set(key, fingerprint, { verified: false, message: errorText(error) });
    } finally { testing = false; if (!disposed) draw(); }
  } }, llm ? l("providers.test_text") : l("providers.test_speech"));
  const draw = () => {
    const result = providerVerification.get(key, taskFingerprint(ctx.info, key));
    readiness.textContent = !configured ? t("providers.not_configured") : result?.verified ? t("providers.test_passed") : t("providers.configured");
    test.disabled = testing || !configured;
    test.textContent = testing ? t("providers.testing") : llm ? t("providers.test_text") : t("providers.test_speech");
    outcome.textContent = testing ? t("providers.processing_sample") : localizeBackendText(result?.message ?? "");
    outcome.className = result && !result.verified ? "error" : "help";
  };
  draw();
  bindLocalization(readiness, draw);
  const flow = () => key === "llm" ? t("providers.text_destination", { destination: providerDestination(account) })
    : processingSummary(settings, key === "meeting_stt" ? "meeting" : "dictation").map((stage) => `${stage.label} → ${stage.destination}.`).join(" ");
  const repairText = h("span", {}, repair());
  bindLocalization(repairText, () => { repairText.textContent = repair(); });
  const flowText = h("p", { class: "help" }, flow());
  bindLocalization(flowText, () => { flowText.textContent = flow(); });
  return section(title, subtitle, field(l("common.provider"), h("div", { class: "row" }, provider, readiness)), field(l("common.model"), model),
    flowText,
    !configured ? h("p", { class: "help" }, repairText, " ", h("button", { class: "text-button", onclick: () => ctx.navigate(repairTarget) }, repairTarget === "local" ? l("providers.open_local_models") : l("providers.open_providers"))) : null,
    catalogStatus,
    h("p", { class: "help" }, l("providers.sample_help", () => ({ destination: providerDestination(account) }))),
    h("div", { class: "row" }, test), outcome);
}
