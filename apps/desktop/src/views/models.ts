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
    await slot(ctx, "dictation_stt", "Dicteren", "Spraak naar tekst wanneer je de sneltoets gebruikt.", false),
    await slot(ctx, "llm", "Opkuisen en samenvatten", "Taalmodel dat je dictaten opkuist en meetings samenvat.", true),
    await slot(ctx, "meeting_stt", "Meetings", "Spraak naar tekst voor opgenomen meetings.", false),
  );
}

async function slot(ctx: Context, key: Slot, title: string, subtitle: string, llm: boolean) {
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
  provider.setAttribute("aria-label", `Provider voor ${title.toLocaleLowerCase("nl-BE")}`);

  for (const option of provider.options) {
    const candidate = eligible.find((item) => item.id === option.value);
    option.disabled = kinds.find((kind) => kind.kind === candidate?.kind)?.availability.status === "unsupported";
  }

  let model: HTMLElement;
  let repair = "Kies een provider.";
  let repairTarget = "providers";
  let catalogError: string | null = null;
  let ready = accountKind?.availability.status === "available" && (!accountKind.requires_key || !!ctx.info.keys[choice.provider_id]);
  if (account) {
    repair = accountKind?.availability.status !== "available" ? accountKind?.availability.reason ?? "Deze provider is niet beschikbaar."
      : accountKind.requires_key && !ctx.info.keys[account.id] ? `Voeg de API-sleutel toe bij Providers → ${account.name}.`
      : "Kies een model.";
  }
  if (accountKind?.is_cli_agent) {
    ready = ready && !!account?.base_url;
    if (!account?.base_url) repair = "Kies het CLI-programma bij Providers.";
  } else ready = ready && !!choice.model.trim();
  if (account?.kind === "local") {
    const downloaded = (await api.localModels(null, false)).filter((m) => m.state.kind === "downloaded");
    const downloadedChoice = downloaded.some((item) => item.id === choice.model);
    ready = ready && downloadedChoice;
    if (!downloadedChoice) { repair = "Download eerst het gekozen spraakmodel."; repairTarget = "local"; }
    model = downloaded.length
      ? select(
          downloaded.map((m) => [m.id, m.name] as [string, string]),
          choice.model,
          (id) => set((c) => (c.model = id)),
          "menu",
        )
      : h("span", { class: "muted" }, "Nog geen model gedownload — zie Lokale modellen.");
  } else if (account?.kind === "apple_speech") {
    model = h("span", { class: "help" }, accountKind?.availability.reason ?? "macOS beheert het dicteermodel en de taalbestanden.");
  } else if (account?.kind === "apple_intelligence") {
    model = h("span", { class: "help" }, "macOS beheert het ingebouwde taalmodel van Apple Intelligence.");
  } else {
    const listId = `models-${key}`;
    let known: RemoteModel[] = [];
    if (account && !accountKind?.is_cli_agent && (!accountKind?.requires_key || ctx.info.keys[account.id])) {
      try { known = await remoteCache.get(account.id, providerFingerprint(ctx.info, account), () => api.providerModels(account.id)); }
      catch (error) { catalogError = `Modellenlijst niet beschikbaar: ${errorText(error)}`; }
    }
    const input = h("input", {
      type: "text",
      value: choice.model,
      list: listId,
      placeholder: accountKind?.is_cli_agent ? "Leeg = het model van de CLI" : "modelnaam",
      onchange: () => set((c) => (c.model = input.value.trim())),
    });
    model = h(
      "div",
      { class: "row" },
      input,
      h(
        "datalist",
        { id: listId },
        known.map((m) => h("option", { value: m.id }, m.name ?? m.id)),
      ),
    );
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
  } }, llm ? "Test tekstverwerking" : "Test spraakherkenning");
  const draw = () => {
    const result = providerVerification.get(key, taskFingerprint(ctx.info, key));
    readiness.textContent = !configured ? "Nog instellen" : result?.verified ? "Test geslaagd" : "Ingesteld";
    test.disabled = testing || !configured;
    test.textContent = testing ? "Testen…" : llm ? "Test tekstverwerking" : "Test spraakherkenning";
    outcome.textContent = testing ? "Voorbeeld verwerken…" : result?.message ?? "";
    outcome.className = result && !result.verified ? "error" : "help";
  };
  draw();
  const flow = key === "llm" ? `Tekst → ${providerDestination(account)}.`
    : processingSummary(settings, key === "meeting_stt" ? "meeting" : "dictation").map((stage) => `${stage.label} → ${stage.destination}.`).join(" ");
  return section(title, subtitle, field("Provider", h("div", { class: "row" }, provider, readiness)), field("Model", model),
    h("p", { class: "help" }, flow),
    !configured ? h("p", { class: "help" }, repair, " ", h("button", { class: "text-button", onclick: () => ctx.navigate(repairTarget) }, repairTarget === "local" ? "Lokale modellen openen" : "Providers openen")) : null,
    catalogError ? h("div", { class: "row" }, h("p", { class: "help" }, catalogError), h("button", { class: "text-button", onclick: () => { if (account) remoteCache.invalidate(account.id); ctx.rerender(); } }, "Opnieuw laden")) : null,
    h("p", { class: "help" }, `De test verwerkt alleen een voorbeeld met ${providerDestination(account)}. Er wordt geen eigen dictaat of meeting gebruikt.`),
    h("div", { class: "row" }, test), outcome);
}
