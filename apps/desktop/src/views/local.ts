import { listen } from "@tauri-apps/api/event";
import { api, type LocalModel } from "../api";
import { confirmDialog, field, formatBytes, h, section, select } from "../dom";
import type { Context } from "../main";

const progress = new Map<string, { downloaded: number; total: number }>();
const failures = new Map<string, string>();
let language: string | null = null;
let supportedOnly = false;
let listening = false;
let refresh: (() => void) | null = null;

async function listenOnce() {
  if (listening) return;
  listening = true;
  await listen<{ id: string; downloaded: number; total: number }>("download-progress", (e) => {
    progress.set(e.payload.id, e.payload);
    const bar = document.querySelector<HTMLProgressElement>(`progress[data-model="${CSS.escape(e.payload.id)}"]`);
    if (bar) {
      bar.max = e.payload.total;
      bar.value = e.payload.downloaded;
    }
  });
  await listen<{ id: string; error: string | null }>("download-done", (e) => {
    progress.delete(e.payload.id);
    if (e.payload.error && e.payload.error !== "cancelled") failures.set(e.payload.id, e.payload.error);
    refresh?.();
  });
}

function bar(label: string, value: number | null) {
  return h(
    "span",
    { class: "muted" },
    `${label} `,
    h("span", { class: "bar" }, h("span", { style: `width:${value ?? 0}%` })),
  );
}

export async function localView(ctx: Context, embedded = false): Promise<HTMLElement> {
  await listenOnce();
  const models = await api.localModels(language, supportedOnly);
  const inUse = new Set(
    [ctx.info.settings.dictation_stt, ctx.info.settings.meeting_stt]
      .filter((c) => ctx.info.settings.providers.find((p) => p.id === c.provider_id)?.kind === "local")
      .map((c) => c.model),
  );
  const localId = ctx.info.settings.providers.find((p) => p.kind === "local")?.id;

  const use = (model: LocalModel) =>
    ctx.save((s) => {
      if (!localId) return;
      s.dictation_stt = { provider_id: localId, model: model.id };
    });

  const card = (model: LocalModel) => {
    const active = progress.get(model.id);
    let actions: (Node | string)[];
    if (active) {
      actions = [
        h("progress", { "data-model": model.id, max: active.total || 1, value: active.downloaded }),
        h("button", { class: "btn", onclick: () => api.cancelDownload(model.id) }, "Annuleren"),
      ];
    } else if (model.state.kind === "downloaded") {
      actions = [
        inUse.has(model.id)
          ? h("span", { class: "badge" }, "In gebruik")
          : h("button", { class: "btn", onclick: () => use(model) }, "Gebruik"),
        h(
          "button",
          {
            class: "btn danger",
            onclick: async () => {
              if (inUse.has(model.id) && !await confirmDialog("Dit model is in gebruik. Verwijderen?", "Dicteren schakelt over naar een ander gedownload model, of werkt niet meer tot je er een downloadt.")) return;
              await ctx.mutate(() => api.deleteModel(model.id));
            },
          },
          "Verwijder",
        ),
      ];
    } else {
      const start = async () => {
        failures.delete(model.id);
        progress.set(model.id, { downloaded: 0, total: model.size_bytes });
        await api.downloadModel(model.id);
        ctx.rerender();
      };
      actions =
        model.state.kind === "partial"
          ? [
              h("button", { class: "btn", onclick: start }, "Verder downloaden"),
              h("button", { class: "btn danger", onclick: async () => await ctx.mutate(() => api.deleteModel(model.id)) }, "Verwijder"),
            ]
          : [h("button", { class: "btn primary", onclick: start }, "Downloaden")];
    }
    return h(
      "div",
      { class: "model-card" },
      h(
        "div",
        {},
        h(
          "div",
          { class: "row" },
          h("strong", {}, model.name),
          model.recommended ? h("span", { class: "badge" }, "Aanbevolen") : null,
          h("span", { class: "muted" }, formatBytes(model.size_bytes)),
        ),
        h("div", { class: "muted" }, model.description),
        h("div", { class: "row" }, bar("Snelheid", model.speed), bar("Nauwkeurigheid", model.accuracy)),
        failures.has(model.id) ? h("div", { class: "error" }, `Download mislukt: ${failures.get(model.id)}`) : null,
      ),
      h("div", { class: "row" }, actions),
    );
  };

  const languagePicker = select<string>(
    [
      ["", "Alle talen"],
      ["nl", "Nederlands"],
      ["en", "Engels"],
      ["fr", "Frans"],
      ["de", "Duits"],
    ],
    language ?? "",
    (value) => {
      language = value || null;
      ctx.rerender();
    },
  );
  const onlyThis = h("input", {
    type: "checkbox",
    checked: supportedOnly,
    onchange: () => {
      supportedOnly = onlyThis.checked;
      ctx.rerender();
    },
  });

  const root = h(
    "div",
    {},
    embedded ? null : h("h1", {}, "Lokale modellen"),
    section(
      "Modellen",
      `Draaien volledig op je computer (${ctx.info.capabilities.local_acceleration === "metal" ? "Metal" : ctx.info.capabilities.local_acceleration === "vulkan" ? "Vulkan" : "CPU"}). Je audio verlaat je computer niet.`,
      h("p", { class: "help" }, "Snelheid is gebenchmarkt op Apple Silicon. De prestaties op deze computer kunnen verschillen."),
      h("div", { class: "row" }, field("Nauwkeurigheid voor", languagePicker), h("label", { class: "row" }, onlyThis, "Alleen deze taal")),
      models.filter((model) => model.recommended || model.state.kind === "downloaded").map(card),
      h("details", {}, h("summary", {}, "Alle modellen"), models.filter((model) => !model.recommended && model.state.kind !== "downloaded").map(card)),
    ),
    section(
      "Geheugen",
      "Het model wordt geladen bij je eerste dictaat en daarna uit het geheugen gehaald als je het niet gebruikt.",
      field(
        "Vrijgeven na",
        select<string>(
          [
            ["1", "1 min"],
            ["5", "5 min"],
            ["15", "15 min"],
            ["60", "1 uur"],
          ],
          String(ctx.info.settings.local_unload_minutes),
          (v) => ctx.save((s) => (s.local_unload_minutes = Number(v))),
        ),
      ),
    ),
  );
  const onRefresh = () => { if (root.isConnected) void ctx.refresh().catch(ctx.report); };
  refresh = onRefresh;
  ctx.onDispose(() => { if (refresh === onRefresh) refresh = null; });
  return root;
}
