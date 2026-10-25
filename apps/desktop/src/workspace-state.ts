export type WorkspaceSection = "dictation" | "meetings" | "history" | "settings" | "recording";
export interface WorkspaceRoute {
  section: WorkspaceSection;
  tab: string | null;
  meeting_id: string | null;
  jump_ms: number | null;
}

/** The original Pennel windows: the meeting library is the main window; the rest open in their own window. */
export type WindowMode = "workspace" | "settings" | "history" | "dictation";
export const windowTitles: Record<WindowMode, string> = { workspace: "Pennel", settings: "Instellingen", history: "Geschiedenis", dictation: "Dicteren" };

export function windowForSection(section: WorkspaceSection): WindowMode {
  return section === "meetings" || section === "recording" ? "workspace" : section;
}

export function windowMode(value: string | null): WindowMode {
  return value === "settings" || value === "history" || value === "dictation" ? value : "workspace";
}

export const settingTabs = ["providers", "models", "dictation", "meeting-settings", "recording", "dictionary", "styles", "appearance", "account"] as const;
const settings = new Set<string>([...settingTabs, "local"]);
const reserved = new Set<string>([...settings, "workspace", "settings", "history", "meeting", "live", "onboarding", "prompt", "record-prompt", "pill", "library", "meetings", "chat"]);

export function validMeetingId(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0 && value !== "." && value !== ".."
    && !reserved.has(value) && !/[\u0000-\u001f\u007f-\u009f/\\:]/u.test(value);
}

export function workspaceRoute(section: WorkspaceSection, tab: string | null = null, meetingId: string | null = null, jumpMs: number | null = null): WorkspaceRoute {
  return { section, tab, meeting_id: meetingId, jump_ms: jumpMs };
}

export function parseWorkspaceRoute(value: unknown): WorkspaceRoute | null {
  if (!value || typeof value !== "object") return null;
  const input = value as Record<string, unknown>;
  if (!["dictation", "meetings", "history", "settings", "recording"].includes(String(input.section))) return null;
  let section = input.section as WorkspaceSection;
  const tab = typeof input.tab === "string" ? input.tab : null;
  if (section === "meetings" && tab === "recording") section = "recording";
  if (section === "settings" && tab !== null && !settings.has(tab) && tab !== "meetings") return null;
  if (input.meeting_id != null && !validMeetingId(input.meeting_id)) return null;
  const id = validMeetingId(input.meeting_id) ? input.meeting_id : null;
  const jump = typeof input.jump_ms === "number" && Number.isFinite(input.jump_ms) && input.jump_ms >= 0 ? input.jump_ms : null;
  return workspaceRoute(section, section === "settings" && tab === "meetings" ? "meeting-settings" : tab, id, jump);
}

export function legacyWorkspaceRoute(kind: string, route?: string | null): WorkspaceRoute | null {
  if (kind === "settings") return workspaceRoute("settings", route === "meetings" ? "meeting-settings" : settings.has(route ?? "") ? route! : "providers");
  if (kind === "history") return workspaceRoute("history");
  if (kind === "dictation") return workspaceRoute("dictation", route === "pad" ? "pad" : null);
  if (kind === "live" || kind === "meeting") return workspaceRoute("recording", null, validMeetingId(route) ? route : null);
  if (kind === "library" || kind === "meetings") return workspaceRoute("meetings", route === "chat" ? "chat" : null, validMeetingId(route) ? route : null);
  if (kind === "workspace") {
    if (!route) return workspaceRoute("meetings");
    if (route === "history") return workspaceRoute("history");
    if (route === "meeting" || route === "live") return workspaceRoute("recording");
    if (route === "chat") return workspaceRoute("meetings", "chat");
    if (route === "meetings" || route === "library") return workspaceRoute("meetings");
    if (settings.has(route)) return workspaceRoute("settings", route);
    return validMeetingId(route) ? workspaceRoute("meetings", null, route) : null;
  }
  return null;
}

export function routeKey(route: WorkspaceRoute): string {
  return JSON.stringify([route.section, route.tab, route.meeting_id, route.jump_ms]);
}

export function samePadRoute(current: WorkspaceRoute, next: WorkspaceRoute): boolean {
  return current.section === "dictation" && next.section === "dictation" && routeKey(current) === routeKey(next);
}

export class WorkspaceNavigation {
  current: WorkspaceRoute;
  constructor(initial: WorkspaceRoute = workspaceRoute("meetings")) { this.current = { ...initial }; }
  private readonly remembered = new Map<WorkspaceSection, WorkspaceRoute>();

  set(route: WorkspaceRoute): void {
    this.current = { ...route };
    if (route.section !== "recording") this.remembered.set(route.section, { ...route, jump_ms: null });
  }

  select(section: WorkspaceSection): WorkspaceRoute {
    return this.remembered.get(section) ?? workspaceRoute(section, section === "settings" ? "providers" : null);
  }
}
