import { bindLocalization, getInterfaceLanguage, l, t, formatNumber, type UiText } from "../i18n.ts";
import { api, type AppInfo, type MeetingInfo, type MeetingStatus, type Passage } from "../api";
import { LatestRequest, SerialQueue } from "../async";
import { askText, confirmDialog, h, snippet } from "../dom";
import { meetingDate, dayGroup } from "../meeting-labels.ts";
export { meetingDate, dayGroup } from "../meeting-labels.ts";
import { icon } from "../icons";
import { badgeFor, resolveOpenMeeting, syncActive, type SyncItemState } from "../sync-badge";
import { workspaceRoute, type WorkspaceRoute } from "../workspace-state";

export interface LibraryHost {
  info: () => AppInfo;
  current: () => WorkspaceRoute;
  go: (route: WorkspaceRoute) => void;
  report: (error: unknown) => void;
}

/** The main window's sidebar, as in the original library: search, questions, live recording and the grouped meetings. */
export class LibrarySidebar {
  readonly element: HTMLElement;
  private readonly search: HTMLInputElement;
  private readonly ask: HTMLButtonElement;
  private readonly live: HTMLButtonElement;
  private readonly list: HTMLElement;
  private readonly requests = new LatestRequest();
  private readonly syncQueue = new SerialQueue();
  private query = "";
  private searchTimer: number | undefined;
  private syncOn = false;
  private syncStates = new Map<string, SyncItemState>();
  private shown: MeetingInfo[] = [];
  private rendered = "";
  private menu: HTMLElement | null = null;
  private menuAnchor: HTMLElement | null = null;

  constructor(private readonly host: LibraryHost) {
    this.search = h("input", { type: "search", placeholder: l("common.search"), "aria-label": l("meetings.search_label"), spellcheck: false });
    this.search.addEventListener("input", () => {
      this.query = this.search.value;
      window.clearTimeout(this.searchTimer);
      this.searchTimer = window.setTimeout(() => { void this.refresh().catch(host.report); }, 180);
    });
    this.ask = h("button", { class: "library-ask", onclick: () => host.go(workspaceRoute("meetings", "chat")) }, icon("chat"), h("span", {}, l("meetings.ask_all")));
    this.live = h("button", { class: "library-live", hidden: true, onclick: () => host.go(workspaceRoute("recording")) });
    this.list = h("div", { class: "meeting-list", role: "list" });
    this.element = h("aside", { class: "library-sidebar", "aria-label": l("meetings.library_label") },
      h("div", { class: "library-search" }, this.search), this.ask, this.live, this.list);
    document.addEventListener("pointerdown", (event) => { if (this.menu && !this.menu.contains(event.target as Node)) this.closeMenu(false); });
    document.addEventListener("keydown", (event) => { if (event.key === "Escape" && this.menu) { event.preventDefault(); this.closeMenu(); } });
    void api.onSyncChanged((report) => {
      void this.syncQueue.run(async () => {
        const wasOn = this.syncOn;
        this.syncOn = syncActive(this.host.info().settings.cloud_sync === true, report.status.kind);
        if (!this.syncOn && !wasOn) return;
        const route = this.host.current();
        const open = route.section === "meetings" ? route.meeting_id : null;
        const remembered = this.shown.find((meeting) => meeting.id === open) ?? null;
        await this.refresh();
        if (open && route === this.host.current()) {
          const resolved = resolveOpenMeeting(open, this.shown, remembered);
          if (resolved !== open) this.host.go(workspaceRoute("meetings", null, resolved ?? null));
        }
      }).catch(this.host.report);
    }).catch(() => undefined);
  }

  async start() {
    this.syncOn = this.host.info().settings.cloud_sync === true
      && syncActive(true, (await api.cloudSyncLast().catch(() => null))?.status.kind);
    await this.refresh();
  }

  async refresh() {
    const query = this.query.trim();
    const states = this.syncOn ? api.meetingSyncStates().catch(() => []) : Promise.resolve([]);
    await this.requests.run(async () => {
      const [result, sync, status] = await Promise.all([
        query ? api.searchLibrary(query) : api.meetings("").then((meetings) => ({ meetings, passages: [] as Passage[] })),
        states, api.meetingStatus(),
      ]);
      return { result, sync, status };
    }, ({ result, sync, status }) => {
      this.syncStates = new Map(sync.map((item) => [item.id, item.state]));
      this.shown = result.meetings;
      this.showLive(status);
      const key = JSON.stringify([getInterfaceLanguage(), query, result, sync]);
      if (key !== this.rendered) { this.rendered = key; this.draw(result.meetings, result.passages, query); }
      this.highlight();
    }, this.host.report);
  }

  async refreshLive() {
    this.showLive(await api.meetingStatus());
  }

  meetings(): MeetingInfo[] {
    return this.shown;
  }

  highlight() {
    const route = this.host.current();
    const chat = route.section === "meetings" && route.tab === "chat";
    this.ask.toggleAttribute("aria-current", chat);
    this.live.toggleAttribute("aria-current", route.section === "recording");
    for (const row of this.list.querySelectorAll<HTMLElement>(".meeting-row")) {
      const selected = !chat && route.section === "meetings" && row.dataset.id === route.meeting_id;
      row.classList.toggle("selected", selected);
      row.querySelector(".meeting-select")?.toggleAttribute("aria-current", selected);
    }
  }

  private showLive(status: MeetingStatus) {
    this.live.hidden = !status.busy;
    if (!status.busy) return;
    this.live.replaceChildren(h("span", { class: "status-dot" }), h("span", { class: "library-live-title" }, status.title ?? l("meetings.default_title")));
    if (status.phase === "finalizing") this.live.append(h("span", { class: "help" }, l("meetings.finishing")));
  }

  private draw(meetings: MeetingInfo[], passages: Passage[], query: string) {
    this.closeMenu();
    this.list.replaceChildren();
    if (!meetings.length) {
      this.list.append(h("p", { class: "library-empty help" }, query ? l("meetings.none_found") : l("meetings.empty_meetings")));
      return;
    }
    let group = "";
    for (const meeting of meetings) {
      const day = dayGroup(meeting.start_iso);
      if (day !== group) {
        group = day;
        const heading = h("h3", { class: "meeting-day" }, day);
        bindLocalization(heading, () => { heading.textContent = dayGroup(meeting.start_iso); });
        this.list.append(heading);
      }
      const state = this.syncStates.get(meeting.id);
      const badge = badgeFor(state, this.syncOn);
      const hits = passages.filter((passage) => passage.meeting_id === meeting.id).slice(0, 2);
      const source = hits[0];
      const metadataText = () => `${meetingDate(meeting.start_iso)}${meeting.duration_min ? ` · ${t("meetings.duration_minutes", { count: formatNumber(meeting.duration_min) })}` : ""}`;
      const metadata = h("span", { class: "meeting-meta" }, metadataText());
      bindLocalization(metadata, () => { metadata.textContent = metadataText(); });
      const syncBadge = badge ? h("span", { class: `sync-badge ${state}`, title: badge.label, role: "img", "aria-label": badge.label }, icon(badge.icon)) : null;
      if (syncBadge) bindLocalization(syncBadge, () => {
        const label = badgeFor(state, this.syncOn)?.label ?? "";
        syncBadge.title = label;
        syncBadge.setAttribute("aria-label", label);
      });
      const open = h("button", { class: "meeting-select", onclick: () => this.host.go(workspaceRoute("meetings", null, meeting.id, source?.start_ms ?? null,
        source ? { kind: source.kind, start_ms: source.start_ms, text: source.text } : undefined)) },
        h("span", { class: "meeting-name" }, h("span", { class: "meeting-title" }, meeting.title),
          syncBadge),
        metadata,
        hits.map((passage) => snippet(passage.snippet)));
      const more = h("button", { class: "meeting-more button-icon", "aria-label": l("meetings.actions_for", { title: meeting.title }), "aria-haspopup": "menu", "aria-expanded": "false" }, icon("more"));
      const row = h("div", { class: "meeting-row", role: "listitem", "data-id": meeting.id }, open, more);
      more.addEventListener("click", (event) => { event.stopPropagation(); this.openMenu(meeting, row, more.getBoundingClientRect(), more); });
      more.addEventListener("keydown", (event) => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault(); this.openMenu(meeting, row, more.getBoundingClientRect(), more);
          if (event.key === "ArrowUp") this.menu?.querySelectorAll<HTMLButtonElement>("button").item(2)?.focus();
        }
      });
      row.addEventListener("contextmenu", (event) => { event.preventDefault(); this.openMenu(meeting, row, new DOMRect(event.clientX, event.clientY, 0, 0), more); });
      open.addEventListener("keydown", (event) => {
        if (event.key === "F10" && event.shiftKey) { event.preventDefault(); this.openMenu(meeting, row, more.getBoundingClientRect(), more); }
      });
      this.list.append(row);
    }
  }

  private openMenu(meeting: MeetingInfo, row: HTMLElement, anchor: DOMRect, invoker: HTMLElement) {
    this.closeMenu(false);
    this.menuAnchor = invoker;
    invoker.setAttribute("aria-expanded", "true");
    const status = api.meetingStatus();
    const item = (label: UiText, action: () => Promise<void>, danger = false) => h("button", { role: "menuitem", class: danger ? "danger" : null, onclick: () => {
      this.closeMenu();
      void action().catch(this.host.report);
    } }, label);
    const menu = h("div", { class: "context-menu", role: "menu", "aria-label": l("meetings.actions_for", { title: meeting.title }) },
      item(l("meetings.rename_action"), async () => {
        const title = await askText(l("meetings.rename_title"), meeting.title);
        if (!title) return;
        const id = await api.renameMeeting(meeting.id, title);
        await this.refresh();
        this.host.go(workspaceRoute("meetings", null, id));
      }),
      item(this.host.info().platform === "macos" ? l("meetings.show_finder") : l("meetings.open_folder"), async () => { await api.openPath((await api.meetingDetail(meeting.id)).dir); }),
      h("hr"),
      item(l("meetings.delete_action"), async () => {
        if (!await confirmDialog(l("meetings.delete_title", { title: meeting.title }), l("meetings.delete_help"), l("meetings.delete_confirm"))) return;
        await api.deleteMeeting(meeting.id);
        const route = this.host.current();
        await this.refresh();
        if (route.section === "meetings" && route.meeting_id === meeting.id) this.host.go(workspaceRoute("meetings"));
      }, true));
    menu.addEventListener("keydown", (event) => {
      const entries = [...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
      const index = entries.indexOf(document.activeElement as HTMLButtonElement);
      if (event.key === "Tab") { this.closeMenu(); return; }
      const next = event.key === "ArrowDown" ? (index + 1) % entries.length : event.key === "ArrowUp" ? (index - 1 + entries.length) % entries.length
        : event.key === "Home" ? 0 : event.key === "End" ? entries.length - 1 : -1;
      if (next >= 0) { event.preventDefault(); entries[next]?.focus(); }
    });
    void status.then((current) => {
      if (current.busy && current.id === meeting.id) {
        menu.replaceChildren(h("p", { class: "help", role: "status" }, l("meetings.recording_active")));
        if (this.menu === menu) { menu.tabIndex = -1; menu.focus(); }
      }
    }).catch(() => undefined);
    row.classList.add("menu-open");
    document.body.append(menu);
    const bounds = menu.getBoundingClientRect();
    menu.style.left = `${Math.max(8, Math.min(anchor.left, innerWidth - bounds.width - 8))}px`;
    menu.style.top = `${Math.max(8, Math.min(anchor.bottom + 2, innerHeight - bounds.height - 8))}px`;
    this.menu = menu;
    menu.querySelector("button")?.focus();
  }

  private closeMenu(restoreFocus = true) {
    this.menu?.remove();
    this.menu = null;
    this.menuAnchor?.setAttribute("aria-expanded", "false");
    if (restoreFocus && this.menuAnchor?.isConnected) this.menuAnchor.focus();
    this.menuAnchor = null;
    this.list.querySelectorAll(".menu-open").forEach((row) => row.classList.remove("menu-open"));
  }
}
