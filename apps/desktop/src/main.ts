import "./styles.css";
import { listen } from "@tauri-apps/api/event";
import { api, type AppInfo, type Settings } from "./api";
import { h } from "./dom";
import { dictationView, dictionaryView, stylesView } from "./views/dictation";
import { historyView } from "./views/history";
import { localView } from "./views/local";
import { meetingView, meetingsView } from "./views/meetings";
import { modelsView } from "./views/models";
import { providersView } from "./views/providers";

export interface Context {
  info: AppInfo;
  /** Saves settings through the core (which normalizes them) and re-renders. */
  save: (change: (settings: Settings) => void) => Promise<void>;
  update: (info: AppInfo) => void;
  rerender: () => void;
  navigate: (tab: string, param?: string) => void;
  param?: string;
}

type View = (ctx: Context) => HTMLElement | Promise<HTMLElement>;

const tabs: [string, string, View][] = [
  ["meeting", "Opnemen", meetingView],
  ["meetings", "Meetings", meetingsView],
  ["history", "Geschiedenis", historyView],
  ["providers", "Providers", providersView],
  ["models", "Modellen", modelsView],
  ["local", "Lokale modellen", localView],
  ["dictation", "Dicteren", dictationView],
  ["dictionary", "Woordenlijst", dictionaryView],
  ["styles", "Stijlen", stylesView],
];

let info: AppInfo;
let current = "providers";
let param: string | undefined;
let generation = 0;
const content = h("main", {});
const navButtons = new Map<string, HTMLButtonElement>();

function navigate(tab: string, p?: string) {
  current = tabs.some(([id]) => id === tab) ? tab : "providers";
  param = p;
  render();
}

async function render() {
  const mine = ++generation;
  for (const [id, button] of navButtons) {
    if (id === current) button.setAttribute("aria-current", "page");
    else button.removeAttribute("aria-current");
  }
  const view = tabs.find(([id]) => id === current)![2];
  const ctx: Context = {
    info,
    save: async (change) => {
      const next = structuredClone(info.settings);
      change(next);
      try {
        info = await api.saveSettings(next);
      } catch (e) {
        alert(`Instellingen bewaren mislukt: ${e}`);
      }
      render();
    },
    update: (next) => {
      info = next;
      render();
    },
    rerender: render,
    navigate,
    param,
  };
  const node = await view(ctx);
  if (mine !== generation) return;
  const scroll = content.scrollTop;
  content.replaceChildren(node);
  content.scrollTop = scroll;
}

async function boot() {
  info = await api.info();
  const nav = h(
    "nav",
    {},
    h("div", { class: "wordmark" }, "pennel", h("span", { class: "dot" }, ".")),
    tabs.map(([id, label]) => {
      const button = h("button", { onclick: () => navigate(id) }, label);
      navButtons.set(id, button);
      return button;
    }),
    h("div", { class: "spacer" }),
    h("button", { onclick: () => api.openLog() }, "Logboek openen"),
    h("div", { class: "version" }, `Versie ${info.version}`),
  );
  document.getElementById("app")!.replaceChildren(nav, content);
  await listen<string>("navigate", (event) => navigate(event.payload));
  await listen("history-changed", () => {
    if (current === "history") render();
  });
  render();
}

boot();
