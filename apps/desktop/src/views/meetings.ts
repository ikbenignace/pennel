import { listen } from "@tauri-apps/api/event";
import { api, type MeetingDetail, type Segment } from "../api";
import { errorText, formatClock, h, section, select } from "../dom";
import type { Context } from "../main";

let stopping = false;
let stoppedListener = false;
let notesTimer: number | undefined;

function speakerLabel(segment: Segment, names: { voice: number; name: string }[]): string {
  if (segment.speaker === "me") return "Ik";
  if (segment.voice === null) return "Anderen";
  return names.find((n) => n.voice === segment.voice)?.name ?? `Anderen ${segment.voice}`;
}

function transcript(segments: Segment[], names: { voice: number; name: string }[] = []) {
  if (!segments.length) return h("p", { class: "muted" }, "Nog geen transcript.");
  return h(
    "div",
    { class: "transcript" },
    segments.map((s) =>
      h(
        "p",
        {},
        h("span", { class: "muted" }, `[${formatClock(s.start_ms / 1000)}] `),
        h("span", { class: "speaker" }, `${speakerLabel(s, names)}: `),
        s.failed ? h("em", { class: "error" }, "transcriptie mislukt") : s.text,
      ),
    ),
  );
}

/** Re-runs `tick` every `ms` while `node` is on screen. */
function poll(node: HTMLElement, ms: number, tick: () => void) {
  const timer = window.setInterval(() => {
    if (!node.isConnected) {
      window.clearInterval(timer);
      return;
    }
    tick();
  }, ms);
}

export async function meetingView(ctx: Context): Promise<HTMLElement> {
  if (!stoppedListener) {
    stoppedListener = true;
    await listen<string | null>("meeting-stopped", (e) => {
      stopping = false;
      if (e.payload) ctx.navigate("meetings", e.payload);
      else ctx.rerender();
    });
  }
  const status = await api.meetingStatus();
  if (stopping) {
    return h("div", {}, h("h1", {}, "Opnemen"), section("Afronden", "Pennel transcribeert de laatste stukken…", h("progress", {})));
  }
  if (!status.recording) {
    const title = h("input", { type: "text", placeholder: "Titel, bv. Weekly sync", class: "grow" });
    const error = h("p", { class: "error" });
    const start = async () => {
      try {
        await api.startMeeting(title.value);
        ctx.rerender();
      } catch (e) {
        error.textContent = errorText(e);
      }
    };
    return h(
      "div",
      {},
      h("h1", {}, "Opnemen"),
      section(
        "Nieuwe meeting",
        "Pennel neemt je microfoon op als “Ik” en wat je computer afspeelt (Teams, Meet, Zoom…) als “Anderen”.",
        h("div", { class: "row" }, title, h("button", { class: "btn primary", onclick: start }, "Start opname")),
        error,
      ),
    );
  }

  const header = h("p", { class: "live" });
  const live = h("div", {});
  const warning = h("p", { class: "muted" });
  const notes = h("textarea", { rows: 8, placeholder: "Je notities — Pennel vult ze later aan met de meeting." });
  notes.addEventListener("input", () => {
    window.clearTimeout(notesTimer);
    notesTimer = window.setTimeout(() => api.setMeetingNotes(notes.value).catch(() => {}), 600);
  });
  const update = async () => {
    const s = await api.meetingStatus();
    if (!s.recording) return;
    header.textContent = `● ${s.title} · ${formatClock(s.elapsed_secs)}${s.pending ? ` · ${s.pending} in wachtrij` : ""}`;
    warning.textContent = !s.system_audio
      ? "Systeemgeluid niet beschikbaar — alleen je microfoon wordt opgenomen."
      : !s.heard_system_audio && s.elapsed_secs > 20
        ? "Nog geen systeemgeluid gehoord."
        : "";
    live.replaceChildren(transcript(s.transcript));
  };
  await update();
  const stop = async () => {
    window.clearTimeout(notesTimer);
    await api.setMeetingNotes(notes.value).catch(() => {});
    stopping = true;
    await api.stopMeeting();
    ctx.rerender();
  };
  const node = h(
    "div",
    {},
    h("h1", {}, "Opnemen"),
    section("Bezig", null, h("div", { class: "row" }, header, h("span", { class: "grow" }), h("button", { class: "btn primary", onclick: stop }, "Stop opname")), warning),
    section("Notities", null, notes),
    section("Transcript", null, live),
  );
  poll(node, 2000, update);
  return node;
}

export async function meetingsView(ctx: Context): Promise<HTMLElement> {
  if (ctx.param) return meetingDetail(ctx, ctx.param);
  const search = h("input", { type: "search", placeholder: "Zoek in meetings", class: "grow" });
  const list = h("div", {});
  const load = async () => {
    const meetings = await api.meetings(search.value);
    list.replaceChildren(
      ...(meetings.length
        ? meetings.map((m) =>
            h(
              "button",
              { class: "meeting-row", onclick: () => ctx.navigate("meetings", m.id) },
              h("span", {}, m.title),
              h(
                "span",
                { class: "muted" },
                new Date(m.start_iso).toLocaleString("nl-BE", { dateStyle: "medium", timeStyle: "short" }),
                m.duration_min ? ` · ${m.duration_min} min` : "",
              ),
            ),
          )
        : [h("p", { class: "muted" }, "Nog geen meetings.")]),
    );
  };
  search.addEventListener("input", load);
  await load();
  return h(
    "div",
    {},
    h("h1", {}, "Meetings"),
    section("Bibliotheek", "Elke meeting is een map met Markdown-bestanden.", h("div", { class: "row" }, search), list),
  );
}

async function meetingDetail(ctx: Context, id: string): Promise<HTMLElement> {
  const detail: MeetingDetail = await api.meetingDetail(id);
  if (!detail.info) {
    return h("div", {}, h("p", { class: "muted" }, "Meeting niet gevonden."));
  }
  const templates = await api.meetingTemplates();
  let template = ctx.info.settings.default_template;
  const summaryStatus = h("span", { class: "muted" });
  const summarize = async () => {
    summaryStatus.textContent = "Bezig…";
    try {
      await api.summarizeMeeting(id, template);
      ctx.rerender();
    } catch (e) {
      summaryStatus.textContent = `Mislukt: ${errorText(e)}`;
    }
  };
  const failed = detail.transcript.some((s) => s.failed);
  const title = h("input", {
    type: "text",
    value: detail.info.title,
    class: "grow",
    onchange: async () => {
      const newId = await api.renameMeeting(id, title.value);
      ctx.navigate("meetings", newId);
    },
  });
  return h(
    "div",
    {},
    h("div", { class: "row" }, h("button", { class: "btn", onclick: () => ctx.navigate("meetings") }, "← Meetings")),
    h("h1", { style: "margin-top:14px" }, detail.info.title),
    section(
      "Meeting",
      null,
      h("div", { class: "row" }, title, h("button", { class: "btn", onclick: () => api.openPath(detail.dir) }, "Open map")),
    ),
    section(
      detail.notes.trim() ? "Notities aanvullen" : "Samenvatting",
      null,
      h(
        "div",
        { class: "row" },
        select(
          templates.map((t) => [t.id, t.name] as [string, string]),
          template,
          (v) => (template = v),
        ),
        h("button", { class: "btn primary", onclick: summarize }, detail.summary ? "Opnieuw schrijven" : "Schrijven"),
        summaryStatus,
      ),
      detail.summary ? h("div", { class: "summary", style: "margin-top:12px" }, detail.summary) : null,
    ),
    detail.notes.trim() ? section("Je notities", null, h("div", { class: "summary" }, detail.notes)) : null,
    section(
      "Transcript",
      null,
      failed
        ? h(
            "button",
            {
              class: "btn",
              onclick: async () => {
                await api.retranscribeMeeting(id);
                ctx.rerender();
              },
            },
            "Mislukte stukken opnieuw transcriberen",
          )
        : null,
      transcript(detail.transcript, detail.speakers),
    ),
    h(
      "div",
      { class: "row" },
      h(
        "button",
        {
          class: "btn danger",
          onclick: async () => {
            if (!confirm("Deze meeting verwijderen?")) return;
            await api.deleteMeeting(id);
            ctx.navigate("meetings");
          },
        },
        "Meeting verwijderen",
      ),
    ),
  );
}
