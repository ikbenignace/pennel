import { liveText, bindLocalization, l, t, uiLocale, localizeBackendText, type LocalizedText, type UiText } from "../i18n.ts";
import { localModelDisplayName } from "../settings-presentation.ts";
import { api, listen, type ModelChoice, type RemoteModel, type Settings } from "../api";
import { controlButton, textField, errorText, field, formatBytes, h, isCancellation, section, select } from "../dom";
import type { Context } from "../main";
import { processingSummary, providerDestination } from "../processing-state";
import { ModelCatalogCache, providerFingerprint, providerVerification, taskFingerprint, type ProviderSlot } from "../provider-verification";
import { inferenceModelsView } from "./inference";

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
    ...await inferenceModelsView(ctx),
  );
}

export async function taskModelView(ctx: Context, key: Slot): Promise<HTMLElement> {
  const title = key === "llm" ? l("settings.setup.text_title") : key === "meeting_stt" ? l("settings.setup.meeting_title") : l("settings.setup.dictation_title");
  const result = await slot(ctx, key, title, null, key === "llm", true);
  result.querySelector("h2,h3")?.classList.add("visually-hidden");
  return result;
}

async function slot(ctx: Context, key: Slot, title: string | LocalizedText, subtitle: string | LocalizedText | null, llm: boolean, taskSetup = false) {
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
    }).catch(ctx.report);

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
    const all = await api.localModels(null, false);
    const downloaded = all.filter((m) => m.state.kind === "downloaded");
    const downloadedChoice = downloaded.some((item) => item.id === choice.model);
    ready = ready && downloadedChoice;
    if (!downloadedChoice) { repair = () => t("providers.download_first"); repairTarget = "local"; }
    model = taskSetup ? select(
      all.map((item) => [item.id, liveText(() => `${localModelDisplayName(item)} · ${formatBytes(item.size_bytes)}`)] as [string, UiText]),
      choice.model, (id) => set((c) => (c.model = id)), "menu",
    ) : downloaded.length
      ? select(
          downloaded.map((m) => [m.id, localModelDisplayName(m)] as [string, UiText]),
          choice.model,
          (id) => set((c) => (c.model = id)),
          "menu",
        )
      : h("span", { class: "muted" }, l("providers.no_downloaded_model"));
    if (taskSetup && !downloadedChoice) {
      const selectedModel = all.find((item) => item.id === choice.model);
      if (selectedModel) {
        let disposed = false;
        let active = ctx.info.active_model_downloads?.includes(selectedModel.id) ?? false;
        const progress = h("progress", { hidden: !active, "aria-label": l("providers.local.downloading_model", () => ({ model: String(localModelDisplayName(selectedModel)) })) });
        const outcome = h("p", { class: "help", role: "status" });
        const download = controlButton({ class: "btn", disabled: active || accountKind?.availability.status !== "available", onclick: async () => {
          active = true; download.disabled = true; cancel.hidden = false; progress.hidden = false;
          try { await api.downloadModel(selectedModel.id); if (!disposed) await ctx.refresh(); }
          catch (error) { if (!disposed && !isCancellation(error)) { outcome.textContent = errorText(error); outcome.className = "error"; } }
          finally { if (!disposed) { active = false; download.disabled = false; cancel.hidden = true; progress.hidden = true; } }
        } }, l("settings.setup.download", () => ({ size: formatBytes(selectedModel.size_bytes) })));
        const cancel = controlButton({ class: "btn", hidden: !active, onclick: () => api.cancelDownload(selectedModel.id).catch(ctx.report) }, l("common.cancel"));
        ctx.onDispose(() => { disposed = true; });
        void listen<{ id: string; downloaded: number; total: number }>("download-progress", (event) => {
          if (!disposed && event.payload.id === selectedModel.id) { progress.max = event.payload.total || 1; progress.value = event.payload.downloaded; }
        }).then((unlisten) => { if (disposed) unlisten(); else ctx.onDispose(unlisten); }).catch(ctx.report);
        void listen<{ id: string }>("download-done", (event) => {
          if (!disposed && event.payload.id === selectedModel.id) void ctx.refresh().catch(ctx.report);
        }).then((unlisten) => { if (disposed) unlisten(); else ctx.onDispose(unlisten); }).catch(ctx.report);
        model = h("div", {}, model, h("div", { class: "row" }, download, cancel), progress, outcome);
      }
    }
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
    const input = textField({ type: "text",
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
  const test = controlButton({ class: "btn", disabled: !configured, onclick: async () => {
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
  return section(title, subtitle, field(taskSetup ? l("settings.setup.process_with") : l("common.provider"), h("div", { class: "row" }, provider, readiness)), field(l("common.model"), model),
    flowText,
    !configured ? h("p", { class: "help" }, repairText, " ",
      taskSetup && repairTarget === "local" ? null : h("button", { class: "text-button", onclick: () => ctx.navigate(repairTarget) }, taskSetup ? l("settings.setup.connection_details") : repairTarget === "local" ? l("providers.open_local_models") : l("providers.open_providers"))) : null,
    catalogStatus,
    h("p", { class: "help" }, l("providers.sample_help", () => ({ destination: providerDestination(account) }))),
    h("div", { class: "row" }, test), outcome);
}
