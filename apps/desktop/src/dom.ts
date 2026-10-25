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
  for (const choice of node.querySelectorAll<HTMLElement>(".choice")) {
    if (!choice.closest(".field") && !choice.hasAttribute("aria-label")) choice.setAttribute("aria-label", title);
  }
  for (const control of node.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input,select,textarea")) {
    if (!control.closest("label") && !control.labels?.length && !control.hasAttribute("aria-label") && !control.hasAttribute("aria-labelledby")) control.setAttribute("aria-labelledby", headingId);
  }
  return node;
}

let sectionNumber = 0;

export function field(label: string, control: HTMLElement) {
  const formControl = control.matches("input,textarea");
  if (control.classList.contains("choice") && !control.hasAttribute("aria-label")) control.setAttribute("aria-label", label);
  return h(formControl ? "label" : "div", { class: "field" }, h("span", { class: "field-label" }, label), control);
}

export interface ChoiceOption { readonly value: string; readonly label: string; disabled: boolean }
/** A choice control with the subset of the select element's interface the views use. */
export interface ChoiceElement<T extends string = string> extends HTMLElement {
  value: T;
  disabled: boolean;
  readonly options: ChoiceOption[];
}
export type ChoiceStyle = "auto" | "segmented" | "menu";

/**
 * A few short fixed options become a segmented pill; longer or open-ended lists a pill button that opens
 * a menu. Neither uses a system dropdown, so both keep the brand look on every platform.
 */
export function select<T extends string>(
  options: [T, string][],
  value: T,
  onChange: (value: T) => void,
  style: ChoiceStyle = "auto",
): ChoiceElement<T> {
  const segmented = style === "segmented" || (style === "auto" && options.length <= 4 && options.reduce((total, [, label]) => total + label.length, 0) <= 44);
  return segmented ? segmentedChoice(options, value, onChange) : menuChoice(options, value, onChange);
}

function segmentedChoice<T extends string>(options: [T, string][], value: T, onChange: (value: T) => void): ChoiceElement<T> {
  let current = value;
  let disabled = false;
  const root = h("div", { class: "segmented choice", role: "radiogroup" }) as unknown as ChoiceElement<T>;
  const buttons = options.map(([option, label]) => {
    const button = h("button", { type: "button", role: "radio", "data-value": option }, label);
    button.addEventListener("click", () => pick(option));
    button.addEventListener("keydown", (event) => {
      const enabled = buttons.filter((item) => !item.disabled);
      const index = enabled.indexOf(button);
      const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
      if (!step || index < 0) return;
      event.preventDefault();
      const next = enabled[(index + step + enabled.length) % enabled.length];
      next.focus();
      pick(next.dataset.value as T);
    });
    return button;
  });
  root.append(...buttons);
  const draw = () => buttons.forEach((button) => {
    const selected = button.dataset.value === current;
    button.setAttribute("aria-checked", String(selected));
    button.tabIndex = selected ? 0 : -1;
  });
  const pick = (option: T) => {
    if (disabled || option === current) return;
    current = option; draw(); onChange(option);
  };
  draw();
  const choices = options.map(([option, label], index): ChoiceOption => ({
    value: option, label,
    get disabled() { return buttons[index].disabled; },
    set disabled(next: boolean) { buttons[index].disabled = next || disabled; },
  }));
  Object.defineProperties(root, {
    value: { get: () => current, set: (next: T) => { current = next; draw(); } },
    disabled: { get: () => disabled, set: (next: boolean) => { disabled = next; buttons.forEach((button) => { button.disabled = next; }); root.toggleAttribute("aria-disabled", next); } },
    options: { get: () => choices },
  });
  return root;
}

function menuChoice<T extends string>(options: [T, string][], value: T, onChange: (value: T) => void): ChoiceElement<T> {
  let current = value;
  let name = "";
  const disabledOptions = new Set<string>();
  const text = h("span", { class: "menu-button-label" });
  const root = h("button", { type: "button", class: "menu-button choice", "aria-haspopup": "listbox", "aria-expanded": "false" }, text, chevron()) as unknown as ChoiceElement<T>;
  const setAttribute = root.setAttribute.bind(root);
  const draw = () => {
    const label = options.find(([option]) => option === current)?.[1] ?? current;
    text.textContent = label;
    if (name) setAttribute("aria-label", `${name}: ${label}`);
  };
  root.setAttribute = (attribute: string, next: string) => { if (attribute === "aria-label") { name = next; draw(); } else setAttribute(attribute, next); };
  root.addEventListener("click", () => openList(root, options.map(([option, label]) => ({ value: option, label, disabled: disabledOptions.has(option) })), current, (option) => {
    if (option === current) return;
    current = option as T; draw(); onChange(current);
  }));
  root.addEventListener("keydown", (event) => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); root.click(); } });
  draw();
  const choices = options.map(([option, label]): ChoiceOption => ({
    value: option, label,
    get disabled() { return disabledOptions.has(option); },
    set disabled(next: boolean) { if (next) disabledOptions.add(option); else disabledOptions.delete(option); },
  }));
  Object.defineProperties(root, {
    value: { get: () => current, set: (next: T) => { current = next; draw(); } },
    options: { get: () => choices },
  });
  return root;
}

/** A pill button that offers a list of actions in a menu, such as Delen or Provider toevoegen…. */
export function menuButton(label: string, items: { label: string; disabled?: boolean; title?: string | null; run: () => unknown }[], className = ""): HTMLButtonElement {
  const button = h("button", { type: "button", class: `menu-button ${className}`.trim(), "aria-haspopup": "menu", "aria-expanded": "false" }, h("span", { class: "menu-button-label" }, label), chevron());
  button.addEventListener("click", () => openList(button, items.map((item, index) => ({ value: String(index), label: item.label, disabled: !!item.disabled, title: item.title ?? undefined })), null, (index) => { void items[Number(index)].run(); }, "menu"));
  button.addEventListener("keydown", (event) => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); button.click(); } });
  return button;
}

function chevron(): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 12 12"); svg.setAttribute("aria-hidden", "true"); svg.setAttribute("class", "menu-button-chevron");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M3 4.5 6 7.5 9 4.5"); path.setAttribute("fill", "none"); path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.5"); path.setAttribute("stroke-linecap", "round"); path.setAttribute("stroke-linejoin", "round");
  svg.append(path);
  return svg;
}

let openMenu: { close: () => void } | null = null;

function openList(anchor: HTMLElement, items: { value: string; label: string; disabled: boolean; title?: string }[], current: string | null, choose: (value: string) => void, role: "listbox" | "menu" = "listbox") {
  openMenu?.close();
  const itemRole = role === "listbox" ? "option" : "menuitem";
  const list = h("div", { class: "context-menu choice-list", role });
  const close = (focus = true) => {
    list.remove(); anchor.setAttribute("aria-expanded", "false");
    document.removeEventListener("pointerdown", outside, true);
    if (openMenu?.close === closer) openMenu = null;
    if (focus) anchor.focus();
  };
  const closer = () => close(false);
  const outside = (event: Event) => { if (!list.contains(event.target as Node) && event.target !== anchor && !anchor.contains(event.target as Node)) close(false); };
  const entries = items.map((item) => {
    const entry = h("button", { type: "button", role: itemRole, disabled: item.disabled, title: item.title ?? null, "aria-selected": role === "listbox" ? String(item.value === current) : null, class: item.value === current ? "chosen" : null }, item.label);
    entry.addEventListener("click", () => { close(); choose(item.value); });
    return entry;
  });
  list.append(...entries);
  list.addEventListener("keydown", (event) => {
    const enabled = entries.filter((entry) => !entry.disabled);
    const index = enabled.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") { event.preventDefault(); close(); return; }
    if (event.key === "Tab") { close(); return; }
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (event.key === "Home" || event.key === "End") { event.preventDefault(); (event.key === "Home" ? enabled[0] : enabled.at(-1))?.focus(); }
    if (step) { event.preventDefault(); enabled[(index + step + enabled.length) % enabled.length]?.focus(); }
  });
  document.body.append(list);
  const bounds = anchor.getBoundingClientRect();
  const size = list.getBoundingClientRect();
  list.style.minWidth = `${Math.max(bounds.width, 160)}px`;
  list.style.left = `${Math.max(8, Math.min(bounds.left, innerWidth - size.width - 8))}px`;
  const below = bounds.bottom + 4;
  list.style.top = `${below + size.height > innerHeight - 8 ? Math.max(8, bounds.top - size.height - 4) : below}px`;
  anchor.setAttribute("aria-expanded", "true");
  document.addEventListener("pointerdown", outside, true);
  openMenu = { close: closer };
  (entries.find((entry) => entry.classList.contains("chosen") && !entry.disabled) ?? entries.find((entry) => !entry.disabled))?.focus();
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
  const input = h("input", { type: "checkbox", role: "switch", checked, disabled });
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
    else if (bullet) node.append(h("span", { class: "bullet-mark" }, "• "));
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

let dialogNumber = 0;

function namedDialog(title: string, detail: string) {
  const id = `dialog-${++dialogNumber}`;
  const invoker = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const dialog = h("dialog", { class: "editor-dialog", "aria-modal": "true", "aria-labelledby": `${id}-title`, "aria-describedby": detail ? `${id}-detail` : null });
  const heading = h("h2", { id: `${id}-title` }, title);
  const description = detail ? h("p", { id: `${id}-detail`, class: "muted" }, detail) : null;
  const close = () => {
    dialog.close(); dialog.remove();
    if (invoker?.isConnected) invoker.focus({ preventScroll: true });
  };
  return { dialog, heading, description, close };
}

export function confirmDialog(title: string, detail: string, action = "Verwijder"): Promise<boolean> {
  return new Promise((resolve) => {
    const { dialog, heading, description, close } = namedDialog(title, detail);
    const finish = (value: boolean) => { close(); resolve(value); };
    const cancel = h("button", { class: "btn", onclick: () => finish(false) }, "Annuleren");
    dialog.append(heading, ...(description ? [description] : []),
      h("div", { class: "dialog-actions row" }, h("span", { class: "grow" }),
        cancel,
        h("button", { class: "btn danger", onclick: () => finish(true) }, action)));
    dialog.addEventListener("cancel", (event) => { event.preventDefault(); finish(false); });
    document.body.append(dialog);
    dialog.showModal();
    cancel.focus();
  });
}

export function askText(title: string, value: string, detail = ""): Promise<string | null> {
  return new Promise((resolve) => {
    const { dialog, heading, description, close } = namedDialog(title, detail);
    const input = h("input", { type: "text", value, "aria-label": title });
    const finish = (result: string | null) => { close(); resolve(result); };
    const form = h("form", {}, heading, description, input,
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
