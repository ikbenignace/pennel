import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { dictionaries, mobileCopy, resolveLanguage, translate } from '../../src/localization.ts';

test('interface locale follows the first supported system language independently of speech', () => {
  assert.equal(resolveLanguage('system', ['fr-BE', 'nl-BE', 'en-GB']), 'nl');
  assert.equal(resolveLanguage('system', ['en-US', 'nl-BE']), 'en');
  assert.equal(resolveLanguage('system', ['fr-BE', 'de']), 'en');
  assert.equal(resolveLanguage('nl', ['en-GB']), 'nl');
});

test('mobile copy has matching translated keys and interpolation fields', () => {
  assert.deepEqual(Object.keys(mobileCopy.en).sort(), Object.keys(mobileCopy.nl).sort());
  for (const key of Object.keys(mobileCopy.en)) {
    const fields = (value) => [...value.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
    assert.deepEqual(fields(mobileCopy.en[key]), fields(mobileCopy.nl[key]), key);
    assert.ok(mobileCopy.en[key].trim()); assert.ok(mobileCopy.nl[key].trim());
  }
  assert.equal(translate('nl', 'mobile.session.ready', { time: '4:12' }), 'Klaar · nog 4:12');
});

test('every statically referenced UI label exists in both supported languages', async () => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const keys = new Set();
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) await walk(file);
      else if (entry.name.endsWith('.tsx')) {
        const source = await readFile(file, 'utf8');
        for (const match of source.matchAll(/\bt\('([^']+)'/g)) keys.add(match[1]);
      }
    }
  }
  await walk(path.join(root, 'app')); await walk(path.join(root, 'src'));
  for (const section of ['providers', 'models', 'dictation', 'meetings', 'recording', 'dictionary', 'styles', 'appearance', 'account']) keys.add(`navigation.${section}`);
  for (const stage of ['preparing', 'uploading', 'recognizing', 'response_unknown', 'interrupted', 'applying', 'completed', 'cancelled', 'failed']) keys.add(`mobile.speakers.${stage}`);
  for (const suffix of ['today', 'week', 'earlier']) keys.add(`meetings.group_${suffix}`);
  for (const key of ['settings.recording.detect_title', 'settings.calendar.title', 'dictation.processing.audio', 'settings.recording.echo']) keys.add(key);
  for (const key of keys) for (const language of ['en', 'nl']) assert.ok(dictionaries[language][key], `${language}: ${key}`);
});
