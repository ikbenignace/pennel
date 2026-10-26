import { api, type Capability, type KnownVoice, type MeetingTemplate, type RecordAction, type SpeakerRecognitionMode, type SpeakerSetup } from "../api";
import { askText, confirmDialog, errorText, h, section, select, toggle } from "../dom";
import { trainedLabel } from "../speaker-state";
import { SpeakerModels, enrollmentBlocker, enrollmentDialog, speakerDownload } from "./voices";
import type { Context } from "../main";
import { icon } from "../icons";
import { appBadge } from "../app-icon";

const actions: [RecordAction, string][] = [["ask", "Vragen"], ["always", "Altijd"], ["never", "Nooit"]];
const apps: [string, string][] = [
  ["com.microsoft.teams2", "Microsoft Teams"], ["us.zoom.xos", "Zoom"],
  ["com.tinyspeck.slackmacgap", "Slack"], ["com.apple.FaceTime", "FaceTime"],
  ["net.whatsapp.WhatsApp", "WhatsApp"], ["Cisco-Systems.Spark", "Webex"],
  ["com.hnc.Discord", "Discord"], ["com.google.Chrome", "Google Chrome"],
  ["com.apple.Safari", "Safari"], ["company.thebrowser.Browser", "Arc"], ["com.microsoft.edgemac", "Microsoft Edge"],
  ["org.mozilla.firefox", "Firefox"], ["com.brave.Browser", "Brave"],
];

function locked(capability: Capability): boolean {
  return capability.status === "unsupported" || capability.status === "unavailable";
}

function templateEditor(ctx: Context, target: { id: string | null; name: string; body: string }) {
  const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const titleId = `template-editor-${crypto.randomUUID()}`;
  const dialog = h("dialog", { class: "editor-dialog template-editor", "aria-labelledby": titleId });
  const name = h("input", { type: "text", value: target.name, placeholder: "Naam", "aria-label": "Naam" });
  const body = h("textarea", { rows: 12, "aria-label": "Inhoud van het sjabloon" });
  body.value = target.body;
  const status = h("p", { class: "error", role: "status" });
  const sections = h("p", { class: "help" });
  const update = () => {
    const found = body.value.split("\n").map((line) => line.trim().match(/^##\s+(.+)/)?.[1]).filter(Boolean);
    sections.textContent = `Secties: ${(found.length ? found : ["Samenvatting", "Beslissingen", "Actiepunten"]).join(" · ")}`;
  };
  body.addEventListener("input", update);
  update();
  const close = () => { dialog.close(); dialog.remove(); if (previousFocus?.isConnected) previousFocus.focus(); };
  const save = h("button", { class: "btn primary", type: "submit" }, "Bewaar");
  const form = h("form", {}, h("h2", { id: titleId }, target.id ? "Sjabloon bewerken" : "Nieuw sjabloon"), name, body,
    h("p", { class: "help" }, "Schrijf wat de samenvatting moet doen. Elke regel die met ## begint wordt een vaste sectie."),
    sections, status, h("div", { class: "row dialog-actions" }, h("span", { class: "grow" }),
      h("button", { class: "btn", type: "button", onclick: close }, "Annuleren"), save));
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!name.value.trim()) { status.textContent = "Geef het sjabloon een naam."; return; }
    save.disabled = true;
    try { await api.saveTemplate(target.id, name.value.trim(), body.value); close(); ctx.rerender(); }
    catch (error) { status.textContent = errorText(error); save.disabled = false; }
  });
  dialog.append(form);
  dialog.addEventListener("cancel", (event) => { event.preventDefault(); close(); });
  document.body.append(dialog);
  dialog.showModal();
  name.focus();
  ctx.onDispose(close);
}

async function templateSection(ctx: Context, templates: MeetingTemplate[]) {
  const edit = async (template: MeetingTemplate) => templateEditor(ctx, {
    id: template.id, name: template.name, body: await api.templateSource(template.id) ?? "",
  });
  const remove = async (template: MeetingTemplate) => {
    if (!await confirmDialog(template.builtin ? "Origineel sjabloon herstellen?" : `Sjabloon ${template.name} verwijderen?`,
      template.builtin ? "Je aanpassingen aan dit sjabloon verdwijnen." : "Het sjabloon verdwijnt. Je bestaande meetingnotities blijven staan.",
      template.builtin ? "Herstel" : "Verwijder")) return;
    await api.deleteTemplate(template.id);
    const remaining = await api.meetingTemplates();
    if (!remaining.some((item) => item.id === ctx.info.settings.default_template)) {
      await ctx.save((settings) => { settings.default_template = remaining[0]?.id ?? "algemeen"; });
    } else ctx.rerender();
  };
  return section("Sjablonen", "Bepalen welke secties een samenvatting krijgt. Bewaard als Markdown in je meetingmap.",
    templates.map((template) => h("div", { class: "row template-row" },
      h("span", { class: "grow" }, template.name),
      !template.builtin || template.customized ? h("span", { class: "badge" }, template.builtin ? "aangepast" : "eigen") : null,
      h("button", { class: "btn", onclick: () => edit(template) }, "Bewerk"),
      !template.builtin || template.customized ? h("button", { class: "btn", onclick: () => remove(template) }, template.builtin ? "Herstel" : "Verwijder") : null)),
    h("button", { class: "btn", onclick: () => templateEditor(ctx, { id: null, name: "", body: "Beschrijf waar de samenvatting op moet letten.\n\n## Samenvatting\n## Actiepunten\n" }) }, "Nieuw sjabloon"));
}

async function speakerSections(ctx: Context, setup: SpeakerSetup): Promise<HTMLElement[]> {
  const settings = ctx.info.settings;
  const mode: SpeakerRecognitionMode = settings.speaker_recognition === "cloud" ? "cloud" : "local";
  const modelStatus = h("p", { class: "help", role: "status" });
  let download: ReturnType<typeof speakerDownload> | null = null;
  const models = new SpeakerModels(setup.model_ids, () => {
    modelStatus.textContent = models.ready ? "Sprekermodellen klaar"
      : models.downloading ? "Sprekermodellen worden gedownload."
      : setup.legacy_model ? "Het oudere sprekermodel werkt nog, maar herkent stemmen niet over meetings heen. Download de nieuwe sprekermodellen om dat wel te doen."
      : "Sprekermodellen nog niet gedownload.";
    download?.update();
    learnReason.textContent = enrollmentBlocker(setup, mode, models) ?? "";
    learnReason.hidden = !learnReason.textContent;
    for (const button of [learn, again]) button.disabled = !!enrollmentBlocker(setup, mode, models);
  });
  download = speakerDownload(models);
  ctx.onDispose(() => models.dispose());

  const choice = select<SpeakerRecognitionMode>([["local", "Op dit toestel"], ["cloud", "Via Pennel Cloud"]], mode,
    (value) => ctx.save((next) => { next.speaker_recognition = value; }), "segmented");
  choice.setAttribute("aria-label", "Waar sprekers herkend worden");
  choice.options[0].disabled = !setup.local_available;
  const route = [
    setup.local_available ? null : h("p", { class: "help" }, "Op dit toestel herkennen kan niet in deze versie van Pennel. Kies ‘Via Pennel Cloud’."),
    ...mode === "cloud"
      ? [h("p", { class: "help" }, "Je audio gaat naar Pennel Cloud, wordt daar in het geheugen verwerkt en niet bewaard. Daarvoor log je in bij Pennel Cloud."),
        setup.signed_in ? null : h("button", { class: "text-button speaker-sign-in", onclick: () => ctx.navigate("account") }, "Inloggen bij Pennel Cloud")]
      : setup.local_available ? [modelStatus, download.element, h("p", { class: "help" }, "Herkent stemmen op je computer. Je audio verlaat je toestel niet.")] : [],
  ];

  const learnVoice = async () => {
    if (await enrollmentDialog(setup.enrollment_text)) ctx.rerender();
  };
  const learn = h("button", { class: "btn", onclick: learnVoice }, "Leer je stem");
  const again = h("button", { class: "btn", onclick: learnVoice }, "Opnieuw");
  const forget = h("button", { class: "btn danger", onclick: async () => {
    if (!await confirmDialog("Je stem vergeten?", "Pennel herkent je dan niet meer automatisch op een gedeelde microfoon. Bestaande meetings blijven zoals ze zijn.", "Vergeet")) return;
    try { await api.forgetVoiceProfile(); ctx.rerender(); } catch (error) { ctx.report(error); }
  } }, "Vergeet mijn stem");
  const learnReason = h("p", { class: "help", hidden: true });

  const knownRow = (voice: KnownVoice) => h("div", { class: "row template-row" },
    h("span", { class: "grow" }, voice.name),
    h("span", { class: "help" }, voice.meetings === 1 ? "1 meeting" : `${voice.meetings} meetings`),
    h("button", { class: "btn", onclick: async () => {
      const name = await askText("Naam van deze stem", voice.name);
      if (!name || name === voice.name) return;
      try { await api.renameKnownVoice(voice.id, name); ctx.rerender(); } catch (error) { ctx.report(error); }
    } }, "Wijzig naam"),
    h("button", { class: "btn", onclick: async () => {
      if (!await confirmDialog(`Stem van ${voice.name} vergeten?`, "Pennel geeft deze stem in nieuwe meetings geen naam meer. Namen in bestaande meetings blijven staan.", "Vergeet")) return;
      try { await api.forgetKnownVoice(voice.id); ctx.rerender(); } catch (error) { ctx.report(error); }
    } }, "Vergeet"));
  const forgetAll = h("button", { class: "btn danger", disabled: !setup.known.length && !setup.profile, onclick: async () => {
    if (!await confirmDialog("Alle stemmen vergeten?", "Je eigen stem en alle onthouden stemmen verdwijnen van dit toestel. Namen in bestaande meetings blijven staan.", "Vergeet alles")) return;
    try { await api.forgetAllVoices(); ctx.rerender(); } catch (error) { ctx.report(error); }
  } }, "Vergeet alle stemmen");

  const sections = [
    section("Sprekers herkennen", "Herkent na de meeting wie er sprak. Stemmen die je onthoudt, krijgen voortaan zelf hun naam.",
      toggle("Sprekers herkennen na elke meeting", settings.diarization,
        (value) => ctx.save((next) => { next.diarization = value; }), ctx.info.capabilities.diarization),
      h("div", { class: "row speaker-route" }, choice), ...route),
    section("Je stem", "Zo blijft wat jij zegt ‘Ik’, ook als iedereen rond één microfoon zit. Alleen een stemafdruk blijft op dit toestel, nooit de opname.",
      h("div", { class: "row" }, h("span", { class: "grow" }, trainedLabel(setup.profile)),
        ...(setup.profile ? [again, forget] : [learn])), learnReason),
    section("Bekende stemmen", "Stemmen die je koos om te onthouden. Pennel geeft ze in volgende meetings zelf hun naam.",
      setup.known.length ? setup.known.map(knownRow)
        : h("p", { class: "help" }, "Nog geen stemmen onthouden. Geef een stem een naam in een meeting en kies ‘Onthoud deze stem’."),
      h("div", { class: "row speaker-forget-all" }, forgetAll)),
  ];
  await models.start(ctx.info.active_model_downloads ?? []);
  return sections;
}

export async function meetingSettingsView(ctx: Context): Promise<HTMLElement> {
  const settings = ctx.info.settings;
  const [templates, recording, setup] = await Promise.all([api.meetingTemplates(), api.meetingStatus(), api.speakerSetup()]);
  const pathLocked = ctx.info.preview || recording.busy;
  const path = h("span", { class: "grow meeting-path", "aria-label": "Meetingmap" }, ctx.info.preview ? "Preview · aparte meetingmap" : settings.meetings_dir || "~/Pennel");
  return h("div", { class: "settings-pane" },
    section("Map", "Elke meeting wordt een map met Markdown en audio. Leesbaar in Obsidian.",
      h("div", { class: "row" }, path,
        h("button", { class: "btn", disabled: pathLocked, onclick: async () => {
          const selected = await api.chooseMeetingsDir();
          if (selected !== null) await ctx.save((next) => { next.meetings_dir = selected; });
        } }, "Kies…"),
        settings.meetings_dir ? h("button", { class: "btn", disabled: pathLocked, onclick: () => ctx.save((next) => { next.meetings_dir = ""; }) }, "Standaard") : null),
      ctx.info.preview ? h("p", { class: "help" }, "De preview gebruikt uitsluitend zijn eigen meetingmap.") : null,
      !ctx.info.preview && recording.busy ? h("p", { class: "help" }, "Wacht tot de opname is afgerond voordat je de meetingmap wijzigt.") : null,
      /(?:Mobile Documents|\/Documents(?:\/|$)|\/Desktop(?:\/|$))/.test(settings.meetings_dir)
        ? h("p", { class: "help" }, "Als je deze map synchroniseert, gaan je meetings en audio ook naar die dienst.") : null),
    section("Standaardsjabloon", null, select(templates.map((template) => [template.id, template.name]), settings.default_template,
      (id) => ctx.save((next) => { next.default_template = id; }), "menu")),
    await templateSection(ctx, templates),
    ...await speakerSections(ctx, setup),
    section("Audio bewaren", "Notities en transcript blijven altijd staan.",
      select([["30", "30 dagen"], ["90", "90 dagen"], ["365", "1 jaar"], ["0", "Altijd"]], String(settings.meeting_audio_retention_days),
        (value) => ctx.save((next) => { next.meeting_audio_retention_days = Number(value); }))));
}

export async function recordingSettingsView(ctx: Context): Promise<HTMLElement> {
  const { settings, capabilities, platform } = ctx.info;
  const snapshot = await api.calendar();
  const names = new Map(apps);
  const appIds = new Map((ctx.info.recording_app_ids ?? apps.map(([id]) => id)).map((id) => [id, names.get(id) ?? id]));
  settings.app_rules.forEach((rule) => { if (!appIds.has(rule.bundle_id)) appIds.set(rule.bundle_id, rule.bundle_id); });
  const addTitle = h("input", { type: "text", placeholder: "Titel van de meeting", class: "grow" });
  const add = async () => {
    const title = addTitle.value.trim();
    if (!title || settings.event_rules.some((rule) => rule.title.toLocaleLowerCase() === title.toLocaleLowerCase())) return;
    await ctx.save((next) => { next.event_rules.push({ title, action: "always" }); });
  };
  addTitle.addEventListener("keydown", (event) => { if (event.key === "Enter") void add(); });
  return h("div", { class: "settings-pane" },
    section("Calls herkennen", "Pennel merkt wanneer een call-app je microfoon gebruikt en vraagt of je wilt opnemen.",
      toggle("Automatisch herkennen", settings.auto_detect, (value) => ctx.save((next) => { next.auto_detect = value; }), capabilities.call_detection),
      toggle(`Sneltoets ${platform === "macos" ? "⌘⇧R" : "Ctrl+Shift+R"} om een opname te starten of stoppen`, settings.record_shortcut_enabled,
        (value) => ctx.save((next) => { next.record_shortcut_enabled = value; }), capabilities.record_shortcut),
      toggle("Echo-onderdrukking (als je via speakers belt)", settings.echo_cancellation,
        (value) => ctx.save((next) => { next.echo_cancellation = value; }), capabilities.echo_cancellation),
      toggle("Toestemmingszin kopiëren bij de start van een opname", settings.consent_enabled,
        (value) => ctx.save((next) => { next.consent_enabled = value; }))),
    section("Per app", null,
      [...appIds].map(([id, name]) => h("div", { class: "row rule-row" }, appBadge(id).icon, h("span", { class: "grow" }, name),
        select(actions, settings.app_rules.find((rule) => rule.bundle_id === id)?.action ?? "ask", (action) => ctx.save((next) => {
          next.app_rules = next.app_rules.filter((rule) => rule.bundle_id !== id);
          if (action !== "ask") next.app_rules.push({ bundle_id: id, action });
        }))))),
    section("Agenda", platform === "macos"
      ? "Geeft meetings de titel en deelnemers uit je Mac-agenda. Outlook verschijnt hier als je het werkaccount toevoegt bij Systeeminstellingen → Internetaccounts."
      : "Geeft meetings de titel en deelnemers uit je agenda.",
      toggle("Agenda gebruiken", settings.calendar_enabled, async (value) => {
        if (value && capabilities.calendar.status === "permission_required") await api.requestPermission("calendar");
        await ctx.save((next) => { next.calendar_enabled = value; });
      }, capabilities.calendar),
      settings.calendar_enabled && !locked(capabilities.calendar) ? snapshot.calendars.map((calendar) => h("div", { class: "row rule-row" },
        h("span", { class: "grow" }, calendar.name),
        select(actions, settings.calendar_rules.find((rule) => rule.calendar_id === calendar.id)?.action ?? "ask", (action) => ctx.save((next) => {
          next.calendar_rules = next.calendar_rules.filter((rule) => rule.calendar_id !== calendar.id);
          if (action !== "ask") next.calendar_rules.push({ calendar_id: calendar.id, calendar_name: calendar.name, action });
        })))) : null),
    section("Per meeting", "Bijvoorbeeld je terugkerende standup altijd, of ‘Lunch’ nooit. Gaat voor op agenda en app.",
      h("div", { class: "row" }, addTitle, h("button", { class: "btn", onclick: add }, "Toevoegen")),
      settings.event_rules.map((rule) => h("div", { class: "row rule-row" }, h("span", { class: "grow" }, rule.title),
        select(actions, rule.action, (action) => ctx.save((next) => { const found = next.event_rules.find((item) => item.title === rule.title); if (found) found.action = action; })),
        h("button", { class: "button-icon", "aria-label": `Regel voor ${rule.title} verwijderen`, onclick: () => ctx.save((next) => { next.event_rules = next.event_rules.filter((item) => item.title !== rule.title); }) }, icon("close"))))));
}
