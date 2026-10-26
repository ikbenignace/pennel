import "./fonts.css";
import "./pill.css";
import { api, listen, listenToWindow, type PillPreview, type PillState } from "./api";
import { PillPreviewState } from "./pill-preview-state";
import { PILL_METRICS } from "./pill-metrics.generated";
import { bindLocalization, getInterfaceLanguage, localizeBackendText, setInterfaceLanguage, t } from "./i18n.ts";

const pill = document.getElementById("pill")!;
const indicator = document.getElementById("indicator")!;
const label = document.getElementById("label")!;
const preview = document.getElementById("preview")!;
const previewText = document.getElementById("preview-text")!;
const previewState = new PillPreviewState(PILL_METRICS.previewLimit);
let visible = false;
let localeReady = false;
let pendingState: PillState | null = null;
const weights = [0.5, 0.8, 1, 0.75, 0.45];
const bars = weights.map(() => document.createElement("i"));

function stateLabel(state: PillState["state"]): string {
  switch (state) {
    case "listening": return t("dictation.pill.listening");
    case "hands_free": return t("dictation.pill.hands_free");
    case "command": return t("dictation.pill.command");
    case "working": return t("dictation.pill.working");
    default: return "";
  }
}

function setLevel(level: number) {
  const gain = Math.min(1, level * 12);
  bars.forEach((bar, i) => (bar.style.transform = `scaleY(${(4 + 12 * weights[i] * gain) / 16})`));
}

function render(next: PillState) {
  pendingState = next;
  if (!localeReady) return;
  visible = next.state !== "hidden";
  pill.hidden = next.state === "hidden";
  if (!visible) previewState.receive(null);
  label.textContent = next.state === "error" ? localizeBackendText(next.message) : stateLabel(next.state);
  indicator.className = next.state;
  if (["listening", "hands_free", "command"].includes(next.state)) {
    indicator.replaceChildren(...bars);
    setLevel(0);
  } else {
    indicator.replaceChildren();
  }
  renderPreview();
}

function renderPreview() {
  const text = visible ? previewState.snapshot?.text ?? "" : "";
  previewText.textContent = text;
  preview.hidden = !text;
  pill.classList.toggle("has-preview", !!text);
}

async function boot() {
  bindLocalization(label, () => { if (pendingState) label.textContent = pendingState.state === "error" ? localizeBackendText(pendingState.message) : stateLabel(pendingState.state); });
  const locale = api.info().then((info) => {
    setInterfaceLanguage(info.interface_language ?? info.settings.ui_language ?? "en");
  }).catch(() => { setInterfaceLanguage("en"); });
  let eventReceived = false;
  let previewReceived = false;
  let pendingPreview: PillPreview | null | undefined;
  await listenToWindow<PillState>("pill-state", (event) => { eventReceived = true; render(event.payload); });
  await listenToWindow<PillPreview | null>("pill-preview", (event) => {
    pendingPreview = event.payload;
    if (previewState.receive(event.payload)) previewReceived = true;
    renderPreview();
  });
  await listenToWindow<number>("pill-level", (event) => setLevel(event.payload));
  await listen("settings-changed", () => { void api.info().then((info) => setInterfaceLanguage(info.interface_language)).catch(() => {}); });
  await locale;
  document.documentElement.lang = getInterfaceLanguage();
  localeReady = true;
  const snapshot = await api.dictationState();
  if (!eventReceived) render(snapshot);
  else if (pendingState) render(pendingState);
  if (!previewReceived && visible) {
    previewState.restore(snapshot.preview);
    if (pendingPreview !== undefined) previewState.receive(pendingPreview);
    renderPreview();
  }
}

void boot();
