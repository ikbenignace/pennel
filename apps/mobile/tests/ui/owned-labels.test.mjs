import assert from 'node:assert/strict';
import test from 'node:test';
import { ownedMetadata, templateLabel } from '../../src/owned-labels.ts';

test('model sample outcomes use the selected interface language without translating provider content', () => {
  assert.equal(ownedMetadata('en', 'Het gekozen spraakmodel heeft het audiovoorbeeld verwerkt.', 'mobile.models.failed'), 'The selected speech model processed the audio sample.');
  assert.equal(ownedMetadata('nl', 'The selected speech model processed the audio sample.', 'mobile.models.failed'), 'Het gekozen spraakmodel heeft het audiovoorbeeld verwerkt.');
  assert.equal(ownedMetadata('en', 'Unrecognized provider output', 'mobile.models.failed').includes('Unrecognized provider output'), false);
});

test('recording metadata save failures keep the retry action in both interface languages', () => {
  const source = 'De opnamegegevens konden niet worden bewaard. Probeer opnieuw te bewaren.';
  assert.match(ownedMetadata('en', source, 'mobile.errors.operation_failed'), /recording.*sav/i);
  assert.equal(ownedMetadata('nl', source, 'mobile.errors.operation_failed'), source);
});

test('built-in template choices are translated by ID while custom names and IDs stay literal', () => {
  const original = { id: 'algemeen', name: 'Algemeen', builtin: true, customized: false };
  assert.equal(templateLabel(original, 'en'), 'General'); assert.equal(templateLabel(original, 'nl'), 'Algemeen');
  assert.equal(templateLabel({ ...original, id: 'een-op-een', name: '1-op-1' }, 'en'), 'One-to-one');
  assert.equal(templateLabel({ ...original, id: 'klantgesprek', name: 'Klantgesprek' }, 'en'), 'Client meeting');
  assert.equal(templateLabel({ ...original, name: 'My own agenda', customized: true }, 'nl'), 'My own agenda');
  assert.equal(templateLabel({ ...original, name: 'Custom General', builtin: false }, 'en'), 'Custom General');
  assert.equal(original.id, 'algemeen'); assert.equal(original.name, 'Algemeen');
});
