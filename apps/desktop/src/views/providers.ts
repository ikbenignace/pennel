import { api, type ProviderAccount, type ProviderKind } from "../api";
import { errorText, field, h, section } from "../dom";
import { localView } from "./local";
import type { Context } from "../main";

let selected: string | null = null;

export async function providersView(ctx: Context): Promise<HTMLElement> {
  const { settings, kinds, keys } = ctx.info;
  if (ctx.param === "__local__") selected = settings.providers.find((provider) => provider.kind === "local")?.id ?? selected;
  if (!settings.providers.some((provider) => provider.id === selected)) selected = settings.providers[0]?.id ?? null;
  const account = settings.providers.find((provider) => provider.id === selected);
  const localReady = settings.providers.some((provider) => provider.kind === "local")
    && (await api.localModels(null, false)).some((model) => model.state.kind === "downloaded");
  const list = h("div", { class: "list", "aria-label": "Providers" }, settings.providers.map((provider) => {
    const kind = kinds.find((item) => item.kind === provider.kind);
    const ready = kind?.availability.status === "available" && (!kind.requires_key || !!keys[provider.id])
      && (!kind.is_cli_agent || !!provider.base_url) && (provider.kind !== "local" || localReady);
    return h("button", { "aria-current": provider.id === selected ? "true" : null, onclick: () => { selected = provider.id; ctx.navigate("providers"); } },
      h("span", { class: `status-dot${ready ? " ready" : ""}` }),
      h("span", { class: "grow" }, h("span", { class: "provider-name" }, provider.name),
        h("span", { class: "provider-kind" }, kind?.label ?? provider.kind)));
  }));
  const add = h("select", { class: "provider-add", "aria-label": "Provider toevoegen" }, h("option", { value: "" }, "Provider toevoegen…"));
  for (const kind of kinds) {
    if ((kind.kind === "local" || kind.kind === "apple_speech" || kind.kind === "apple_intelligence") && settings.providers.some((provider) => provider.kind === kind.kind)) continue;
    const unsupported = kind.availability.status === "unsupported";
    add.append(h("option", { value: kind.kind, disabled: unsupported, title: kind.availability.reason },
      unsupported && kind.availability.reason ? `${kind.label} — ${kind.availability.reason}` : kind.label));
  }
  add.addEventListener("change", () => {
    if (!add.value) return;
    const kind = add.value as ProviderKind;
    void ctx.mutate(async () => {
      const next = await api.addProvider(kind);
      selected = next.settings.providers.at(-1)?.id ?? null;
      return next;
    });
  });
  list.append(add);
  return h("div", {},
    ctx.info.migration_warning ? section("API-sleutels", null,
      h("p", { class: "help", role: "status" }, ctx.info.migration_warning),
      h("p", { class: "help" }, "Controleer je providers en stel een ontbrekende sleutel opnieuw in. Je bestaande sleutels blijven bewaard.")) : null,
    h("div", { class: "providers" }, list,
      account ? await detail(ctx, account) : h("p", { class: "muted" }, "Kies een provider")));
}

async function detail(ctx: Context, account: ProviderAccount): Promise<HTMLElement> {
  const kind = ctx.info.kinds.find((item) => item.kind === account.kind);
  if (!kind) return section(account.name, "Deze provider is niet beschikbaar in deze versie.");
  const status = h("p", { class: "help", role: "status" });
  const saveAccount = (key: "name" | "base_url", value: string) => ctx.save((settings) => {
    const provider = settings.providers.find((item) => item.id === account.id);
    if (provider) provider[key] = value;
  });
  const name = h("input", { type: "text", value: account.name, "aria-label": "Naam" });
  name.addEventListener("change", () => { void saveAccount("name", name.value.trim() || account.name); });
  const body = section(kind.label, null, field("Naam", name));
  if (kind.availability.reason) body.append(h("p", { class: "help" }, kind.availability.reason));
  if (account.kind === "apple_speech") {
    body.append(h("p", { class: "help" }, "Het dicteermodel van macOS zelf. Gratis, volledig offline. macOS beheert de taalbestanden. Werkt voor dicteren en meetings vanaf macOS 26."));
    if (kind.availability.status === "permission_required") {
      const speech = (await api.permissions()).find((permission) => permission.kind === "speech");
      let disposed = false;
      let checking = false;
      const timer = window.setInterval(async () => {
        if (checking || disposed) return;
        checking = true;
        try {
          const next = (await api.permissions()).find((permission) => permission.kind === "speech");
          if (!disposed && next?.status !== speech?.status) await ctx.refresh();
        } catch (error) {
          if (!disposed) { window.clearInterval(timer); ctx.report(error); }
        } finally { checking = false; }
      }, 1000);
      ctx.onDispose(() => { disposed = true; window.clearInterval(timer); });
      body.append(h("button", { class: "btn", onclick: async () => { await api.requestPermission("speech"); await ctx.refresh(); } },
        speech?.status === "denied" ? "Open instellingen" : "Geef toegang"));
    }
    body.append(h("button", { class: "btn", disabled: kind.availability.status !== "available", onclick: () => ctx.save((settings) => {
      settings.dictation_stt = { provider_id: account.id, model: kind.default_stt_model };
      settings.meeting_stt = { provider_id: account.id, model: kind.default_stt_model };
    }) }, "Gebruik voor dicteren en meetings"));
  } else if (account.kind === "apple_intelligence") {
    body.append(h("p", { class: "help" }, "Het ingebouwde taalmodel van Apple Intelligence. Gratis en volledig op deze Mac, zonder API-sleutel of aparte modeldownload. Voor opkuisen, opdrachten, samenvatten en vragen over meetings."));
    const test = h("button", { class: "btn", disabled: kind.availability.status !== "available", onclick: async () => {
      test.disabled = true; status.textContent = "Testen…";
      try { await api.testAppleIntelligence(); status.textContent = "✓ Werkt volledig op deze Mac"; }
      catch (error) { status.textContent = `Mislukt: ${errorText(error)}`; }
      test.disabled = false;
    } }, "Test model");
    body.append(h("div", { class: "row" },
      h("button", { class: "btn", disabled: kind.availability.status !== "available", onclick: () => ctx.save((settings) => {
        settings.llm = { provider_id: account.id, model: kind.default_llm_model };
      }) }, "Gebruik voor tekstverwerking"),
      test,
      h("button", { class: "btn", onclick: () => ctx.refresh().catch(ctx.report) }, "Controleer beschikbaarheid")), status);
  } else if (kind.is_cli_agent) {
    body.append(h("p", { class: "help" }, "Gebruikt je eigen abonnement via de CLI op deze computer, voor opkuisen, samenvatten en vragen. Pennel draait hem in een lege map, zonder tools."));
    const executable = h("input", { type: "text", value: account.base_url, placeholder: "Pad naar de CLI", "aria-label": "Programma" });
    executable.addEventListener("change", () => { void saveAccount("base_url", executable.value.trim()); });
    const detect = h("button", { class: "btn", onclick: async () => {
      detect.disabled = true;
      try {
        const found = (await api.detectCliAgents()).find((agent) => agent.kind === account.kind);
        if (!found) status.textContent = "Niet gevonden. Installeer de CLI en meld je aan, of geef het pad hierboven op.";
        else { await saveAccount("base_url", found.path); }
      } catch (error) { status.textContent = errorText(error); }
      detect.disabled = false;
    } }, "Zoeken");
    const check = h("button", { class: "btn", onclick: async () => {
      check.disabled = true; status.textContent = "Testen…";
      try { await api.checkCliAgent(account.id, kind.default_llm_model); status.textContent = "✓ Werkt"; }
      catch (error) { status.textContent = `Mislukt: ${errorText(error)}`; }
      check.disabled = false;
    } }, "Test");
    body.append(field("Programma", executable), h("div", { class: "row" }, detect, check), status);
  } else if (account.kind !== "local") {
    const baseUrl = h("input", { type: "text", value: account.base_url, placeholder: kind.default_base_url || "https://…/v1", "aria-label": "Base URL" });
    baseUrl.addEventListener("change", () => { void saveAccount("base_url", baseUrl.value.trim()); });
    body.append(field("Base URL", baseUrl));
    if (kind.requires_key) {
      const key = h("input", { type: "password", placeholder: ctx.info.keys[account.id] ? "•••••••• (bewaard)" : "Plak je sleutel", autocomplete: "off", class: "grow", "aria-label": "API-sleutel" });
      key.addEventListener("change", () => { void ctx.mutate(() => api.setKey(account.id, key.value)); });
      key.addEventListener("keydown", (event) => { if (event.key === "Enter") key.blur(); });
      body.append(field("API-sleutel", h("div", { class: "row" }, key,
        ctx.info.keys[account.id] ? h("button", { class: "btn", onclick: () => ctx.mutate(() => api.setKey(account.id, "")) }, "Wis") : null)));
    }
    const test = h("button", { class: "btn", onclick: async () => {
      test.disabled = true; status.textContent = "Testen…";
      try { const models = await api.providerModels(account.id); status.textContent = `✓ Verbonden · ${models.length} modellen`; }
      catch (error) { status.textContent = `Mislukt: ${errorText(error)}`; }
      test.disabled = false;
    } }, "Test verbinding");
    body.append(h("div", { class: "row" }, status, h("span", { class: "grow" }), test));
  }
  return h("div", {}, body, account.kind === "local" ? await localView(ctx, true) : null,
    h("div", { class: "row" }, h("span", { class: "grow" }), h("button", {
      class: "btn danger", disabled: ctx.info.settings.providers.length < 2,
      onclick: () => ctx.mutate(() => api.removeProvider(account.id)),
    }, "Provider verwijderen")));
}
