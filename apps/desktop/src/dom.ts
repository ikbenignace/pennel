type Child = Node | string | null | undefined | false;
type Attrs = Record<string, string | number | boolean | EventListener | null | undefined>;

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  ...children: (Child | Child[])[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === null || value === undefined || value === false) continue;
    if (key.startsWith("on") && typeof value === "function") {
      el.addEventListener(key.slice(2).toLowerCase(), value);
    } else if (key === "value" && "value" in el) {
      (el as HTMLInputElement).value = String(value);
    } else if (key === "checked" && "checked" in el) {
      (el as HTMLInputElement).checked = Boolean(value);
    } else {
      el.setAttribute(key, value === true ? "" : String(value));
    }
  }
  for (const child of children.flat()) {
    if (child === null || child === undefined || child === false) continue;
    el.append(child);
  }
  return el;
}

export function section(title: string, subtitle: string | null, ...body: (Child | Child[])[]) {
  const headingId = `section-${++sectionNumber}`;
  const node = h(
    "section",
    { class: "pane-section", "aria-labelledby": headingId },
    h("h3", { id: headingId }, title),
    subtitle ? h("p", { class: "subtitle" }, subtitle) : null,
    ...body,
  );
  for (const control of node.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input,select,textarea")) {
    if (!control.labels?.length && !control.hasAttribute("aria-label")) control.setAttribute("aria-labelledby", headingId);
  }
  return node;
}

let sectionNumber = 0;

export function field(label: string, control: Node) {
  return h("label", { class: "field" }, h("span", { class: "field-label" }, label), control);
}

export function select<T extends string>(
  options: [T, string][],
  value: T,
  onChange: (value: T) => void,
): HTMLSelectElement {
  const el = h(
    "select",
    { onchange: () => onChange(el.value as T) },
    options.map(([v, label]) => h("option", { value: v, selected: v === value }, label)),
  );
  el.value = value;
  return el;
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  return `${Math.round(bytes / 1e6)} MB`;
}

export function formatClock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor(total / 60) % 60;
  const remainder = String(total % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${remainder}` : `${String(minutes).padStart(2, "0")}:${remainder}`;
}

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (typeof e === "object" && e !== null && typeof (e as { message?: unknown }).message === "string") {
    return (e as { message: string }).message;
  }
  return JSON.stringify(e);
}

export function errorCode(e: unknown): string | null {
  const code = typeof e === "object" && e !== null ? (e as { code?: unknown }).code : null;
  return typeof code === "string" ? code : null;
}

export function toggle(
  label: string,
  checked: boolean,
  change: (value: boolean) => void,
  capability?: { status: string; reason: string | null },
) {
  const disabled = capability?.status === "unsupported" || capability?.status === "unavailable";
  const input = h("input", { type: "checkbox", checked, disabled });
  input.addEventListener("change", () => change(input.checked));
  return h("div", { class: "toggle-field" },
    h("label", { class: "checkbox" }, input, label),
    capability?.reason ? h("p", { class: "help" }, capability.reason) : null);
}

function inline(text: string): DocumentFragment {
  const result = document.createDocumentFragment();
  const pattern = /(\*\*[^*]+\*\*|`[^`]+`)/g;
  let offset = 0;
  for (const match of text.matchAll(pattern)) {
    result.append(document.createTextNode(text.slice(offset, match.index)));
    const bold = match[0].startsWith("**");
    result.append(h(bold ? "strong" : "code", {}, match[0].slice(bold ? 2 : 1, bold ? -2 : -1)));
    offset = (match.index ?? 0) + match[0].length;
  }
  result.append(document.createTextNode(text.slice(offset)));
  return result;
}

export function markdown(text: string): HTMLElement {
  const block = h("div", { class: "markdown" });
  for (const line of text.split("\n")) {
    const clean = line.trim();
    if (!clean) { block.append(h("div", { class: "paragraph-gap" })); continue; }
    const heading = clean.match(/^#{1,6}\s+(.+)/);
    const bullet = clean.match(/^[-*•]\s+(.+)/);
    const task = bullet?.[1].match(/^\[([ xX])\]\s+(.+)/);
    const node = heading ? h("h4", {}) : h("p", { class: bullet ? "bullet" : null });
    if (task) node.append(h("span", { class: "markdown-check", role: "img", "aria-label": task[1].trim() ? "Voltooid" : "Open actiepunt" }, task[1].trim() ? "☑ " : "□ "));
    else if (bullet) node.append(document.createTextNode("• "));
    node.append(inline(heading?.[1] ?? task?.[2] ?? bullet?.[1] ?? clean));
    block.append(node);
  }
  return block;
}

export function snippet(text: string): HTMLElement {
  const result = h("span", { class: "search-snippet" });
  for (const part of text.split(/(«[^»]*»)/g)) {
    result.append(part.startsWith("«") && part.endsWith("»")
      ? h("mark", {}, part.slice(1, -1)) : document.createTextNode(part));
  }
  return result;
}

export function confirmDialog(title: string, detail: string, action = "Verwijder"): Promise<boolean> {
  return new Promise((resolve) => {
    const dialog = h("dialog", { class: "editor-dialog" });
    const finish = (value: boolean) => { dialog.close(); dialog.remove(); resolve(value); };
    dialog.append(h("h2", {}, title), h("p", { class: "muted" }, detail),
      h("div", { class: "dialog-actions row" }, h("span", { class: "grow" }),
        h("button", { class: "btn", onclick: () => finish(false) }, "Annuleren"),
        h("button", { class: "btn danger", onclick: () => finish(true) }, action)));
    dialog.addEventListener("cancel", (event) => { event.preventDefault(); finish(false); });
    document.body.append(dialog);
    dialog.showModal();
  });
}

export function askText(title: string, value: string, detail = ""): Promise<string | null> {
  return new Promise((resolve) => {
    const dialog = h("dialog", { class: "editor-dialog" });
    const input = h("input", { type: "text", value, "aria-label": title });
    const finish = (result: string | null) => { dialog.close(); dialog.remove(); resolve(result); };
    const form = h("form", {}, h("h2", {}, title), detail ? h("p", { class: "muted" }, detail) : null, input,
      h("div", { class: "dialog-actions row" }, h("span", { class: "grow" }),
        h("button", { class: "btn", type: "button", onclick: () => finish(null) }, "Annuleren"),
        h("button", { class: "btn primary", type: "submit" }, "Bewaar")));
    form.addEventListener("submit", (event) => { event.preventDefault(); finish(input.value.trim()); });
    dialog.addEventListener("cancel", (event) => { event.preventDefault(); finish(null); });
    dialog.append(form);
    document.body.append(dialog);
    dialog.showModal();
    input.focus();
    input.select();
  });
}
