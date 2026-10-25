import { api, type HotkeyTrigger, type Style } from "../api";
import { errorText, h, section, select, toggle } from "../dom";
import type { Context } from "../main";
import { icon } from "../icons";
import { applicationLabel } from "../presentation";

function hotkeys(platform: string): [HotkeyTrigger, string][] {
  return platform === "macos" ? [["fn", "fn"], ["right_option", "Rechter Option"], ["right_command", "Rechter Command"], ["right_control", "Rechter Ctrl"]]
    : [["right_control", "Rechter Ctrl"], ["right_option", "Rechter Alt"], ["control_super", platform === "windows" ? "Ctrl + Windows" : "Ctrl + Super"]];
}

const styles: [Style, string][] = [
  ["neutral", "Neutraal"],
  ["casual", "Los"],
  ["formal", "Netjes"],
  ["literal", "Letterlijk"],
];

function radio(name: string, value: string, current: string, label: string, onPick: () => void) {
  return h(
    "label",
    { class: "radio" },
    h("input", { type: "radio", name, value, checked: value === current, onchange: onPick }),
    label,
  );
}

export function dictationView(ctx: Context): HTMLElement {
  const s = ctx.info.settings;
  const linux = ctx.info.platform === "linux";
  return h(
    "div",
    { class: "dictation-settings" },
    section(
      "Stand",
      null,
      radio("mode", "clean", s.mode, "Opgekuist — euh's weg, zelfcorrecties verwerkt, stijl per app", () =>
        ctx.save((n) => (n.mode = "clean")),
      ),
      radio("mode", "fast", s.mode, "Snel — letterlijk wat je zei, zonder taalmodel", () => ctx.save((n) => (n.mode = "fast"))),
    ),
    section(
      "Sneltoets",
      "Ingedrukt houden om te praten, twee keer tikken voor handenvrij. Houd Shift erbij voor een opdracht over de geselecteerde tekst.",
      h(
        "div",
        { class: "segmented" },
        hotkeys(ctx.info.platform).map(([value, label]) =>
          h(
            "button",
            { "aria-pressed": String(value === s.hotkey), onclick: () => ctx.save((n) => (n.hotkey = value)) },
            label,
          ),
        ),
      ),
      linux && ctx.info.capabilities.global_hotkey.reason
        ? h(
            "p",
            { class: "subtitle", style: "margin-top:10px" },
            ctx.info.capabilities.global_hotkey.reason,
          )
        : null,
      ctx.info.hotkey_error ? h("p", { class: "error" }, ctx.info.hotkey_error) : null,
    ),
    section(
      "Taal",
      null,
      select<string>(
        [
          ["", "Automatisch"],
          ["nl", "Nederlands"],
          ["en", "Engels"],
          ["fr", "Frans"],
          ["de", "Duits"],
        ],
        s.language ?? "",
        (v) => ctx.save((n) => (n.language = v || null)),
      ),
    ),
    section(
      "Geschiedenis bewaren",
      null,
      select<string>(
        [
          ["7", "7 dagen"],
          ["30", "30 dagen"],
          ["90", "90 dagen"],
          ["0", "Altijd"],
        ],
        String(s.history_retention_days),
        (v) => ctx.save((n) => (n.history_retention_days = Number(v))),
      ),
    ),
    section("Dicteerpad", "Deze instelling geldt alleen voor het dicteerpad in Pennel. Fn-dicteren vanuit andere apps blijft direct invoegen.",
      toggle("Dicteerpad ook extern plakken", s.dictation_pad_auto_paste, (value) => { void ctx.save((settings) => { settings.dictation_pad_auto_paste = value; }); }, ctx.info.capabilities.pad_external_paste),
      h("p", { class: "help" }, "De tekst blijft in Pennel en wordt ook in de vastgelegde vorige app geplaatst.")),
    section("Toegang", "Controleer microfoon en toegang om tekst in andere apps te plakken.",
      h("button", { class: "btn", onclick: () => api.showWindow("onboarding", "resume") }, "Controleer toegang…")),
  );
}

export function dictionaryView(ctx: Context): HTMLElement {
  const terms = ctx.info.settings.dictionary;
  const input = h("input", { type: "text", placeholder: "Bv. Pennel, Kubernetes, Sarah", class: "grow" });
  const add = () => {
    const term = input.value.trim();
    if (!term || terms.includes(term)) return;
    ctx.save((n) => n.dictionary.push(term));
  };
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") add();
  });
  return h(
    "div",
    { class: "dictionary-settings" },
    section(
      "Woordenlijst",
      "Namen en vaktermen die Pennel exact zo moet schrijven.",
      h("div", { class: "row" }, input, h("button", { class: "btn", onclick: add }, "Toevoegen")),
      h(
        "div",
        { class: "dictionary-list" },
        terms.map((term) =>
          h(
            "div",
            { class: "dictionary-row" },
            h("span", { class: "grow" }, term),
            h(
              "button",
              { class: "button-icon", "aria-label": `${term} verwijderen`, onclick: () => ctx.save((n) => (n.dictionary = n.dictionary.filter((t) => t !== term))) },
              icon("close"),
            ),
          ),
        ),
      ),
    ),
  );
}

export function stylesView(ctx: Context): HTMLElement {
  const rules = ctx.info.settings.style_rules;
  const input = h("input", { type: "text", placeholder: ctx.info.platform === "macos" ? "App-id, bv. com.apple.TextEdit" : "Programmanaam, bv. notepad", class: "grow" });
  const add = () => {
    const value = input.value.trim();
    const id = ctx.info.platform === "macos" || (value.includes(".") && !/\.exe$/i.test(value)) ? value : value.toLowerCase().replace(/\.exe$/, "");
    if (!id || rules.some((r) => r.bundle_id === id)) return;
    void ctx.save((n) => n.style_rules.push({ bundle_id: id, style: "neutral" }));
  };
  input.addEventListener("keydown", (event) => { if (event.key === "Enter") add(); });
  const pickerStatus = h("p", { class: "help", role: "status" });
  const unavailable = ctx.info.platform === "linux" && ctx.info.capabilities.selection.status === "unsupported";
  const pick = h("button", { class: "btn", disabled: unavailable, onclick: async () => {
    pick.disabled = true;
    try {
      const application = await api.pickStyleApplication();
      if (application && !ctx.info.settings.style_rules.some((rule) => rule.bundle_id === application.id)) {
        await ctx.save((settings) => { settings.style_rules.push({ bundle_id: application.id, style: "neutral" }); });
      }
    } catch (error) { pickerStatus.textContent = errorText(error); }
    finally { pick.disabled = unavailable; }
  } }, "App toevoegen…");
  return h(
    "div",
    { class: "style-settings" },
    section(
      "Stijl per app",
      "Andere apps gebruiken Neutraal. Pennel kijkt enkel naar welke app actief is, nooit naar je scherm.",
      rules.map((rule) =>
        h(
          "div",
          { class: "row style-rule" },
          icon("laptop"),
          h("span", { class: "grow" }, applicationLabel(rule.bundle_id)),
          select(styles, rule.style, (style) =>
            ctx.save((n) => { const item = n.style_rules.find((r) => r.bundle_id === rule.bundle_id); if (item) item.style = style; }),
          ),
          h(
            "button",
            {
              class: "button-icon",
              "aria-label": `Stijl voor ${applicationLabel(rule.bundle_id)} verwijderen`,
              onclick: () => ctx.save((n) => (n.style_rules = n.style_rules.filter((r) => r.bundle_id !== rule.bundle_id))),
            },
            icon("close"),
          ),
        ),
      ),
      h("div", { class: "row style-add" }, pick), pickerStatus,
      h("details", { class: "advanced-style" }, h("summary", {}, "App-ID handmatig toevoegen"),
        h("div", { class: "row" }, input, h("button", { class: "btn", disabled: unavailable, onclick: add }, "Toevoegen"))),
      unavailable ? h("p", { class: "help" }, "Op Wayland is de actieve app niet beschikbaar. Appstijlen worden daar niet toegepast.") : null,
    ),
  );
}
