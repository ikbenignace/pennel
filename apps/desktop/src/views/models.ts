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
    {},
    h("h1", {}, "Modellen"),
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

  let model: HTMLElement;
  if (account?.kind === "local") {
    const downloaded = (await api.localModels(null, false)).filter((m) => m.state.kind === "downloaded");
    model = downloaded.length
      ? select(
          downloaded.map((m) => [m.id, m.name] as [string, string]),
          choice.model,
          (id) => set((c) => (c.model = id)),
        )
      : h("span", { class: "muted" }, "Nog geen model gedownload — zie Lokale modellen.");
  } else {
    const listId = `models-${key}`;
    const known = account ? await remoteModels(account.id) : [];
    const input = h("input", {
      type: "text",
      value: choice.model,
      list: listId,
      placeholder: "modelnaam",
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
  return section(title, subtitle, field("Provider", provider), field("Model", model));
}
