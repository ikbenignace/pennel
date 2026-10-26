import { l } from "../i18n.ts";
import { api, listen, type RecordPrompt } from "../api";
import { errorText, h } from "../dom";
import type { Context } from "../main";

export async function promptView(ctx: Context): Promise<HTMLElement> {
  const node = h("div", { class: "record-prompt" });
  let current: RecordPrompt | null = null;
  let pending = false;
  const draw = (prompt: RecordPrompt | null) => {
    current = prompt;
    pending = false;
    if (!prompt || prompt.expires_at_ms <= Date.now()) { node.replaceChildren(); return; }
    const status = h("span", { class: "help", role: "status" });
    const respond = async (answer: "yes" | "no" | "never") => {
      if (!current || pending || current.id !== prompt.id) return;
      const id = prompt.id;
      pending = true;
      node.querySelectorAll("button").forEach((button) => { button.disabled = true; });
      try { await api.answerRecordPrompt(answer, id); }
      catch (error) { status.textContent = errorText(error); }
      if (current?.id === id) node.replaceChildren();
    };
    node.replaceChildren(
      h("div", { class: "row" }, h("span", { class: "status-dot" }), h("strong", {}, l("dictation.prompt.title"))),
      h("p", { class: "help prompt-description" }, [prompt.app_name, prompt.title].filter(Boolean).join(" · ")),
      h("div", { class: "row" }, h("button", { class: "btn", onclick: () => respond("yes") }, l("common.yes")),
        h("button", { class: "btn", onclick: () => respond("no") }, l("common.no")), h("span", { class: "grow" }),
        h("button", { class: "text-button", onclick: () => respond("never") }, l("dictation.prompt.never", { app: prompt.app_name }))), status);
  };
  let eventReceived = false;
  const dispose = await listen<RecordPrompt | null>("record-prompt", (event) => { eventReceived = true; draw(event.payload); });
  ctx.onDispose(dispose);
  const snapshot = await api.recordPrompt();
  if (!eventReceived) draw(snapshot);
  const timer = window.setInterval(() => { if (current && current.expires_at_ms <= Date.now()) draw(null); }, 1000);
  ctx.onDispose(() => window.clearInterval(timer));
  return node;
}
