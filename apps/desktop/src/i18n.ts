import commonEn from "../../../assets/locales/common.en.json" with { type: "json" };
import commonNl from "../../../assets/locales/common.nl.json" with { type: "json" };
import dictationEn from "../../../assets/locales/dictation.en.json" with { type: "json" };
import dictationNl from "../../../assets/locales/dictation.nl.json" with { type: "json" };
import meetingsEn from "../../../assets/locales/meetings.en.json" with { type: "json" };
import meetingsNl from "../../../assets/locales/meetings.nl.json" with { type: "json" };
import providersEn from "../../../assets/locales/providers.en.json" with { type: "json" };
import providersNl from "../../../assets/locales/providers.nl.json" with { type: "json" };
import settingsEn from "../../../assets/locales/settings.en.json" with { type: "json" };
import settingsNl from "../../../assets/locales/settings.nl.json" with { type: "json" };
import onboardingEn from "../../../assets/locales/onboarding.en.json" with { type: "json" };
import onboardingNl from "../../../assets/locales/onboarding.nl.json" with { type: "json" };
import accountEn from "../../../assets/locales/account.en.json" with { type: "json" };
import accountNl from "../../../assets/locales/account.nl.json" with { type: "json" };
import backendEn from "../../../assets/locales/backend.en.json" with { type: "json" };
import backendNl from "../../../assets/locales/backend.nl.json" with { type: "json" };
import backendSources from "../../../assets/locales/backend.sources.json" with { type: "json" };

export type InterfaceLanguage = "en" | "nl";
export type InterfaceLanguagePreference = "system" | InterfaceLanguage;
const english = { ...commonEn, ...dictationEn, ...meetingsEn, ...providersEn, ...settingsEn, ...onboardingEn, ...accountEn, ...backendEn };
export type TranslationKey = keyof typeof english;
const dutch: Record<TranslationKey, string> = { ...commonNl, ...dictationNl, ...meetingsNl, ...providersNl, ...settingsNl, ...onboardingNl, ...accountNl, ...backendNl };
export const catalogs = { en: english, nl: dutch };
let language: InterfaceLanguage = "en";
type TranslationArguments = Record<string, string | number>;
type Binding = { update: () => void; epoch: number };
let localeEpoch = 0;
const bindings = new WeakMap<Node, Set<Binding>>();
const boundNodes = new Set<WeakRef<Node>>();

/** Presentation bindings retain their owner weakly; discarded views do not accumulate listeners. */
export function bindLocalization(owner: Node, update: () => void): () => void {
  let callbacks = bindings.get(owner);
  if (!callbacks) {
    callbacks = new Set();
    bindings.set(owner, callbacks);
    boundNodes.add(new WeakRef(owner));
  }
  const binding = { update, epoch: localeEpoch };
  callbacks.add(binding);
  return () => callbacks.delete(binding);
}

/** Async views can finish mounting after the language changed while they were detached. */
export function refreshLocalization(root?: Node): void {
  for (const reference of [...boundNodes]) {
    const owner = reference.deref();
    if (!owner) { boundNodes.delete(reference); continue; }
    if (!owner.isConnected || (root && owner !== root && !root.contains(owner))) continue;
    for (const binding of bindings.get(owner) ?? []) {
      if (binding.epoch === localeEpoch) continue;
      binding.epoch = localeEpoch;
      binding.update();
    }
  }
}

/** Explicit provenance lets DOM helpers relabel app text without inspecting user content. */
export class LiveText {
  readonly read: () => string;
  constructor(read: () => string) { this.read = read; }
  toString(): string { return this.read(); }
  get length(): number { return this.toString().length; }
}

export class LocalizedText extends LiveText {
  readonly key: TranslationKey;
  readonly args: TranslationArguments | (() => TranslationArguments);
  constructor(key: TranslationKey, args: TranslationArguments | (() => TranslationArguments) = {}) {
    super(() => t(key, typeof args === "function" ? args() : args));
    this.key = key;
    this.args = args;
  }
}

export type UiText = string | LiveText;
export function liveText(read: () => string): LiveText { return new LiveText(read); }
export function l(key: TranslationKey, args: TranslationArguments | (() => TranslationArguments) = {}): LocalizedText {
  return new LocalizedText(key, args);
}

export function setInterfaceLanguage(value: unknown): void {
  const next = value === "nl" ? "nl" : "en";
  const changed = next !== language;
  language = next;
  if (typeof document !== "undefined") document.documentElement.lang = language;
  if (!changed) return;
  localeEpoch += 1;
  refreshLocalization();
}

export function getInterfaceLanguage(): InterfaceLanguage { return language; }
export function uiLocale(): "en-GB" | "nl-BE" { return language === "nl" ? "nl-BE" : "en-GB"; }

export function t(key: TranslationKey, args: Record<string, string | number> = {}): string {
  return translated(key, args, language);
}

function translated(key: TranslationKey, args: TranslationArguments, target: InterfaceLanguage): string {
  const template = catalogs[target][key] ?? english[key] ?? key;
  return template.replace(/\{([a-zA-Z0-9_]+)\}/g, (match, name: string) =>
    Object.hasOwn(args, name) ? String(args[name]) : match);
}

export function formatDateTime(value: number | string | Date, options: Intl.DateTimeFormatOptions = { dateStyle: "medium", timeStyle: "short" }): string {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.valueOf()) ? String(value) : new Intl.DateTimeFormat(uiLocale(), options).format(date);
}

export function formatNumber(value: number, options: Intl.NumberFormatOptions = {}): string {
  return new Intl.NumberFormat(uiLocale(), options).format(value);
}

function captureTemplate(template: string, source: string): Record<string, string> | null {
  const parts = [...template.matchAll(/\{([a-zA-Z_][a-zA-Z0-9_]*)\}/g)];
  if (!parts.length) return template === source ? {} : null;
  if (!/\p{L}/u.test(template.replace(/\{[a-zA-Z_][a-zA-Z0-9_]*\}/g, ""))) return null;
  const prefix = template.slice(0, parts[0].index);
  if (!source.startsWith(prefix)) return null;
  let position = prefix.length;
  const values: Record<string, string> = {};
  for (const [index, part] of parts.entries()) {
    const nextPart = parts[index + 1];
    const suffix = template.slice(part.index! + part[0].length, nextPart?.index ?? template.length);
    let end: number;
    if (!nextPart) {
      if (!source.endsWith(suffix)) return null;
      end = source.length - suffix.length;
    } else {
      if (!suffix) return null;
      end = source.indexOf(suffix, position);
      if (end < 0 || source.indexOf(suffix, end + 1) >= 0) return null;
    }
    if (end < position) return null;
    const name = part[1];
    const value = source.slice(position, end);
    if (Object.hasOwn(values, name) && values[name] !== value) return null;
    values[name] = value;
    position = end + suffix.length;
  }
  return position === source.length ? values : null;
}

const backendTemplates = Object.entries(backendSources).flatMap(([key, source]) =>
  [...new Set([source, english[key as TranslationKey], dutch[key as TranslationKey]])]
    .filter((text): text is string => typeof text === "string")
    .map((text) => [key, text] as const)).sort((a, b) => b[1].length - a[1].length);

/** Only call for app-owned status/error channels, never dictated text, notes or other user content. */
export function localizeBackendText(source: string): string {
  return localizeBackendTextForLanguage(source, language);
}

export function localizeBackendTextForLanguage(source: string, target: InterfaceLanguage): string {
  if (source.length > 65_536) return source;
  for (const [key, template] of backendTemplates) {
    if (!Object.hasOwn(english, key)) continue;
    const args = captureTemplate(template, source);
    if (args) return translated(key as TranslationKey, args, target);
  }
  return source;
}
