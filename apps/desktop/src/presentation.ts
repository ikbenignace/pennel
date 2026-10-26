import { localizeBackendText, t, uiLocale } from "./i18n.ts";
import type { AppInfo, AppearancePreference, HotkeyTrigger } from "./api";

export function appearanceIsDark(preference: AppearancePreference, systemDark: boolean): boolean {
  return preference === "dark" || (preference === "system" && systemDark);
}

function canonicalJson(value: unknown): string {
  const ordered = (item: unknown): unknown => Array.isArray(item) ? item.map(ordered)
    : item && typeof item === "object" ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b)).map(([key, entry]) => [key, ordered(entry)])) : item;
  return JSON.stringify(ordered(value));
}

export function appInfoContentEqual(previous: AppInfo, next: AppInfo): boolean {
  const message = (value: string | null | undefined) => value ? localizeBackendText(value) : value;
  const content = (info: AppInfo) => ({ ...info,
    interface_language: "en", system_interface_language: "en",
    settings: { ...info.settings, appearance: "system", ui_language: "system" },
    hotkey_error: message(info.hotkey_error), migration_warning: message(info.migration_warning),
    kinds: info.kinds?.map((kind) => ({ ...kind, label: message(kind.label), availability: { ...kind.availability, reason: message(kind.availability.reason) } })),
    capabilities: info.capabilities && Object.fromEntries(Object.entries(info.capabilities).map(([key, capability]) => [key, { ...capability, reason: message(capability.reason) }])),
  });
  return canonicalJson(content(previous)) === canonicalJson(content(next));
}

export function hotkeyLabel(key: HotkeyTrigger, platform: string): string {
  return ({ fn: platform === "macos" ? "fn" : t("dictation.shortcut.right_control"), right_control: t("dictation.shortcut.right_control"), right_option: platform === "macos" ? t("dictation.shortcut.right_option") : t("dictation.shortcut.right_alt"), right_command: platform === "macos" ? t("dictation.shortcut.right_command") : t("dictation.shortcut.right_windows"), control_super: platform === "windows" ? "Ctrl + Windows" : "Ctrl + Super" })[key];
}

export function hotkeyKeyLabel(key: HotkeyTrigger, platform: string): string {
  return ({ fn: platform === "macos" ? "fn" : "Ctrl", right_control: "Ctrl", right_option: platform === "macos" ? "⌥" : "Alt", right_command: platform === "macos" ? "⌘" : "⊞", control_super: platform === "windows" ? "Ctrl+⊞" : "Ctrl+Super" })[key];
}

export function applicationLabel(id: string): string {
  const known: Record<string, string> = { "com.apple.mail": "Mail", "com.apple.MobileSMS": t("dictation.application.messages"), "com.apple.Notes": t("dictation.application.notes"), "com.apple.Terminal": "Terminal", "com.apple.Safari": "Safari", "com.google.Chrome": "Google Chrome", "com.microsoft.VSCode": "Visual Studio Code", "com.microsoft.Outlook": "Outlook", "com.tinyspeck.slackmacgap": "Slack", "com.anthropic.claudefordesktop": "Claude", "com.openai.chat": "ChatGPT", "com.microsoft.teams2": "Microsoft Teams", "us.zoom.xos": "Zoom", "com.apple.FaceTime": "FaceTime", "net.whatsapp.WhatsApp": "WhatsApp", "Cisco-Systems.Spark": "Webex", "com.hnc.Discord": "Discord", "company.thebrowser.Browser": "Arc", "com.microsoft.edgemac": "Microsoft Edge", "org.mozilla.firefox": "Firefox", "com.brave.Browser": "Brave", "ru.keepcoder.Telegram": "Telegram", "com.microsoft.Word": "Word", "com.googlecode.iterm2": "iTerm", "com.mitchellh.ghostty": "Ghostty", "dev.zed.Zed": "Zed", "com.todesktop.230313mzl4w4u92": "Cursor" };
  if (/pennel|musli/i.test(id)) return "Pennel";
  if (known[id]) return known[id];
  const name = id.replace(/\.exe$/i, "").split(".").filter(Boolean).at(-1) ?? id;
  return name.charAt(0).toLocaleUpperCase(uiLocale()) + name.slice(1);
}

export function languageModelConfigured(info: AppInfo): boolean {
  const choice = info.settings.llm;
  const account = info.settings.providers.find((provider) => provider.id === choice.provider_id);
  const kind = info.kinds.find((candidate) => candidate.kind === account?.kind);
  if (!account || !kind?.llm || kind.availability.status !== "available") return false;
  if (kind.is_cli_agent) return !!account.base_url && (!kind.requires_key || !!info.keys[account.id]);
  return !!choice.model.trim() && (!kind.requires_key || !!info.keys[account.id]);
}
