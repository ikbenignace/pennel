import "./fonts.css";
import "./pill.css";
import { api, listenToWindow, type PillPreview, type PillState } from "./api";
import { PillPreviewState } from "./pill-preview-state";
import { PILL_METRICS } from "./pill-metrics.generated";

const pill = document.getElementById("pill")!;
const indicator = document.getElementById("indicator")!;
const label = document.getElementById("label")!;
const preview = document.getElementById("preview")!;
const previewText = document.getElementById("preview-text")!;
const previewState = new PillPreviewState(PILL_METRICS.previewLimit);
let visible = false;
const weights = [0.5, 0.8, 1, 0.75, 0.45];
const bars = weights.map(() => document.createElement("i"));

const labels: Record<string, string> = {
  listening: "Luistert…",
  hands_free: "Handenvrij — tik om te stoppen",
  command: "Opdracht…",
  working: "Bezig…",
};

function setLevel(level: number) {
  const gain = Math.min(1, level * 12);
  bars.forEach((bar, i) => (bar.style.transform = `scaleY(${(4 + 12 * weights[i] * gain) / 16})`));
}

function render(next: PillState) {
  visible = next.state !== "hidden";
  pill.hidden = next.state === "hidden";
  if (!visible) previewState.receive(null);
  label.textContent = next.state === "error" ? next.message : (labels[next.state] ?? "");
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
  const snapshot = await api.dictationState();
  if (!eventReceived) render(snapshot);
  if (!previewReceived && visible) {
    previewState.restore(snapshot.preview);
    if (pendingPreview !== undefined) previewState.receive(pendingPreview);
    renderPreview();
  }
}

void boot();
