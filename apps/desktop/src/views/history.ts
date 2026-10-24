import { api, type HistoryEntry } from "../api";
import { errorText, h, section } from "../dom";
import type { Context } from "../main";

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
      h("div", { class: "muted" }, when(entry.created_at_ms), entry.app_bundle_id ? ` · ${entry.app_bundle_id}` : ""),
      h("p", { class: failed ? "error" : "" }, failed ? `Mislukt: ${entry.error ?? "onbekende fout"}` : entry.text),
      h(
        "div",
        { class: "row" },
        failed && entry.has_audio
          ? h(
              "button",
              { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) },
              retrying.has(entry.id) ? "Bezig…" : "Opnieuw proberen",
            )
          : null,
        failed
          ? null
          : h("button", { class: "btn", onclick: () => navigator.clipboard.writeText(entry.text) }, "Kopiëren"),
        h(
          "button",
          {
            class: "btn",
            onclick: async () => {
              await api.deleteHistory(entry.id);
              ctx.rerender();
            },
          },
          "Verwijder",
        ),
      ),
    );
  };
  return h(
    "div",
    {},
    h("h1", {}, "Geschiedenis"),
    message ? h("p", { class: "error" }, message) : null,
    section(
      "Dictaten",
      null,
      entries.length ? entries.map(row) : h("p", { class: "muted" }, "Nog geen dictaten."),
    ),
  );
}
