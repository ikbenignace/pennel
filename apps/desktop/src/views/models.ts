import { api, type ModelChoice, type RemoteModel, type Settings } from "../api";
import { field, h, section, select } from "../dom";
import type { Context } from "../main";

type Slot = "dictation_stt" | "llm" | "meeting_stt";

const remoteCache = new Map<string, RemoteModel[]>();

async function remoteModels(providerId: string): Promise<RemoteModel[]> {
  if (!remoteCache.has(providerId)) {
    try {
      remoteCache.set(providerId, await api.providerModels(providerId));
    } catch {
      remoteCache.set(providerId, []);
    }
  }
  return remoteCache.get(providerId)!;
}

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
  );

  for (const option of provider.options) {
    const candidate = eligible.find((item) => item.id === option.value);
    option.disabled = kinds.find((kind) => kind.kind === candidate?.kind)?.availability.status === "unsupported";
  }

  let model: HTMLElement;
  let ready = accountKind?.availability.status === "available" && (!accountKind.requires_key || !!ctx.info.keys[choice.provider_id]);
  if (accountKind?.is_cli_agent) ready = ready && !!account?.base_url;
  if (account?.kind === "local") {
    const downloaded = (await api.localModels(null, false)).filter((m) => m.state.kind === "downloaded");
    ready = downloaded.some((item) => item.id === choice.model);
    model = downloaded.length
      ? select(
          downloaded.map((m) => [m.id, m.name] as [string, string]),
          choice.model,
          (id) => set((c) => (c.model = id)),
        )
      : h("span", { class: "muted" }, "Nog geen model gedownload — zie Lokale modellen.");
  } else if (account?.kind === "apple_speech") {
    model = h("span", { class: "help" }, accountKind?.availability.reason ?? "macOS beheert het dicteermodel en de taalbestanden.");
  } else if (account?.kind === "apple_intelligence") {
    model = h("span", { class: "help" }, "macOS beheert het ingebouwde taalmodel van Apple Intelligence.");
  } else {
    const listId = `models-${key}`;
    const known = account && !accountKind?.is_cli_agent && (!accountKind?.requires_key || ctx.info.keys[account.id])
      ? await remoteModels(account.id) : [];
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
  return section(title, subtitle, field("Provider", h("div", { class: "row" }, provider,
    h("span", { class: `status-dot${ready ? " ready" : ""}`, "aria-label": ready ? "In orde" : "Nog instellen" }))), field("Model", model),
    accountKind?.availability.status !== "available" && accountKind?.availability.reason
      ? h("p", { class: "help" }, accountKind.availability.reason) : null);
}
