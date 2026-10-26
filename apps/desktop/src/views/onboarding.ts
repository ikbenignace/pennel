import { api, fixtureClient, listen, type DictationSessionSnapshot, type KindInfo, type LocalModel, type OnboardingStep, type PermissionInfo, type ProviderAccount, type ProviderKind, type SetupReadiness } from "../api";
import { errorCode, errorText, formatBytes, h, select } from "../dom";
import { OwnedDictationSession } from "../dictation-session";
import { icon } from "../icons";
import { defaultModelId, initialEngine, manualPracticeAllowed, modelOptionLabel, onboardingProgressChange, onboardingSteps, resumeStep, stepIndex, type Engine } from "../onboarding-state";
import { trainedLabel } from "../speaker-state";
import { EnrollmentPanel, SpeakerModels, enrollmentBlocker, enrollmentIntro, speakerDownload } from "./voices";
import { hotkeyLabel, hotkeyKeyLabel } from "../presentation";
import { workspaceRoute } from "../workspace-state";
import type { Context } from "../main";
import wordmarkSource from "../../../../assets/brand/wordmark.svg?raw";
import { VisibilityPoll } from "../window-lifecycle";

const languages: [string, string][] = [["", "Automatisch"], ["nl", "Nederlands"], ["en", "Engels"], ["fr", "Frans"], ["de", "Duits"]];
const phrases: Record<string, string> = { nl: "Zullen we morgen om tien uur afspreken?", en: "Shall we meet tomorrow at ten?", fr: "On se retrouve demain à dix heures ?", de: "Treffen wir uns morgen um zehn Uhr?" };

/** The wordmark follows the text colour so it stays legible in dark mode; the full stop keeps the brand orange. */
function wordmark(): HTMLElement {
  const node = h("div", { class: "onboarding-wordmark", role: "img", "aria-label": "Pennel" });
  node.innerHTML = wordmarkSource.replace(/stroke="#2A2723"/gi, 'stroke="currentColor"').replace(/fill="#C2410C"/gi, 'fill="var(--accent)"');
  node.querySelector("svg")?.setAttribute("aria-hidden", "true");
  return node;
}

class OnboardingWizard {
  readonly root = h("div", { class: "onboarding-shell" });
  private readonly body = h("div", { class: "onboarding-body" });
  private readonly progress = h("div", { class: "onboarding-progress", role: "img" });
  private readonly next = h("button", { class: "btn primary onboarding-next", onclick: () => { void this.advance().catch(this.fail); } });
  private readonly back = h("button", { class: "text-button", onclick: () => { void this.move(Math.max(0, this.index - 1)).catch(this.fail); } }, "Terug");
  private readonly skip = h("button", { class: "text-button", onclick: () => { void this.leave().catch(this.fail); } });
  private readonly count = h("span", { class: "help onboarding-count" });
  private readonly theme = h("button", { class: "button-icon", "aria-label": "Weergave wijzigen", onclick: () => {
    const dark = this.ctx.info.settings.appearance === "dark" || (this.ctx.info.settings.appearance === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
    void this.ctx.save((settings) => { settings.appearance = dark ? "light" : "dark"; }).catch(this.fail);
  } });
  private readonly message = h("p", { class: "onboarding-message error", role: "status", "aria-live": "polite", hidden: true });
  private readonly readyRow = h("div", { class: "onboarding-permissions" });
  private readonly engineDetail = h("section", { class: "onboarding-engine-detail", id: "onboarding-engine-detail" });
  private readonly localButtons = new Map<Engine, HTMLButtonElement>();
  private index: number;
  private engine: Engine;
  private language: string;
  private localModelId: string;
  private providerChoice: string;
  private providerModel: string;
  private providerBaseUrl: string;
  private models: LocalModel[] = [];
  private permissions: PermissionInfo[] = [];
  private readiness: SetupReadiness | null = null;
  private downloading = false;
  private readonly attachedDownloads = new Set<string>();
  private downloadProgress = { downloaded: 0, total: 0 };
  private downloadError: string | null = null;
  private actionBusy = false;
  private disposed = false;
  private renderGeneration = 0;
  private modelsGeneration = 0;
  private permissionSignature = "";
  private poll: VisibilityPoll | undefined;
  private readonly cleanup: (() => void)[] = [];
  private practice: OwnedDictationSession | null = null;
  private practiceDone = false;
  private practiceStart: Promise<void> = Promise.resolve();
  private held = false;
  private practiceText: HTMLTextAreaElement | null = null;
  private practiceStatus: HTMLParagraphElement | null = null;
  private practiceButton: HTMLButtonElement | null = null;
  private practiceKey: HTMLButtonElement | null = null;
  private fresh: boolean;
  private voicePanel: EnrollmentPanel | null = null;
  private voiceModels: SpeakerModels | null = null;

  constructor(private readonly ctx: Context) {
    const settings = ctx.info.settings;
    const account = settings.providers.find((provider) => provider.id === settings.dictation_stt.provider_id);
    this.engine = initialEngine(account?.kind, ctx.info.kinds.find((kind) => kind.kind === "local")?.availability.status !== "unsupported");
    this.language = settings.language ?? "";
    this.localModelId = account?.kind === "local" ? settings.dictation_stt.model : ctx.info.kinds.find((kind) => kind.kind === "local")?.default_stt_model ?? "";
    this.providerChoice = account && this.customAccounts().some((item) => item.id === account.id) ? account.id
      : this.customAccounts()[0]?.id ?? `new:${this.customKinds()[0]?.kind ?? "open_ai"}`;
    const providerKind = this.customKind();
    this.providerModel = this.engine === "custom" ? settings.dictation_stt.model : providerKind?.default_stt_model ?? "";
    this.providerBaseUrl = this.customAccount()?.base_url ?? providerKind?.default_base_url ?? "";
    this.index = stepIndex(settings.onboarding?.resume_step);
    this.fresh = settings.onboarding?.status === "pending";
    const requested = new URLSearchParams(location.search).get("tab");
    if (requested === "welcome") { this.index = 0; this.fresh = false; }
    const fixture = fixtureClient()?.fixture?.name ?? "";
    if (fixture.includes("engine-apple")) this.engine = "apple";
    if (fixture.includes("engine-custom")) this.engine = "custom";
    if (fixture.includes("download-error")) this.downloadError = "Het model kon niet worden gedownload. Controleer je verbinding en probeer opnieuw.";
    this.progress.append(...onboardingSteps.map(() => h("span")));
    this.root.append(this.progress,
      this.body, this.message, h("footer", { class: "onboarding-footer" }, h("div", { class: "onboarding-footer-left" }, this.theme, this.back, this.skip, this.count), this.next));
    ctx.onInfoChange(() => { if (ctx.info.settings.onboarding.status !== "pending") this.fresh = false; this.updateCommon(); if (this.index === 2) void this.refreshPermissions().catch(this.fail); });
    ctx.onFocusChange((focused) => {
      if (this.index === 2 && focused) void this.refreshPermissions().catch(this.fail);
      if (this.index === 3) {
        if (!focused) { if (this.practiceText) this.practiceText.disabled = true; void this.practice?.close().catch(this.fail); }
        else void this.practice?.open().catch(this.fail);
      }
    });
    ctx.onVisibilityChange((visible) => {
      this.poll?.setVisible(visible);
      if (visible && this.index === 3) {
        if (this.practice) void this.practice.open().catch(this.fail);
        else void this.renderStep().catch(this.fail);
      }
    });
    ctx.beforeLeave(async () => { this.fresh = false; await this.closePractice(); });
    ctx.onFlush(() => this.persist(onboardingSteps[this.index]));
    ctx.onDispose(() => this.dispose());
  }

  private readonly fail = (error: unknown) => {
    if (this.disposed) return;
    this.message.textContent = errorText(error); this.message.hidden = false;
    this.actionBusy = false; this.updateCommon();
  };

  async start(): Promise<void> {
    this.cleanup.push(await listen<{ id: string; downloaded: number; total: number }>("download-progress", (event) => {
      if (event.payload.id !== this.localModelId || this.disposed) return;
      this.downloading = true; this.downloadProgress = event.payload; this.updateDownload(); this.updateCommon();
    }));
    this.cleanup.push(await listen<{ id: string; error: string | null }>("download-done", (event) => {
      this.attachedDownloads.delete(event.payload.id);
      if (event.payload.id !== this.localModelId || this.disposed) return;
      this.downloading = false;
      this.downloadError = event.payload.error && !event.payload.error.toLowerCase().includes("cancel") ? event.payload.error : null;
      void this.ctx.refresh().then(() => this.loadModels()).then(() => { if (this.index === 1 && this.engine === "local") this.renderEngineDetail(); this.updateCommon(); }).catch(this.fail);
    }));
    const route = (event: Event) => {
      const step = (event as CustomEvent<unknown>).detail;
      if (step === "resume") { this.fresh = false; void this.resume().catch(this.fail); }
      else if (onboardingSteps.includes(step as OnboardingStep)) { if (step === "welcome") this.fresh = false; void this.move(stepIndex(step)).catch(this.fail); }
    };
    window.addEventListener("pennel-onboarding-route", route);
    this.cleanup.push(() => window.removeEventListener("pennel-onboarding-route", route));
    if (new URLSearchParams(location.search).get("tab") === "resume") await this.resume(); else await this.renderStep();
  }

  private async resume() {
    const readiness = await api.onboardingReadiness("practice");
    this.readiness = readiness;
    await this.move(stepIndex(resumeStep(readiness.resume_step, this.ctx.info.settings.onboarding.resume_step)));
  }

  private async persist(step: OnboardingStep, status?: "skipped" | "completed") {
    if (!onboardingProgressChange(this.ctx.info.settings.onboarding, step, status)) return;
    await this.ctx.save((settings) => {
      const next = onboardingProgressChange(settings.onboarding, step, status);
      if (next) settings.onboarding = next;
    });
  }

  private async move(index: number) {
    if (this.actionBusy || this.disposed) return;
    await this.closePractice(); this.closeVoice();
    await this.persist(onboardingSteps[index]);
    this.index = index; this.message.hidden = true;
    await this.renderStep();
  }

  private async closePractice() {
    this.held = false;
    if (this.practiceText) this.practiceText.disabled = true;
    await this.practice?.close(); this.practice = null;
  }

  private closeVoice() {
    this.voicePanel?.dispose(); this.voicePanel = null;
    this.voiceModels?.dispose(); this.voiceModels = null;
  }

  private async leave() {
    this.fresh = false;
    this.actionBusy = true; this.updateCommon();
    // The voice step is optional, so skipping it still completes the onboarding.
    const status = onboardingSteps[this.index] === "voice" ? "completed" : "skipped";
    try { await this.closePractice(); this.closeVoice(); await this.persist(onboardingSteps[this.index], status); await api.showWorkspace(workspaceRoute("dictation")); }
    finally { this.actionBusy = false; this.updateCommon(); }
  }

  private async renderStep() {
    const generation = ++this.renderGeneration;
    this.poll?.dispose(); this.poll = undefined;
    this.root.dataset.step = onboardingSteps[this.index];
    this.body.className = `onboarding-body onboarding-${onboardingSteps[this.index]}`;
    if (this.index === 0) this.renderWelcome();
    if (this.index === 1) { await this.loadModels(); if (generation !== this.renderGeneration || this.disposed) return; this.renderEngine(); }
    if (this.index === 2) {
      this.body.replaceChildren(this.title("Luisteren. En tekst plaatsen.", "Zo kan Pennel ook in je andere apps werken."), this.readyRow,
        h("p", { class: "help onboarding-permission-help" }, "Agenda en systeemaudio vragen we pas wanneer je meetings wilt opnemen."));
      this.permissionSignature = ""; await this.refreshPermissions();
      this.poll = new VisibilityPoll(1500, () => this.refreshPermissions(), this.fail);
      this.poll.setVisible(this.ctx.visible, false);
    }
    if (this.index === 3) await this.renderPractice();
    if (this.index === 4) { await this.renderVoice(generation); if (generation !== this.renderGeneration || this.disposed) return; }
    this.updateCommon();
  }

  private title(text: string, description: string) {
    return h("div", { class: "onboarding-heading" }, h("h1", {}, text), h("p", { class: "muted" }, description));
  }

  private hotkey(): string {
    return hotkeyLabel(this.ctx.info.settings.hotkey, this.ctx.info.platform);
  }

  private keycap(): HTMLButtonElement {
    return h("button", { class: "keycap", type: "button", "aria-label": "Houd ingedrukt om te dicteren" }, icon("globe"), h("span", {}, this.keyLabel()));
  }

  private keyLabel(): string {
    return hotkeyKeyLabel(this.ctx.info.settings.hotkey, this.ctx.info.platform);
  }

  private renderWelcome() {
    this.body.replaceChildren(wordmark(),
      h("h1", {}, "Welkom bij Pennel"), h("p", { class: "onboarding-intro" }, "Spreek. Pennel schrijft waar je cursor staat."),
      h("div", { class: "onboarding-demo" }, h("div", { class: "keycap", "aria-hidden": "true" }, icon("globe"), h("span", {}, this.keyLabel())),
        h("div", { class: "onboarding-demo-document" }, h("span", { class: "help" }, "Je volgende bericht"), h("p", {}, phrases.nl, h("span", { class: "typing-caret", "aria-hidden": "true" })))),
      h("p", { class: "help" }, `Hou ${this.hotkey()} in. Praat. Laat los.`),
      h("p", { class: "help onboarding-welcome-note" }, "Dicteren en meetingnotities. Zonder verplicht account."));
  }

  private async loadModels() {
    const generation = ++this.modelsGeneration;
    const models = await api.localModels(this.language || null, false);
    if (this.disposed || generation !== this.modelsGeneration) return;
    this.models = models;
    this.localModelId = defaultModelId(models, this.localModelId);
    await this.attachDownload();
  }

  private async attachDownload() {
    const id = this.localModelId;
    if (!this.ctx.info.active_model_downloads?.includes(id) || this.attachedDownloads.has(id)) return;
    this.attachedDownloads.add(id);
    const model = this.localModel();
    this.downloading = true;
    this.downloadProgress = { downloaded: model?.state.kind === "partial" ? model.state.bytes : 0, total: model?.size_bytes ?? 0 };
    void api.joinModelDownload(id).then(async (joined) => {
      if (this.disposed || joined) return;
      this.attachedDownloads.delete(id);
      if (id === this.localModelId) this.downloading = false;
      await this.ctx.refresh();
      await this.loadModels();
      if (this.index === 1 && this.engine === "local") this.renderEngineDetail();
      this.updateCommon();
    }).catch((error) => {
      this.attachedDownloads.delete(id);
      if (id === this.localModelId) this.downloading = false;
      if (errorCode(error) === "cancelled" || errorText(error).toLowerCase() === "cancelled") { this.updateCommon(); return; }
      this.fail(error);
    });
  }

  private async chooseModel(id: string) {
    if (id === this.localModelId) return;
    const previous = this.localModelId;
    if (this.downloading) await api.cancelDownload(previous).catch(() => undefined);
    this.attachedDownloads.delete(previous);
    this.localModelId = id; this.downloading = false; this.downloadError = null; this.downloadProgress = { downloaded: 0, total: 0 };
    await this.attachDownload();
    if (this.disposed) return;
    this.renderEngineDetail(); this.updateCommon();
  }

  private localModel(): LocalModel | undefined { return this.models.find((model) => model.id === this.localModelId); }
  private customKinds(): KindInfo[] { return this.ctx.info.kinds.filter((kind) => kind.stt && !["local", "apple_speech", "pennel_cloud"].includes(kind.kind)); }
  private customAccounts(): ProviderAccount[] { return this.ctx.info.settings.providers.filter((account) => this.customKinds().some((kind) => kind.kind === account.kind)); }
  private customAccount(): ProviderAccount | undefined { return this.customAccounts().find((account) => account.id === this.providerChoice); }
  private customKind(): KindInfo | undefined {
    const kind = this.providerChoice.startsWith("new:") ? this.providerChoice.slice(4) : this.customAccount()?.kind;
    return this.customKinds().find((item) => item.kind === kind);
  }

  private renderEngine() {
    const language = select(languages, this.language, (value) => {
      this.language = value;
      void this.loadModels().then(() => { if (this.engine === "local") this.renderEngineDetail(); this.updateCommon(); }).catch(this.fail);
    }, "menu");
    language.setAttribute("aria-label", "Taal");
    const choices = h("div", { class: "engine-choices", role: "group", "aria-label": "Spraakherkenning" });
    this.localButtons.clear();
    for (const [value, label] of [["local", "Lokaal"], ["apple", "Apple-dicteren"], ["custom", "Eigen provider"]] as [Engine, string][]) {
      const button = h("button", { class: "engine-choice", "aria-controls": "onboarding-engine-detail", onclick: () => { void this.chooseEngine(value).catch(this.fail); } }, label);
      button.addEventListener("keydown", (event) => {
        const order: Engine[] = ["local", "apple", "custom"];
        const index = order.indexOf(value);
        const next = event.key === "ArrowRight" ? (index + 1) % 3 : event.key === "ArrowLeft" ? (index + 2) % 3 : event.key === "Home" ? 0 : event.key === "End" ? 2 : -1;
        if (next >= 0) { event.preventDefault(); const target = order[next]; if (!this.localButtons.get(target)?.disabled) void this.chooseEngine(target).then(() => this.localButtons.get(target)?.focus()).catch(this.fail); }
      });
      this.localButtons.set(value, button); choices.append(button);
    }
    this.body.replaceChildren(this.title("Hoe wil je dicteren?", "Kies je spraakherkenning. Je kunt dit later aanpassen."),
      h("div", { class: "engine-language" }, h("span", {}, "Taal"), language), choices, this.engineDetail);
    this.renderEngineDetail();
  }

  private async chooseEngine(engine: Engine) {
    this.engine = engine; this.message.hidden = true; this.renderEngineDetail(); this.updateCommon();
  }

  private renderEngineDetail() {
    this.localButtons.forEach((button, engine) => button.setAttribute("aria-pressed", String(engine === this.engine)));
    const apple = this.ctx.info.kinds.find((kind) => kind.kind === "apple_speech");
    const appleButton = this.localButtons.get("apple");
    if (appleButton) { appleButton.disabled = apple?.availability.status === "unsupported"; if (apple?.availability.reason) appleButton.title = apple.availability.reason; }
    const local = this.ctx.info.kinds.find((kind) => kind.kind === "local");
    const localButton = this.localButtons.get("local");
    if (localButton) { localButton.disabled = local?.availability.status === "unsupported"; if (local?.availability.reason) localButton.title = local.availability.reason; }
    if (this.engine === "local") {
      const model = this.localModel();
      const downloaded = model?.state.kind === "downloaded";
      const picker = select(this.models.map((item): [string, string] => [item.id, modelOptionLabel(item, formatBytes)]), this.localModelId, (value) => { void this.chooseModel(value).catch(this.fail); }, "menu");
      picker.setAttribute("aria-label", "Spraakmodel");
      picker.disabled = this.models.length === 0;
      const name = h("div", {}, picker, h("small", {}, model ? (downloaded ? formatBytes(model.size_bytes) : `${formatBytes(model.size_bytes)} · eenmalige download`) : "Geen passend lokaal model beschikbaar."));
      const status = h("span", { class: "engine-model-status", "data-download-state": true });
      const track = h("progress", { class: "engine-download-progress", "aria-label": "Model downloaden", hidden: !this.downloading });
      const caption = h("div", { class: "engine-download-caption", hidden: !this.downloading }, h("span", { "data-download-caption": true }), h("button", { class: "text-button", onclick: () => {
        void api.cancelDownload(this.localModelId).then(() => { this.downloading = false; this.updateDownload(); this.updateCommon(); }).catch(this.fail);
      } }, "Annuleer"));
      this.engineDetail.replaceChildren(h("h2", {}, "Je audio blijft op je apparaat."), h("p", { class: "engine-description" }, "Pennel zet je spraak zelf om naar tekst. Daarvoor download je eenmalig een spraakmodel."),
        h("p", { class: "help" }, "De eerste oefening gebruikt alleen spraakherkenning. Opkuisen kies je later bij Modellen; daarvoor kan een andere provider worden gebruikt."),
        h("div", { class: "engine-model" }, icon("download"), name, status), track, caption,
        ...(this.downloadError ? [h("div", { class: "engine-error", role: "alert" }, h("p", {}, this.downloadError),
          h("button", { class: "text-button", disabled: apple?.availability.status === "unsupported", onclick: () => { void this.chooseEngine("apple"); } }, "Gebruik Apple-dicteren"))] : []));
      this.updateDownload();
    } else if (this.engine === "apple") {
      this.engineDetail.replaceChildren(h("h2", {}, "Met de spraakherkenning van macOS."),
        h("p", { class: "engine-description" }, "Laat Apple je spraak omzetten naar tekst. macOS beheert de benodigde taalmodellen."),
        h("p", { class: "help" }, "De eerste oefening gebruikt alleen spraakherkenning. Opkuisen kies je later bij Modellen; daarvoor kan een andere provider worden gebruikt."),
        h("div", { class: "engine-model" }, icon("audioLines"), h("div", {}, h("strong", {}, "Apple-dicteren"), h("small", {}, "Beschikbaar vanaf macOS 26"))),
        ...(apple?.availability.reason ? [h("p", { class: "help" }, apple.availability.reason)] : []));
    } else this.renderProvider();
  }

  private renderProvider() {
    const options: [string, string][] = [
      ...this.customAccounts().map((account): [string, string] => [account.id, account.name]),
      ...this.customKinds().filter((kind) => !this.customAccounts().some((account) => account.kind === kind.kind)).map((kind): [string, string] => [`new:${kind.kind}`, kind.label]),
    ];
    const picker = select(options, this.providerChoice, (value) => {
      this.providerChoice = value;
      const account = this.customAccount(); const kind = this.customKind();
      this.providerModel = account?.id === this.ctx.info.settings.dictation_stt.provider_id ? this.ctx.info.settings.dictation_stt.model : kind?.default_stt_model ?? "";
      this.providerBaseUrl = account?.base_url ?? kind?.default_base_url ?? "";
      this.renderProvider(); this.updateCommon();
    }, "menu");
    picker.setAttribute("aria-label", "Eigen provider");
    const kind = this.customKind(); const account = this.customAccount();
    const keySet = !!account && this.ctx.info.keys[account.id];
    const key = h("input", { type: "password", "data-onboarding-key": true, "aria-label": "API-sleutel", autocomplete: "off", placeholder: keySet ? "Ingesteld · laat leeg om te behouden" : "Plak je API-sleutel" });
    const model = h("input", { type: "text", value: this.providerModel, "aria-label": "Model", placeholder: "Modelnaam", oninput: (event) => { this.providerModel = (event.target as HTMLInputElement).value; } });
    const url = h("input", { type: "text", value: this.providerBaseUrl, "aria-label": "Serveradres", placeholder: "https://…", oninput: (event) => { this.providerBaseUrl = (event.target as HTMLInputElement).value; } });
    for (const option of picker.options) {
      const optionKind = option.value.startsWith("new:") ? this.customKinds().find((item) => item.kind === option.value.slice(4)) : this.customKinds().find((item) => item.kind === this.customAccounts().find((item) => item.id === option.value)?.kind);
      option.disabled = optionKind?.availability.status === "unsupported";
    }
    this.engineDetail.replaceChildren(h("h2", {}, "Gebruik de provider die je al kent."), h("p", { class: "engine-description" }, "Je audio wordt naar je gekozen provider gestuurd. De eerste oefening gebruikt geen opkuisbeurt."),
      h("div", { class: "onboarding-provider-fields" }, h("label", { class: "preference-row" }, "Provider", picker),
        kind?.requires_key ? h("label", { class: "preference-row" }, "API-sleutel", key) : null,
        h("label", { class: "preference-row" }, "Model", model),
        ["custom", "ollama", "lm_studio"].includes(kind?.kind ?? "") ? h("label", { class: "preference-row" }, "Serveradres", url) : null),
      h("p", { class: "help provider-key-note" }, "Je API-sleutel wordt alleen in de sleutelopslag van je apparaat bewaard."));
  }

  private updateDownload() {
    if (this.engine !== "local" || this.index !== 1) return;
    const progress = this.engineDetail.querySelector<HTMLProgressElement>("progress");
    if (progress) { progress.hidden = !this.downloading; progress.max = this.downloadProgress.total || this.localModel()?.size_bytes || 1; progress.value = this.downloadProgress.downloaded; }
    const caption = this.engineDetail.querySelector<HTMLElement>(".engine-download-caption");
    if (caption) caption.hidden = !this.downloading;
    const label = this.engineDetail.querySelector("[data-download-caption]");
    if (label) label.textContent = this.downloadProgress.total ? `${Math.min(100, Math.round(this.downloadProgress.downloaded / this.downloadProgress.total * 100))}%` : "Download voorbereiden…";
    const status = this.engineDetail.querySelector("[data-download-state]");
    if (status) status.replaceChildren(...(this.localModel()?.state.kind === "downloaded" ? [icon("check"), document.createTextNode("Gedownload")] : []));
  }

  private async refreshPermissions() {
    const [permissions, readiness] = await Promise.all([api.permissions(), api.onboardingReadiness("practice")]);
    if (this.disposed || this.index !== 2) return;
    this.permissions = permissions; this.readiness = readiness;
    const signature = JSON.stringify([permissions, readiness]);
    if (signature !== this.permissionSignature) {
      this.permissionSignature = signature;
      const kinds: [PermissionInfo["kind"], string, string][] = [["microphone", "Microfoon", "Zodat Pennel kan horen wat je zegt."], ["accessibility", "Toegankelijkheid", "Voor de sneltoets en om tekst in andere apps te plaatsen."]];
      if (this.engine === "apple") kinds.push(["speech", "Spraakherkenning", "Nodig om Apple-dicteren te gebruiken."]);
      this.readyRow.replaceChildren(...kinds.map(([kind, label, description]) => this.permissionRow(kind, label, description)),
        ...(!readiness.can_use_shortcut ? [h("p", { class: "help" }, this.ctx.info.capabilities.global_hotkey.reason ?? "Je kunt de knop gebruiken en je sneltoets later instellen.")] : []));
    }
    this.updateCommon();
  }

  private permissionRow(kind: PermissionInfo["kind"], label: string, description: string): HTMLElement {
    const permission = this.permissions.find((item) => item.kind === kind);
    const granted = permission?.status === "granted" || permission?.status === "not_required";
    const unsupported = permission?.status === "unsupported";
    const unknown = permission?.status === "unknown";
    const status = h("span", { class: "permission-status" }, ...(granted ? [icon("check"), document.createTextNode("In orde")] : []));
    const button = h("button", { class: "btn", disabled: unsupported || (this.ctx.info.platform === "linux" && kind === "microphone"), onclick: async () => {
      button.disabled = true;
      try { await api.requestPermission(kind); await this.refreshPermissions(); }
      catch (error) { this.fail(error); button.disabled = false; }
    } }, permission?.status === "denied" || kind === "accessibility" || this.ctx.info.platform === "windows" ? "Open instellingen" : "Geef toegang");
    return h("div", { class: "permission-row" }, icon(kind === "microphone" ? "mic" : kind === "speech" ? "audioLines" : "keyboard"),
      h("div", { class: "grow" }, h("strong", {}, label), h("p", { class: "help" }, description),
        granted ? null : unsupported ? h("p", { class: "help" }, "Niet nodig of niet ondersteund op dit platform.")
          : unknown && this.ctx.info.platform === "linux" ? h("p", { class: "help" }, "Je microfoon wordt bij de oefening gecontroleerd.") : button,
        kind === "microphone" && unknown && this.ctx.info.platform === "windows" ? h("p", { class: "help" }, "Je microfoon wordt bij de oefening gecontroleerd.") : null), status);
  }

  private async renderPractice() {
    this.practiceDone = false;
    this.practiceText = h("textarea", { class: "practice-text", "aria-label": "Je eerste dictaat", placeholder: "Je woorden verschijnen hier…", spellcheck: true, disabled: true });
    this.practiceStatus = h("p", { class: "practice-status help", role: "status", "aria-live": "polite" });
    this.practiceKey = this.keycap();
    this.practiceButton = h("button", { class: "text-button", onclick: () => { void this.practice?.toggle().catch(this.fail); } }, "Start dicteren");
    const apply = (snapshot: DictationSessionSnapshot) => {
      if (this.disposed || this.index !== 3 || !this.practiceText || !this.practiceStatus || !this.practiceButton) return;
      const busy = snapshot.phase === "listening" || snapshot.phase === "working";
      this.practiceText.disabled = busy;
      if (snapshot.phase === "done") this.practiceText.value = snapshot.text;
      this.practiceStatus.textContent = snapshot.error ?? (snapshot.phase === "listening" ? "Luistert… laat de toets los." : snapshot.phase === "working" ? "Je woorden worden uitgeschreven…" : snapshot.phase === "done" ? "Dit is je eerste dictaat." : `Hou de toets hieronder in of gebruik ${this.hotkey()}.`);
      this.practiceStatus.classList.toggle("error", !!snapshot.error);
      this.practiceKey?.setAttribute("data-pressed", String(snapshot.phase === "listening"));
      this.practiceButton.textContent = snapshot.phase === "listening" ? "Stop dicteren" : snapshot.phase === "working" ? "Tekst verwerken…" : "Start dicteren";
      this.practiceButton.disabled = snapshot.phase === "working";
      if (snapshot.phase === "done" && snapshot.text.trim() && !this.practiceDone) {
        this.fresh = false;
        this.practiceDone = true;
        this.body.querySelector("h1")!.textContent = "Precies. Zo werkt Pennel.";
        const introduction = this.body.querySelector(".onboarding-heading p");
        if (introduction) introduction.textContent = "Dezelfde sneltoets werkt waar je ook typt.";
        void this.persist("voice").catch(this.fail);
      }
      this.updateCommon();
    };
    this.practice = new OwnedDictationSession("practice", apply);
    const press = () => {
      this.held = true;
      this.practiceStart = (async () => { await this.practice?.open(); if (this.practice?.state.snapshot?.phase !== "listening") await this.practice?.toggle(); })().catch(this.fail);
    };
    const release = () => {
      if (!this.held) return;
      this.held = false;
      void this.practiceStart.then(async () => { if (this.practice?.state.snapshot?.phase === "listening") await this.practice.toggle(); }).catch(this.fail);
    };
    this.practiceKey.addEventListener("pointerdown", (event) => { event.preventDefault(); this.practiceKey?.setPointerCapture(event.pointerId); press(); });
    this.practiceKey.addEventListener("pointerup", release);
    this.practiceKey.addEventListener("pointercancel", () => { this.held = false; void this.practice?.cancel().catch(this.fail); });
    this.practiceKey.addEventListener("keydown", (event) => { if ([" ", "Enter"].includes(event.key)) { event.preventDefault(); if (!event.repeat) press(); } });
    this.practiceKey.addEventListener("keyup", (event) => { if ([" ", "Enter"].includes(event.key)) { event.preventDefault(); release(); } });
    this.practiceKey.addEventListener("click", (event) => { if (event.detail === 0 && !this.held) void this.practice?.toggle().catch(this.fail); });
    this.body.replaceChildren(this.title("Probeer je eerste dictaat.", `Hou ${this.hotkey()} in, spreek de zin uit en laat los.`),
      h("div", { class: "practice-gesture" }, this.practiceKey, h("div", {}, h("p", { class: "practice-phrase" }, `“${phrases[this.language] ?? phrases.nl}”`), h("p", { class: "help" }, "Je kunt natuurlijk ook iets anders zeggen."))),
      this.practiceText, this.practiceStatus, h("div", { class: "row practice-help" }, this.practiceButton,
        h("details", {}, h("summary", {}, `Werkt ${this.hotkey()} nog niet?`), h("p", { class: "help" }, this.ctx.info.platform === "macos"
          ? "Zet bij Systeeminstellingen → Toetsenbord de fn-toets op ‘Doe niets’. Je kunt ook een andere sneltoets kiezen bij Dicteren."
          : this.ctx.info.capabilities.global_hotkey.reason ?? "Gebruik de knop om te oefenen. Je kunt je sneltoets later instellen."))));
    try { await this.practice.open(); } catch (error) { this.fail(error); }
  }

  private async renderVoice(generation: number) {
    const setup = await api.speakerSetup();
    if (generation !== this.renderGeneration || this.disposed) return;
    const mode = this.ctx.info.settings.speaker_recognition === "cloud" ? "cloud" : "local";
    const detail = h("div", { class: "voice-step" });
    const models = new SpeakerModels(setup.model_ids, () => draw());
    const download = speakerDownload(models);
    let shown = "";
    const draw = () => {
      if (this.disposed || this.voiceModels !== models) return;
      download.update(); this.updateCommon();
      const needsModels = mode === "local" && setup.local_available && !models.ready;
      const blocker = needsModels ? null : enrollmentBlocker(setup, mode, models);
      const view = needsModels ? "download" : blocker ? "blocked" : "record";
      if (view === shown) return;
      shown = view;
      if (view === "download") {
        detail.replaceChildren(h("section", { class: "onboarding-engine-detail" }, h("h2", {}, "Eerst de sprekermodellen."),
          h("p", { class: "engine-description" }, "Pennel herkent stemmen op je computer. Daarvoor download je eenmalig twee kleine modellen; je audio verlaat je toestel niet."),
          download.element));
      } else if (view === "blocked") {
        detail.replaceChildren(h("section", { class: "onboarding-engine-detail" }, h("h2", {}, mode === "cloud" ? "Via Pennel Cloud." : "Niet op dit toestel."),
          h("p", { class: "engine-description" }, mode === "cloud" ? "Je koos om sprekers via Pennel Cloud te herkennen. Log in bij Instellingen → Account om je stem te leren." : blocker),
          h("p", { class: "help" }, "Sla deze stap gerust over. Je kunt je stem later leren bij Instellingen → Meetings.")));
      } else {
        this.voicePanel ??= new EnrollmentPanel(setup.enrollment_text, () => { this.fresh = false; }, () => this.updateCommon());
        detail.replaceChildren(this.voicePanel.element,
          ...setup.profile ? [h("p", { class: "help voice-step-note" }, `${trainedLabel(setup.profile)}. Neem opnieuw op om bij te werken.`)] : []);
      }
    };
    this.voiceModels = models;
    this.body.replaceChildren(this.title("Leer je stem", enrollmentIntro), detail);
    await models.start(this.ctx.info.active_model_downloads ?? []);
  }

  private async commitEngine() {
    let account: ProviderAccount | undefined;
    let model: string;
    if (this.engine === "local") {
      account = this.ctx.info.settings.providers.find((item) => item.kind === "local"); model = this.localModelId;
      if ((await api.modelState(model)).kind !== "downloaded") { await this.loadModels(); this.renderEngineDetail(); throw new Error("Download eerst het spraakmodel om verder te gaan."); }
    }
    else if (this.engine === "apple") { account = this.ctx.info.settings.providers.find((item) => item.kind === "apple_speech"); model = this.ctx.info.kinds.find((item) => item.kind === "apple_speech")?.default_stt_model ?? "dictation"; }
    else { account = this.customAccount(); model = this.providerModel.trim(); }
    const kind: ProviderKind = this.engine === "local" ? "local" : this.engine === "apple" ? "apple_speech" : this.customKind()?.kind ?? "open_ai";
    if (!model) throw new Error("Kies een model om verder te gaan.");
    const infoKind = this.ctx.info.kinds.find((item) => item.kind === kind);
    if (this.engine === "custom" && ["custom", "ollama", "lm_studio"].includes(kind)) {
      try { if (!["http:", "https:"].includes(new URL(this.providerBaseUrl.trim()).protocol)) throw new Error(); }
      catch { throw new Error("Vul een geldig serveradres in om verder te gaan."); }
    }
    const key = this.body.querySelector<HTMLInputElement>("[data-onboarding-key]")?.value.trim() ?? "";
    if (infoKind?.requires_key && !key && (!account || !this.ctx.info.keys[account.id])) throw new Error("Vul je API-sleutel in om verder te gaan.");
    if (!account) {
      const next = await api.addProvider(kind); this.ctx.update(next);
      account = [...next.settings.providers].reverse().find((item) => item.kind === kind);
    }
    if (!account) throw new Error("De provider kon niet worden toegevoegd.");
    const id = account.id;
    if (key) this.ctx.update(await api.setKey(id, key));
    await this.ctx.save((settings) => {
      if (this.engine === "custom") {
        const provider = settings.providers.find((item) => item.id === id);
        if (provider && this.providerBaseUrl.trim()) provider.base_url = this.providerBaseUrl.trim();
      }
      settings.dictation_stt = { provider_id: id, model };
      if (this.fresh) settings.meeting_stt = { provider_id: id, model };
      settings.language = this.language || null;
    });
  }

  private async advance() {
    if (this.actionBusy || this.disposed) return;
    this.actionBusy = true; this.message.hidden = true; this.updateCommon();
    try {
      if (this.index === 1) {
        const model = this.localModel();
        if (this.engine === "local" && model?.state.kind !== "downloaded") {
          if (!model || this.downloading) return;
          this.downloadError = null; this.downloading = true;
          this.downloadProgress = { downloaded: model.state.kind === "partial" ? model.state.bytes : 0, total: model.size_bytes };
          this.renderEngineDetail(); this.updateCommon();
          try { await api.downloadModel(model.id); } catch (error) { this.downloading = false; this.downloadError = errorText(error); this.renderEngineDetail(); throw error; }
          return;
        }
        await this.commitEngine();
      }
      if (this.index === 3 && !this.practiceDone) return;
      if (this.index === 4) {
        if (this.voicePanel?.busy) return;
        this.closeVoice(); await this.persist("voice", "completed"); await api.showWorkspace(workspaceRoute("dictation")); return;
      }
      const next = this.index + 1;
      await this.closePractice(); await this.persist(onboardingSteps[next]); this.index = next; await this.renderStep();
    } finally { this.actionBusy = false; this.updateCommon(); }
  }

  private updateCommon() {
    if (this.disposed) return;
    this.body.inert = this.actionBusy;
    this.progress.hidden = this.index === 0;
    this.progress.setAttribute("aria-label", `Stap ${this.index + 1} van ${onboardingSteps.length}`);
    [...this.progress.children].forEach((segment, index) => segment.setAttribute("data-active", String(index <= this.index)));
    this.back.hidden = this.index === 0; this.back.disabled = this.actionBusy;
    this.skip.textContent = this.index === 3 ? "Later proberen" : this.index === 4 ? "Overslaan" : "Later instellen";
    this.skip.disabled = this.actionBusy || !!this.voicePanel?.busy;
    this.count.textContent = `${this.index + 1} van ${onboardingSteps.length}`;
    const dark = this.ctx.info.settings.appearance === "dark" || (this.ctx.info.settings.appearance === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
    this.theme.replaceChildren(icon(dark ? "sun" : "moon"));
    let label = this.index === 0 ? "Aan de slag" : this.index === 4 ? "Klaar" : "Verder";
    let disabled = this.actionBusy;
    if (this.index === 1 && this.engine === "local") {
      const model = this.localModel(); disabled ||= !model || this.downloading;
      if (model?.state.kind !== "downloaded") label = this.downloading ? "Downloaden…" : this.downloadError ? "Opnieuw proberen" : model?.state.kind === "partial" ? "Verder downloaden" : "Download model";
    }
    if (this.index === 1 && this.engine === "custom") { label = "Gebruik provider"; disabled ||= !this.customKind(); }
    if (this.index === 1 && this.engine === "apple") {
      const availability = this.ctx.info.kinds.find((kind) => kind.kind === "apple_speech")?.availability;
      disabled ||= !availability || ["unsupported", "unavailable"].includes(availability.status);
    }
    if (this.index === 2) {
      const manualAllowed = manualPracticeAllowed(this.readiness, this.permissions, this.ctx.info.settings);
      disabled ||= !manualAllowed;
      if (manualAllowed && this.ctx.info.platform === "macos" && this.readiness?.can_use_shortcut === false) label = `Verder zonder ${this.hotkey()}`;
    }
    if (this.index === 3) disabled ||= !this.practiceDone;
    if (this.index === 4) disabled ||= !!this.voicePanel?.busy || !!this.voiceModels?.downloading;
    this.next.disabled = disabled; this.next.replaceChildren(label, icon("arrowRight"));
  }

  private dispose() {
    this.disposed = true; ++this.renderGeneration; this.poll?.dispose(); this.closeVoice();
    this.cleanup.forEach((cleanup) => cleanup());
    void this.closePractice().catch(this.ctx.report);
  }
}

export async function onboardingView(ctx: Context): Promise<HTMLElement> {
  const wizard = new OnboardingWizard(ctx);
  await wizard.start();
  return wizard.root;
}
