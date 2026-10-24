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
  return h(
    "section",
    { class: "pane-section" },
    h("h3", {}, title),
    subtitle ? h("p", { class: "subtitle" }, subtitle) : null,
    ...body,
  );
}

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
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
