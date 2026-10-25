import { api, type HistoryEntry } from "../api";
import { confirmDialog, errorText, h } from "../dom";
import type { Context } from "../main";
import { applicationLabel } from "../presentation";
import { icon } from "../icons";
import { appBadge } from "../app-icon";

const retrying = new Set<string>();
let message: string | null = null;
let messageFailed = false;

function when(ms: number): string {
  return new Date(ms).toLocaleString("nl-BE", { dateStyle: "medium", timeStyle: "short" });
}

export async function historyView(ctx: Context): Promise<HTMLElement> {
  const entries = await api.history(200);
  const retry = async (entry: HistoryEntry) => {
    retrying.add(entry.id);
    ctx.rerender();
    try {
      if (entry.status === "unsaved") await api.saveHistory(entry.id);
      else await api.retryHistory(entry.id);
      message = entry.status === "unsaved" ? "Dictaat bewaard." : "Dictaat hersteld. Je kunt de tekst nu kopiëren.";
      messageFailed = false;
    } catch (e) {
      message = `${entry.status === "unsaved" ? "Bewaren" : "Opnieuw proberen"} mislukt: ${errorText(e)}`;
      messageFailed = true;
    }
    retrying.delete(entry.id);
    ctx.rerender();
  };
  const row = (entry: HistoryEntry) => {
    const failed = entry.status === "failed";
    const unsaved = entry.status === "unsaved";
    const source = entry.app_bundle_id ? appBadge(entry.app_bundle_id) : null;
    if (source) source.icon.title = source.name.textContent ?? "";
    return h(
      "div",
      { class: "history-entry" },
      h("span", { class: "history-destination" }, source?.icon ?? h("span", { class: "app-icon", "aria-hidden": "true" })),
      h("div", { class: "history-content" },
        unsaved ? h("p", { class: "error", role: "status" }, "Nog niet bewaard") : null,
        h("p", { class: failed ? "error" : "" }, failed ? `Mislukt: ${entry.error ?? "onbekende fout"}` : entry.text),
        unsaved ? h("p", { class: "help" }, "Deze tekst staat alleen tijdelijk in Pennel en verdwijnt als Pennel stopt of crasht. Kopieer de tekst of probeer opnieuw te bewaren.") : null,
        unsaved ? h("button", { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) }, retrying.has(entry.id) ? "Bezig…" : "Opnieuw bewaren") : null,
        h("div", { class: "history-metadata" }, when(entry.created_at_ms), entry.app_bundle_id ? h("span", { class: "visually-hidden" }, ` · ${applicationLabel(entry.app_bundle_id)}`) : "")),
      h(
        "div",
        { class: "history-actions" },
        failed && entry.has_audio
          ? h(
              "button",
              { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) },
              retrying.has(entry.id) ? "Bezig…" : "Opnieuw proberen",
            )
          : null,
        failed
          ? null
          : h("button", { class: "btn", "aria-label": "Dictaat kopiëren", onclick: () => api.copyText(entry.text) }, "Kopiëren"),
        h(
          "button",
          {
            class: "button-icon",
            "aria-label": "Dictaat verwijderen",
            onclick: async () => {
              if (unsaved && !await confirmDialog("Niet bewaard dictaat verwijderen?", "Dit verwijdert de tijdelijke tekst. Kopieer de tekst eerst als je die wilt houden.", "Verwijder")) return;
              try { await api.deleteHistory(entry.id); ctx.rerender(); }
              catch (error) { ctx.report(error); }
            },
          },
          icon("trash"),
        ),
      ),
    );
  };
  return h(
    "div",
    { class: "history-view" },
    h("h1", {}, "Geschiedenis"),
    message ? h("p", { class: messageFailed ? "error" : "help", role: "status" }, message) : null,
    entries.length ? h("div", { class: "history-list" }, entries.map(row)) : h("div", { class: "empty-state muted" }, "Nog geen dictaten."),
  );
}
