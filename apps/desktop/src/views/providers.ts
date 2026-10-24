import { api, type ProviderAccount } from "../api";
import { errorText, field, h, section, select } from "../dom";
import type { Context } from "../main";

let selected: string | null = null;

export function providersView(ctx: Context): HTMLElement {
  const { settings, kinds, keys } = ctx.info;
  if (!settings.providers.some((p) => p.id === selected)) {
    selected = settings.providers[0]?.id ?? null;
  }
  const account = settings.providers.find((p) => p.id === selected);
  const kindLabel = (kind: string) => kinds.find((k) => k.kind === kind)?.label ?? kind;

  const list = h(
    "div",
    { class: "list" },
    settings.providers.map((p) => {
      const kind = kinds.find((k) => k.kind === p.kind);
      const missingKey = kind?.requires_key && !keys[p.id];
      return h(
        "button",
        {
          "aria-current": p.id === selected ? "true" : null,
          onclick: () => {
            selected = p.id;
            ctx.rerender();
          },
        },
        h("span", {}, p.name),
        missingKey ? h("span", { class: "badge accent" }, "sleutel") : null,
      );
    }),
    h(
      "div",
      { style: "margin-top:10px" },
      select(
        [["", "Provider toevoegen…"], ...kinds.filter((k) => k.kind !== "local").map((k) => [k.kind, k.label] as [string, string])],
        "",
        async (kind) => {
          if (!kind) return;
          const next = await api.addProvider(kind as ProviderAccount["kind"]);
          selected = next.settings.providers[next.settings.providers.length - 1].id;
          ctx.update(next);
        },
      ),
    ),
  );

  return h(
    "div",
    {},
    h("h1", {}, "Providers"),
    ctx.info.hotkey_error
      ? section(
          "Sneltoets",
          null,
          h("p", { class: "error" }, `De sneltoets werkt niet: ${ctx.info.hotkey_error}`),
        )
      : null,
    h(
      "div",
      { class: "providers" },
      list,
      account ? detail(ctx, account, kindLabel(account.kind)) : h("p", { class: "muted" }, "Kies een provider"),
    ),
  );
}

function detail(ctx: Context, account: ProviderAccount, kindLabel: string): HTMLElement {
  const kind = ctx.info.kinds.find((k) => k.kind === account.kind)!;
  const status = h("span", { class: "muted" });
  if (account.kind === "local") {
    return section(
      kindLabel,
      "Spraakherkenning op je eigen computer. Je audio verlaat je computer niet.",
      h(
        "button",
        { class: "btn", onclick: () => ctx.navigate("local") },
        "Lokale modellen beheren",
      ),
    );
  }
  const name = h("input", {
    type: "text",
    value: account.name,
    onchange: () =>
      ctx.save((s) => {
        s.providers.find((p) => p.id === account.id)!.name = name.value.trim() || account.name;
      }),
  });
  const baseUrl = h("input", {
    type: "text",
    value: account.base_url,
    placeholder: kind.default_base_url || "https://…/v1",
    onchange: () =>
      ctx.save((s) => {
        s.providers.find((p) => p.id === account.id)!.base_url = baseUrl.value.trim();
      }),
  });
  const hasKey = ctx.info.keys[account.id];
  const key = h("input", {
    type: "password",
    placeholder: hasKey ? "•••••••• (bewaard)" : "Plak je sleutel",
    autocomplete: "off",
    class: "grow",
  });
  const saveKey = async () => {
    try {
      ctx.update(await api.setKey(account.id, key.value));
    } catch (e) {
      status.textContent = `Bewaren mislukt: ${errorText(e)}`;
    }
  };
  const test = async () => {
    status.textContent = "Testen…";
    try {
      const models = await api.providerModels(account.id);
      status.textContent = `Verbonden — ${models.length} modellen`;
    } catch (e) {
      status.textContent = `Mislukt: ${errorText(e)}`;
    }
  };
  const canDelete = ctx.info.settings.providers.filter((p) => p.kind !== "local").length > 1;
  return section(
    kindLabel,
    "Sleutels worden bewaard in de beveiligde opslag van je besturingssysteem.",
    field("Naam", name),
    field("Base URL", baseUrl),
    field(
      kind.requires_key ? "API-sleutel" : "Sleutel (optioneel)",
      h("div", { class: "row" }, key, h("button", { class: "btn", onclick: saveKey }, "Bewaar"),
        hasKey ? h("button", { class: "btn", onclick: async () => ctx.update(await api.setKey(account.id, "")) }, "Wis") : null),
    ),
    h(
      "div",
      { class: "row", style: "margin-top:12px" },
      h("button", { class: "btn", onclick: test }, "Test verbinding"),
      status,
      h("span", { class: "grow" }),
      h(
        "button",
        {
          class: "btn danger",
          disabled: !canDelete,
          onclick: async () => ctx.update(await api.removeProvider(account.id)),
        },
        "Provider verwijderen",
      ),
    ),
  );
}
