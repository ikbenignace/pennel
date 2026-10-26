import { bindLocalization, l, t, localizeBackendText } from "../i18n.ts";
import { api, listen, type EnrollmentProgress, type ModelState, type SpeakerSetup, type VoiceProfile } from "../api";
import { errorText, formatClock, h } from "../dom";
import { afterPaint } from "../async";
import { combinedProgress, enrollmentButton, enrollmentStatus, enrollmentStep, enrollmentTone, percent, speakerModelsReady, type DownloadProgress, type Enrollment } from "../speaker-state";

export function speakerDownloadLabel() { return l("settings.voice.download_models"); }

/** Downloads the speaker models one after the other as a single download. */
export class SpeakerModels {
  states: Record<string, ModelState | undefined> = {};
  private progress: Record<string, DownloadProgress | undefined> = {};
  private waiting = new Map<string, (error: string | null) => void>();
  private current: string | null = null;
  private cleanup: (() => void)[] = [];
  downloading = false;
  error: string | null = null;

  constructor(readonly ids: string[], private readonly changed: () => void) {}

  async start(active: string[] = []) {
    this.cleanup.push(await listen<{ id: string; downloaded: number; total: number }>("download-progress", (event) => {
      if (!this.ids.includes(event.payload.id)) return;
      this.progress[event.payload.id] = event.payload; this.changed();
    }));
    this.cleanup.push(await listen<{ id: string; error: string | null }>("download-done", (event) => {
      this.waiting.get(event.payload.id)?.(event.payload.error);
    }));
    await this.load();
    if (this.ids.some((id) => active.includes(id))) void this.download();
  }

  async load() {
    const states = await Promise.all(this.ids.map((id) => api.modelState(id)));
    this.ids.forEach((id, index) => { this.states[id] = states[index]; });
    this.changed();
  }

  get ready(): boolean { return speakerModelsReady(this.ids, this.states); }
  get combined(): DownloadProgress { return combinedProgress(this.ids, this.states, this.progress); }

  async download() {
    if (this.downloading || this.ready) return;
    this.downloading = true; this.error = null; this.changed();
    try {
      for (const id of this.ids) {
        if (this.states[id]?.kind === "downloaded") continue;
        this.current = id;
        const done = new Promise<string | null>((resolve) => this.waiting.set(id, resolve));
        // Joining fails only for a download that was already running, which then reports its own result.
        if (!(await api.joinModelDownload(id).catch(() => true))) await api.downloadModel(id);
        const error = await done;
        this.waiting.delete(id);
        this.states[id] = await api.modelState(id);
        if (error) {
          if (!error.toLowerCase().includes("cancel")) this.error = errorText(error);
          break;
        }
      }
    } catch (error) { this.error = errorText(error); }
    finally { this.current = null; this.downloading = false; this.changed(); }
  }

  cancel() { if (this.current) void api.cancelDownload(this.current); }

  dispose() { this.cleanup.forEach((dispose) => dispose()); this.waiting.forEach((resolve) => resolve("cancelled")); }
}

/** Download button, progress and cancel for the speaker models; redraw with `update`. */
export function speakerDownload(models: SpeakerModels) {
  const button = h("button", { class: "btn", onclick: () => { void models.download(); } }, speakerDownloadLabel());
  const progress = h("progress", { class: "engine-download-progress", "aria-label": l("settings.voice.download_label"), max: 1, value: 0 });
  const caption = h("span", {});
  const cancel = h("button", { class: "text-button", onclick: () => models.cancel() }, l("onboarding.cancel_download"));
  const error = h("p", { class: "error", role: "status" });
  const track = h("div", { class: "speaker-download" }, progress, h("div", { class: "engine-download-caption" }, caption, cancel));
  const element = h("div", { class: "speaker-download-block" }, h("div", { class: "row" }, button), track, error);
  const update = () => {
    button.hidden = models.ready || models.downloading;
    track.hidden = !models.downloading;
    const combined = models.combined;
    progress.max = combined.total || 1; progress.value = combined.downloaded;
    caption.textContent = combined.downloaded ? `${percent(combined)}%` : t("onboarding.preparing_download");
    error.textContent = localizeBackendText(models.error ?? ""); error.hidden = !models.error;
    button.textContent = models.error ? t("onboarding.retry") : String(speakerDownloadLabel());
  };
  update();
  bindLocalization(element, update);
  return { element, update };
}

/** The text to read aloud with a record button. Audio stays in memory; only a voiceprint is kept. */
export class EnrollmentPanel {
  readonly element: HTMLElement;
  private state: Enrollment = { phase: "idle" };
  private readonly button = h("button", { class: "btn primary enrollment-record" });
  private readonly status = h("p", { class: "help enrollment-status", role: "status", "aria-live": "polite" });
  private stopListening: (() => void) | null = null;
  private disposed = false;
  private blocked = false;

  constructor(text: string, private readonly finished: (profile: VoiceProfile) => void, private readonly changed: (state: Enrollment) => void = () => {}) {
    this.button.addEventListener("click", () => { void this.toggle(); });
    this.element = h("div", { class: "enrollment" }, h("blockquote", { class: "enrollment-text" }, text), h("div", { class: "row enrollment-controls" }, this.button, this.status));
    this.draw();
    bindLocalization(this.element, () => { if (!this.disposed) this.draw(); });
  }

  get busy(): boolean { return this.state.phase === "starting" || this.state.phase === "recording" || this.state.phase === "working"; }

  private get waiting(): boolean { return this.state.phase === "starting" || this.state.phase === "working"; }

  set disabled(value: boolean) { this.blocked = value; this.button.disabled = value || this.waiting; }

  private apply(state: Enrollment) {
    if (this.disposed) return;
    this.state = state; this.draw(); this.changed(state);
  }

  private draw() {
    this.button.textContent = enrollmentButton(this.state, formatClock);
    this.button.disabled = this.blocked || this.waiting;
    this.button.dataset.phase = this.state.phase;
    this.button.classList.toggle("primary", this.state.phase !== "done");
    this.button.setAttribute("aria-busy", String(this.waiting));
    this.status.textContent = enrollmentStatus(this.state);
    const tone = enrollmentTone(this.state);
    this.status.classList.toggle("error", tone === "error");
    this.status.classList.toggle("success", tone === "success");
  }

  private async toggle() {
    if (this.state.phase === "recording") return this.finish();
    if (this.waiting) return;
    this.apply(enrollmentStep(this.state, { kind: "request" }));
    // Opening and closing the microphone runs on the app's main thread; paint the new state first.
    await afterPaint();
    try {
      this.stopListening ??= await listen<EnrollmentProgress>("voice-enrollment", (event) => {
        if (this.state.phase !== "recording") return;
        if (event.payload.stopped) void this.finish();
        else this.apply(enrollmentStep(this.state, { kind: "tick", seconds: event.payload.seconds }));
      });
      await api.startVoiceEnrollment();
      if (this.disposed) { void api.cancelVoiceEnrollment(); return; }
      this.apply(enrollmentStep(this.state, { kind: "start" }));
    } catch (error) { this.apply(enrollmentStep(this.state, { kind: "failed", message: errorText(error) })); }
  }

  private async finish() {
    if (this.state.phase !== "recording") return;
    this.apply(enrollmentStep(this.state, { kind: "finish" }));
    await afterPaint();
    try {
      const profile = await api.finishVoiceEnrollment();
      this.apply(enrollmentStep(this.state, { kind: "succeeded", profile }));
      if (!this.disposed) this.finished(profile);
    } catch (error) { this.apply(enrollmentStep(this.state, { kind: "failed", message: errorText(error) })); }
  }

  dispose() {
    if (this.state.phase === "recording") void api.cancelVoiceEnrollment();
    this.disposed = true;
    this.stopListening?.();
  }
}

/** Why the user's voice cannot be learned right now, or null. */
export function enrollmentBlocker(setup: SpeakerSetup, mode: string, models: SpeakerModels | null): string | null {
  if (mode === "cloud") return setup.signed_in ? null : t("backend.sign_in_to_pennel_cloud_under_settings_account_first");
  if (!setup.local_available) return t("settings.voice.unavailable");
  return models?.ready ? null : t("settings.voice.download_first");
}

export function enrollmentIntro() { return l("settings.voice.intro"); }

export function enrollmentDialog(text: string): Promise<VoiceProfile | null> {
  return new Promise((resolve) => {
    const invoker = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = h("dialog", { class: "editor-dialog enrollment-dialog", "aria-modal": "true", "aria-labelledby": "enrollment-title" });
    let learned: VoiceProfile | null = null;
    const done = h("button", { class: "btn primary", disabled: true, onclick: () => close() }, l("onboarding.done"));
    const cancel = h("button", { class: "btn", onclick: () => close() }, l("common.cancel"));
    const panel = new EnrollmentPanel(text, (profile) => { learned = profile; done.disabled = false; }, () => { cancel.disabled = panel.busy; });
    const close = () => {
      panel.dispose(); dialog.close(); dialog.remove();
      if (invoker?.isConnected) invoker.focus({ preventScroll: true });
      resolve(learned);
    };
    dialog.append(h("h2", { id: "enrollment-title" }, l("settings.voice.learn")), h("p", { class: "muted" }, enrollmentIntro()), panel.element,
      h("div", { class: "dialog-actions row" }, h("span", { class: "grow" }), cancel, done));
    dialog.addEventListener("cancel", (event) => { event.preventDefault(); if (!panel.busy) close(); });
    document.body.append(dialog);
    dialog.showModal();
  });
}
