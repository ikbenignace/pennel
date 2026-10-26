import type { MeetingTemplate, RecordAction } from "./api";
import { l, uiLocale, type LocalizedText } from "./i18n.ts";

const builtinTemplateKeys = {
  algemeen: "settings.template.general",
  "een-op-een": "settings.template.one_to_one",
  klantgesprek: "settings.template.client",
  standup: "settings.template.standup",
} as const;

export function templateDisplayName(template: Pick<MeetingTemplate, "id" | "name" | "builtin" | "customized">): string | LocalizedText {
  const key = builtinTemplateKeys[template.id as keyof typeof builtinTemplateKeys];
  return template.builtin && !template.customized && key ? l(key) : template.name;
}

export function localModelLanguageName(code: string): string {
  try {
    const name = new Intl.DisplayNames([uiLocale()], { type: "language" }).of(code) ?? code;
    return name.charAt(0).toLocaleUpperCase(uiLocale()) + name.slice(1);
  } catch {
    return code;
  }
}

export function recordActionOptions(): [RecordAction, string | LocalizedText][] {
  return [["ask", l("settings.action.ask")], ["always", l("settings.action.always")], ["never", l("settings.action.never")]];
}
