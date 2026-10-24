import type { HotkeyTrigger, Style } from "../api";
import { h, section, select } from "../dom";
import type { Context } from "../main";

const hotkeys: [HotkeyTrigger, string][] = [
  ["right_control", "Rechter Ctrl"],
  ["right_option", "Rechter Alt"],
  ["control_super", "Ctrl + Windows"],
];

const styles: [Style, string][] = [
  ["neutral", "Neutraal"],
  ["casual", "Los"],
  ["formal", "Netjes"],
  ["literal", "Letterlijk"],
];

const appNames: Record<string, string> = {
  "com.tinyspeck.slackmacgap": "Slack",
  "net.whatsapp.WhatsApp": "WhatsApp",
  "com.apple.MobileSMS": "Berichten",
  "ru.keepcoder.Telegram": "Telegram",
  "com.apple.mail": "Mail / Thunderbird",
  "com.microsoft.Outlook": "Outlook",
  "com.microsoft.Word": "Word",
  "com.anthropic.claudefordesktop": "Claude",
  "com.openai.chat": "ChatGPT",
  "com.apple.Terminal": "Terminal",
  "com.googlecode.iterm2": "iTerm",
  "com.mitchellh.ghostty": "Ghostty",
  "com.microsoft.VSCode": "Visual Studio Code",
  "dev.zed.Zed": "Zed",
  "com.todesktop.230313mzl4w4u92": "Cursor",
};

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
    {},
    h("h1", {}, "Dicteren"),
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
        hotkeys.map(([value, label]) =>
          h(
            "button",
            { "aria-pressed": String(value === s.hotkey), onclick: () => ctx.save((n) => (n.hotkey = value)) },
            label,
          ),
        ),
      ),
      linux
        ? h(
            "p",
            { class: "subtitle", style: "margin-top:10px" },
            "Op Wayland ziet Pennel de sneltoets alleen in X11-apps (XWayland). Zie de documentatie.",
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
    {},
    h("h1", {}, "Woordenlijst"),
    section(
      "Woordenlijst",
      "Namen en vaktermen die Pennel exact zo moet schrijven.",
      h("div", { class: "row" }, input, h("button", { class: "btn", onclick: add }, "Toevoegen")),
      h(
        "div",
        { class: "chips" },
        terms.map((term) =>
          h(
            "span",
            { class: "chip" },
            term,
            h(
              "button",
              { title: "Verwijder", onclick: () => ctx.save((n) => (n.dictionary = n.dictionary.filter((t) => t !== term))) },
              "×",
            ),
          ),
        ),
      ),
    ),
  );
}

export function stylesView(ctx: Context): HTMLElement {
  const rules = ctx.info.settings.style_rules;
  const input = h("input", { type: "text", placeholder: "Programmanaam, bv. notepad", class: "grow" });
  const add = () => {
    const id = input.value.trim().toLowerCase().replace(/\.exe$/, "");
    if (!id || rules.some((r) => r.bundle_id === id)) return;
    ctx.save((n) => n.style_rules.push({ bundle_id: id, style: "neutral" }));
  };
  return h(
    "div",
    {},
    h("h1", {}, "Stijlen"),
    section(
      "Stijl per app",
      "Andere apps gebruiken Neutraal. Pennel kijkt enkel naar welke app actief is, nooit naar je scherm.",
      rules.map((rule) =>
        h(
          "div",
          { class: "row", style: "margin:6px 0" },
          h("span", { class: "grow" }, appNames[rule.bundle_id] ?? rule.bundle_id),
          select(styles, rule.style, (style) =>
            ctx.save((n) => (n.style_rules.find((r) => r.bundle_id === rule.bundle_id)!.style = style)),
          ),
          h(
            "button",
            {
              class: "btn",
              title: "Verwijder",
              onclick: () => ctx.save((n) => (n.style_rules = n.style_rules.filter((r) => r.bundle_id !== rule.bundle_id))),
            },
            "×",
          ),
        ),
      ),
      h("div", { class: "row", style: "margin-top:10px" }, input, h("button", { class: "btn", onclick: add }, "App toevoegen")),
      ctx.info.platform === "linux"
        ? h("p", { class: "subtitle", style: "margin-top:10px" }, "Op Linux herkent Pennel de actieve app nog niet; alles gebruikt Neutraal.")
        : null,
    ),
  );
}
