import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

function strings(value) {
  if (typeof value === 'string') return [value];
  if (Array.isArray(value)) return value.flatMap(strings);
  if (value && typeof value === 'object') return Object.values(value).flatMap(strings);
  return [];
}

export function checkCleanupLabels(capture, language, enabled = false) {
  if (!['en', 'nl'].includes(language)) throw new Error('Choose English or Dutch for the owned UI capture.');
  const copy = JSON.parse(readFileSync(new URL(`../../../../assets/locales/dictation.${language}.json`, import.meta.url), 'utf8'));
  const labels = strings(capture);
  const text = labels.join('\n');
  if (/\{(?:destination|host|path|state)\}/.test(text)) throw new Error('The capture contains an unresolved verification template.');
  if (text.includes('dictation.verification.')) throw new Error('The capture contains an untranslated verification key.');
  for (const key of ['title', enabled ? 'saved_enabled' : 'saved_disabled']) {
    if (!text.includes(copy[`dictation.verification.${key}`])) throw new Error('The capture is missing the expected verification controls.');
  }
  const destinationPrefix = copy['dictation.verification.destination'].split('{destination}')[0];
  if (!text.includes(destinationPrefix)) throw new Error('The capture is missing its verification destination explanation.');
  if (!['unqualified', 'unconfigured', 'mutation_in_progress', 'configuration_changed'].some((key) => text.includes(copy[`dictation.verification.${key}`]))) {
    throw new Error('The capture is missing the verification availability explanation.');
  }
  return { language, saved_enabled: enabled, labels_checked: labels.length, valid: true };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { values } = parseArgs({ options: { file: { type: 'string' }, language: { type: 'string' }, enabled: { type: 'boolean', default: false } } });
  if (!values.file) throw new Error('Pass --file with the owned settings screen accessibility capture.');
  const source = readFileSync(values.file, 'utf8');
  let capture;
  try { capture = JSON.parse(source); } catch { capture = source; }
  console.log(JSON.stringify(checkCleanupLabels(capture, values.language, values.enabled)));
}
