import { api, type HistoryEntry } from "../api";
import { errorText, h } from "../dom";
import type { Context } from "../main";
import { applicationLabel } from "../presentation";
import { icon } from "../icons";

const retrying = new Set<string>();
let message: string | null = null;

function when(ms: number): string {
  return new Date(ms).toLocaleString("nl-BE", { dateStyle: "medium", timeStyle: "short" });
}

export async function historyView(ctx: Context): Promise<HTMLElement> {
  const entries = await api.history(200);
  const retry = async (entry: HistoryEntry) => {
    retrying.add(entry.id);
    ctx.rerender();
    try {
      await api.retryHistory(entry.id);
      message = null;
    } catch (e) {
      message = `Opnieuw proberen mislukt: ${errorText(e)}`;
    }
    retrying.delete(entry.id);
    ctx.rerender();
  };
  const row = (entry: HistoryEntry) => {
    const failed = entry.status === "failed";
    return h(
      "div",
      { class: "history-entry" },
      h("span", { class: "history-destination", "aria-hidden": true }, icon("laptop")),
      h("div", { class: "history-content" },
        h("p", { class: failed ? "error" : "" }, failed ? `Mislukt: ${entry.error ?? "onbekende fout"}` : entry.text),
        h("div", { class: "history-metadata" }, when(entry.created_at_ms), entry.app_bundle_id ? ` · ${applicationLabel(entry.app_bundle_id)}` : "")),
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
          : h("button", { class: "button-icon", "aria-label": "Dictaat kopiëren", onclick: () => api.copyText(entry.text) }, icon("copy")),
        h(
          "button",
          {
            class: "button-icon",
            "aria-label": "Dictaat verwijderen",
            onclick: async () => {
              await api.deleteHistory(entry.id);
              ctx.rerender();
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
    message ? h("p", { class: "error" }, message) : null,
    entries.length ? h("div", { class: "history-list" }, entries.map(row)) : h("div", { class: "empty-state muted" }, "Nog geen dictaten."),
  );
}
