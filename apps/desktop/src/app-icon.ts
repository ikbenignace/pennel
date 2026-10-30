import { api, type AppIdentity } from "./api";
import { h } from "./dom";
import { icon as uiIcon } from "./icons";
import { applicationLabel } from "./presentation";

const identities = new Map<string, Promise<AppIdentity | null>>();

function identity(id: string): Promise<AppIdentity | null> {
  let pending = identities.get(id);
  if (!pending) {
    pending = api.appIdentity(id).catch(() => null);
    identities.set(id, pending);
  }
  return pending;
}

/** Real application icon and name when the platform can resolve them; a generic application symbol otherwise. */
export function appBadge(id: string): { icon: HTMLElement; name: HTMLSpanElement } {
  const icon = h("span", { class: "app-icon", "aria-hidden": "true" }, uiIcon("laptop"));
  const name = h("span", { class: "app-name" }, applicationLabel(id));
  void identity(id).then((found) => {
    if (!found) return;
    icon.replaceChildren(h("img", { src: found.icon, alt: "" }));
    icon.classList.add("resolved");
    if (found.name.trim()) name.textContent = found.name;
  });
  return { icon, name };
}
