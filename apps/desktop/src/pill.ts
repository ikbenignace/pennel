import "./fonts.css";
import "./pill.css";
import { listen } from "@tauri-apps/api/event";
import { api, type PillState } from "./api";


const pill = document.getElementById("pill")!;
const indicator = document.getElementById("indicator")!;
const label = document.getElementById("label")!;
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
  bars.forEach((bar, i) => (bar.style.height = `${4 + 12 * weights[i] * gain}px`));
}

function render(next: PillState) {
  pill.hidden = next.state === "hidden";
  label.textContent = next.state === "error" ? next.message : (labels[next.state] ?? "");
  indicator.className = next.state;
  if (["listening", "hands_free", "command"].includes(next.state)) {
    indicator.replaceChildren(...bars);
    setLevel(0);
  } else {
    indicator.replaceChildren();
  }
}

async function boot() {
  let eventReceived = false;
  await listen<PillState>("pill-state", (event) => { eventReceived = true; render(event.payload); });
  await listen<number>("pill-level", (event) => setLevel(event.payload));
  const snapshot = await api.dictationState();
  if (!eventReceived) render(snapshot);
}

void boot();
