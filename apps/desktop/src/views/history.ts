import { bindLocalization, l, t, localizeBackendText, formatDateTime } from "../i18n.ts";
import { api, type HistoryEntry } from "../api";
import { confirmDialog, errorText, h } from "../dom";
import type { Context } from "../main";
import { applicationLabel } from "../presentation";
import { icon } from "../icons";
import { appBadge } from "../app-icon";
import { HistoryFocusMemory, type HistoryFocusTarget } from "../history-focus-state";

const retrying = new Set<string>();
type HistoryMessage = { key: "dictation.history.saved" | "dictation.history.recovered" | "dictation.history.save_failed" | "dictation.history.retry_failed"; error?: string };
let message: HistoryMessage | null = null;
let messageFailed = false;
const focusMemory = new HistoryFocusMemory();

function when(ms: number): string {
  return formatDateTime(ms, { dateStyle: "medium", timeStyle: "short" });
}

export async function historyView(ctx: Context): Promise<HTMLElement> {
  const previousView = document.querySelector<HTMLElement>(".history-view");
  const invoker = document.activeElement;
  const previousFocus = document.activeElement instanceof HTMLElement && previousView?.contains(document.activeElement) ? document.activeElement : null;
  const previousEntry = previousFocus?.closest<HTMLElement>("[data-history-id]")?.dataset.historyId;
  const previousAction = previousFocus?.dataset.historyAction;
  const entries = await api.history(200);
  const available = new Set(entries.map((entry) => entry.id));
  const rows = new Map<string, { element: HTMLElement; actions: Map<string, HTMLElement> }>();
  const requested = ctx.param;
  const selected = requested ? available.has(requested) ? { id: requested } : null : focusMemory.selection(available);
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
    const actions = new Map<string, HTMLElement>();
    const action = <T extends HTMLElement>(name: string, element: T): T => { element.dataset.historyAction = name; actions.set(name, element); return element; };
    const element = h(
      "div",
      { class: `history-entry${requested === entry.id ? " history-target source-highlight" : ""}${selected?.id === entry.id ? " history-selected" : ""}`, "data-history-id": entry.id,
        role: "group", tabindex: -1, "aria-label": l("dictation.history.entry_label", () => ({ time: when(entry.created_at_ms) })),
        "aria-current": selected?.id === entry.id ? "true" : null,
        "aria-describedby": requested === entry.id ? "history-target-notice" : null },
      h("span", { class: "history-destination" }, source?.icon ?? h("span", { class: "app-icon", "aria-hidden": "true" })),
      h("div", { class: "history-content" },
        unsaved ? h("p", { class: "error", role: "status" }, l("dictation.history.unsaved")) : null,
        h("p", { class: failed ? "error" : "" }, failed ? l("dictation.history.failed", () => ({ error: entry.error ? localizeBackendText(entry.error) : t("dictation.history.unknown_error") })) : entry.text),
        unsaved ? h("p", { class: "help" }, l("dictation.history.temporary")) : null,
        unsaved ? action("save", h("button", { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) }, retrying.has(entry.id) ? l("dictation.working") : l("dictation.history.save_again"))) : null,
        h("div", { class: "history-metadata" }, timestamp, entry.app_bundle_id ? h("span", { class: "visually-hidden" }, ` · ${applicationLabel(entry.app_bundle_id)}`) : "")),
      h(
        "div",
        { class: "history-actions" },
        failed && entry.has_audio
          ? action("retry", h(
              "button",
              { class: "btn", disabled: retrying.has(entry.id), onclick: () => retry(entry) },
              retrying.has(entry.id) ? l("dictation.working") : l("common.retry"),
            ))
          : null,
        failed
          ? null
          : action("copy", h("button", { class: "btn", "aria-label": l("dictation.history.copy"), onclick: () => api.copyText(entry.text) }, l("common.copy"))),
        action("delete", h(
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
        )),
      ),
    );
    rows.set(entry.id, { element, actions });
    return element;
  };
  const heading = h("h1", { tabindex: -1 }, l("dictation.history.title"));
  const targetNotice = requested ? h("p", { id: "history-target-notice", class: "help history-target-notice", role: "status", tabindex: -1 },
    l(entries.some((entry) => entry.id === requested) ? "dictation.history.target_found" : "dictation.history.target_missing")) : null;
  const result = h(
    "div",
    { class: "history-view" },
    heading, targetNotice,
    message ? h("p", { class: messageFailed ? "error" : "help", role: "status" }, l(message.key, () => ({ error: message?.error ? localizeBackendText(message.error) : "" }))) : null,
    entries.length ? h("div", { class: "history-list" }, entries.map(row)) : h("div", { class: "empty-state muted" }, l("dictation.history.empty")),
  );
  const showSelection = (selection: HistoryFocusTarget | null) => {
    for (const [id, row] of rows) {
      row.element.classList.toggle("history-selected", selection?.id === id);
      if (selection?.id === id) row.element.setAttribute("aria-current", "true");
      else row.element.removeAttribute("aria-current");
      if (selection?.id !== id) {
        row.element.classList.remove("history-target", "source-highlight");
        row.element.removeAttribute("aria-describedby");
      }
    }
    if (targetNotice) targetNotice.hidden = selection?.id !== requested;
  };
  const rememberIntent = (event: Event) => {
    if (!(event.target instanceof Element)) return;
    const entry = event.target.closest<HTMLElement>("[data-history-id]");
    const id = entry?.dataset.historyId;
    if (!entry || !id || !result.contains(entry)) return;
    focusMemory.remember(id, event.target.closest<HTMLElement>("[data-history-action]")?.dataset.historyAction);
    showSelection(focusMemory.selection(available));
  };
  result.addEventListener("focusin", rememberIntent);
  result.addEventListener("pointerdown", rememberIntent);
  requestAnimationFrame(() => {
    if (!result.isConnected) return;
    const plan = focusMemory.plan({ requested, previous: previousEntry ? { id: previousEntry, action: previousAction } : null, available,
      returning: !previousView, unobstructed: !document.activeElement || document.activeElement === document.body || document.activeElement === invoker });
    const row = plan.target ? rows.get(plan.target.id) : null;
    const target = plan.target?.action ? row?.actions.get(plan.target.action) ?? row?.element : row?.element;
    const focus = target instanceof HTMLButtonElement && target.disabled ? row?.element : target;
    const destination = focus ?? (plan.fallback === "notice" ? targetNotice : plan.fallback === "heading" ? heading : null);
    destination?.focus({ preventScroll: true });
    if (plan.scroll) destination?.scrollIntoView({ block: plan.scroll });
    if (plan.target) showSelection(focusMemory.selection(available));
  });
  return result;
}
