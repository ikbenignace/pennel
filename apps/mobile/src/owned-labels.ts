import sources from '../../../assets/locales/backend.sources.json' with { type: 'json' };
import english from '../../../assets/locales/backend.en.json' with { type: 'json' };
import dutch from '../../../assets/locales/backend.nl.json' with { type: 'json' };
import type { InterfaceLanguage, MeetingTemplate } from './domain.ts';
import { translate } from './localization.ts';

const catalogs: Record<InterfaceLanguage, Record<string, string>> = { en: english, nl: dutch };
const metadataKeys = new Map<string, string>();
for (const catalog of [sources, english, dutch]) for (const [key, text] of Object.entries(catalog)) metadataKeys.set(text, key);

// Only fixed UI metadata enters this channel. Meeting content and provider output stay literal.
export function ownedMetadata(language: InterfaceLanguage, text: string, fallbackKey: string): string {
  const key = metadataKeys.get(text);
  return key ? catalogs[language][key] ?? translate(language, fallbackKey) : translate(language, fallbackKey);
}

const builtins: Record<string, string> = { algemeen: 'general', 'een-op-een': 'one_to_one', klantgesprek: 'client', standup: 'standup' };
export function templateLabel(template: Pick<MeetingTemplate, 'id' | 'name' | 'builtin' | 'customized'>, language: InterfaceLanguage): string {
  const key = builtins[template.id];
  return template.builtin && !template.customized && key ? translate(language, `meetings.template.${key}`) : template.name;
}
