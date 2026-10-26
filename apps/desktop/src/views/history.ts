import { bindLocalization, l, t, localizeBackendText, formatDateTime } from "../i18n.ts";
import { api, type HistoryEntry } from "../api";
import { confirmDialog, errorText, h } from "../dom";
import type { Context } from "../main";
import { applicationLabel } from "../presentation";
import { icon } from "../icons";
import { appBadge } from "../app-icon";

const retrying = new Set<string>();
type HistoryMessage = { key: "dictation.history.saved" | "dictation.history.recovered" | "dictation.history.save_failed" | "dictation.history.retry_failed"; error?: string };
let message: HistoryMessage | null = null;
let messageFailed = false;

function when(ms: number): string {
  return formatDateTime(ms, { dateStyle: "medium", timeStyle: "short" });
}

export async function historyView(ctx: Context): Promise<HTMLElement> {
  const entries = await api.history(200);
  const retry = async (entry: HistoryEntry) => {
    retrying.add(entry.id);
    ctx.rerender();
    try {
      if (entry.status === "unsaved") await api.saveHistory(entry.id);
      else await api.retryHistory(entry.id);
      message = { key: entry.status === "unsaved" ? "dictation.history.saved" : "dictation.history.recovered" };
      messageFailed = false;
    } catch (e) {
      message = { key: entry.status === "unsaved" ? "dictation.history.save_failed" : "dictation.history.retry_failed", error: errorText(e) };
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
    const timestamp = h("span", {}, when(entry.created_at_ms));
    bindLocalization(timestamp, () => { timestamp.textContent = when(entry.created_at_ms); });
    return h(
      "div",
      { class: "history-entry" },
      h("span", { class: "history-destination" }, source?.icon ?? h("span", { class: "app-icon", "aria-hidden": "true" })),
      h("div", { class: "history-content" },
        unsaved ? h("p", { class: "error", role: "status" }, l("dictation.history.unsaved")) : null,
        h("p", { class: failed ? "error" : "" }, failed ? l("dictation.history.failed", () => ({ error: entry.error ? localizeBackendText(entry.error) : t("dictation.history.unknown_error") })) : entry.text),
        unsaved ? h("p", { class: "help" }, l("dictation.history.temporary")) : null,
        unsaved ? h("button", { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) }, retrying.has(entry.id) ? l("dictation.working") : l("dictation.history.save_again")) : null,
        h("div", { class: "history-metadata" }, timestamp, entry.app_bundle_id ? h("span", { class: "visually-hidden" }, ` · ${applicationLabel(entry.app_bundle_id)}`) : "")),
      h(
        "div",
        { class: "history-actions" },
        failed && entry.has_audio
          ? h(
              "button",
              { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) },
              retrying.has(entry.id) ? l("dictation.working") : l("common.retry"),
            )
          : null,
        failed
          ? null
          : h("button", { class: "btn", "aria-label": l("dictation.history.copy"), onclick: () => api.copyText(entry.text) }, l("common.copy")),
        h(
          "button",
          {
            class: "button-icon",
            "aria-label": l("dictation.history.delete"),
            onclick: async () => {
              if (unsaved && !await confirmDialog(l("dictation.history.discard_title"), l("dictation.history.discard_help"), l("common.delete"))) return;
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
    h("h1", {}, l("dictation.history.title")),
    message ? h("p", { class: messageFailed ? "error" : "help", role: "status" }, l(message.key, () => ({ error: message?.error ? localizeBackendText(message.error) : "" }))) : null,
    entries.length ? h("div", { class: "history-list" }, entries.map(row)) : h("div", { class: "empty-state muted" }, l("dictation.history.empty")),
  );
}
