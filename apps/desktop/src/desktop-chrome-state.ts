import tokens from "../../../assets/brand/tokens.json" with { type: "json" };

export const DESKTOP_COMMANDS = ["settings", "new_recording", "find", "toggle_sidebar", "dictation", "meetings", "history"] as const;
export type DesktopCommand = typeof DESKTOP_COMMANDS[number];
export interface DesktopCommandState extends Record<DesktopCommand, boolean> {
  ready: boolean;
  sidebar_visible: boolean;
  recording_active: boolean;
}

export interface DesktopEnvironment {
  material: "liquid_glass" | "vibrancy" | "opaque" | "unavailable";
  appearance: "light" | "dark";
  reduce_transparency: boolean;
  reduce_motion: boolean;
  increase_contrast: boolean;
  titlebar_height: number;
  titlebar_controls_width?: number;
  focused: boolean;
}

export function desktopEnvironment(value: unknown): DesktopEnvironment | null {
  if (!value || typeof value !== "object") return null;
  const environment = value as Record<string, unknown>;
  if (typeof environment.material !== "string" || !["liquid_glass", "vibrancy", "opaque", "unavailable"].includes(environment.material)
      || typeof environment.appearance !== "string" || !["light", "dark"].includes(environment.appearance)
      || !["reduce_transparency", "reduce_motion", "increase_contrast", "focused"].every((key) => typeof environment[key] === "boolean")
      || typeof environment.titlebar_height !== "number" || !Number.isFinite(environment.titlebar_height)
      || environment.titlebar_height < 0 || environment.titlebar_height > 128) return null;
  if (environment.titlebar_controls_width !== undefined && (typeof environment.titlebar_controls_width !== "number"
      || !Number.isFinite(environment.titlebar_controls_width) || environment.titlebar_controls_width < 0 || environment.titlebar_controls_width > 512)) return null;
  return environment as unknown as DesktopEnvironment;
}

export function nativeChromeIsTransparent(environment: DesktopEnvironment): boolean {
  return ["liquid_glass", "vibrancy"].includes(environment.material)
    && !environment.reduce_transparency && !environment.increase_contrast;
}

export interface SidebarPresentation { width: number; visible: boolean }
export const SIDEBAR = {
  width: tokens.desktop["sidebar-width"],
  min: tokens.desktop["sidebar-min-width"],
  max: tokens.desktop["sidebar-max-width"],
  step: tokens.spacing["1"],
} as const;

export function sidebarWidth(value: number): number {
  return Number.isFinite(value) ? Math.min(SIDEBAR.max, Math.max(SIDEBAR.min, Math.round(value))) : SIDEBAR.width;
}

export function sidebarPresentation(value: unknown): SidebarPresentation {
  const saved = value && typeof value === "object" ? value as Partial<SidebarPresentation> : {};
  return { width: typeof saved.width === "number" ? sidebarWidth(saved.width) : SIDEBAR.width, visible: saved.visible !== false };
}

export function resizedSidebarWidth(width: number, key: string): number | null {
  if (key === "Home") return SIDEBAR.min;
  if (key === "End") return SIDEBAR.max;
  if (key === "ArrowLeft") return sidebarWidth(width - SIDEBAR.step);
  if (key === "ArrowRight") return sidebarWidth(width + SIDEBAR.step);
  return null;
}

export function desktopCommand(value: unknown): DesktopCommand | null {
  const command = value && typeof value === "object" ? (value as { command?: unknown }).command : value;
  return DESKTOP_COMMANDS.includes(command as DesktopCommand) ? command as DesktopCommand : null;
}

type Shortcut = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "isComposing" | "repeat">;
export function desktopShortcut(event: Shortcut, platform: string): DesktopCommand | null {
  if (event.isComposing || event.repeat || event.altKey) return null;
  const key = event.key.toLowerCase();
  const mac = platform === "macos";
  if (key === "s" && (mac ? event.metaKey && event.ctrlKey && !event.shiftKey : event.ctrlKey && event.shiftKey && !event.metaKey)) return "toggle_sidebar";
  if (event.shiftKey || !(mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey)) return null;
  const commands: Partial<Record<string, DesktopCommand>> = { ",": "settings", n: "new_recording", f: "find", "1": "dictation", "2": "meetings", "3": "history" };
  return commands[key] ?? null;
}
