import assert from 'node:assert/strict';
import test from 'node:test';
import { localizedFailure, nativeFailure } from '../../src/errors.ts';
import { recognitionActions } from '../../src/format.ts';

test('native NSError envelopes retain diagnostic codes while English UI replaces Dutch backend literals', () => {
  const diagnostic = 'De opname is ondertussen gewijzigd.';
  const rejected = { code: 'ERR_NATIVE', message: `Call to function PennelMobile.invoke was rejected.\nCaused by: ${JSON.stringify({ code: 'speaker_stale', message: diagnostic })}` };
  const error = nativeFailure(rejected);
  assert.equal(error.code, 'speaker_stale'); assert.equal(error.message, diagnostic);
  assert.match(localizedFailure('en', error), /Start recognition again/);
  assert.equal(localizedFailure('en', error).includes(diagnostic), false);
  assert.match(localizedFailure('nl', error), /Start de herkenning opnieuw/);
});

test('every new recognition error has actionable EN/NL text independent of backend message', () => {
  for (const code of ['speaker_stale', 'speaker_checkpoint_version', 'speaker_checkpoint_invalid', 'speaker_retry_confirmation', 'speaker_cleanup', 'speaker_model', 'quota', 'auth', 'signed_out', 'storage', 'network', 'cancelled', 'interrupted', 'recognition']) {
    const original = { code, message: 'Backenddetails die niet in Engelse tekst thuishoren.' };
    const en = localizedFailure('en', original); const nl = localizedFailure('nl', original);
    assert.ok(en.length > 15, code); assert.ok(nl.length > 15, code);
    assert.notEqual(en, nl, code); assert.equal(en.includes('Backenddetails'), false, code);
    assert.equal(en.startsWith('mobile.errors.'), false, code);
    assert.deepEqual(original, { code, message: 'Backenddetails die niet in Engelse tekst thuishoren.' });
  }
});

test('structured errors can be recovered from NSError metadata without swallowing quoted braces', () => {
  const embedded = JSON.stringify({ code: 'stale_notes', message: 'De tekst {naam} is veranderd.' });
  const error = nativeFailure({ code: 'ERR_NATIVE', message: `Error Domain=PennelMobile Code=1 "${embedded}" UserInfo={NSLocalizedDescription=${embedded}}` });
  assert.equal(error.code, 'stale_notes');
  assert.equal(error.message, 'De tekst {naam} is veranderd.');
  assert.match(localizedFailure('en', error), /Reload/);
});

test('stale-note codes instruct reload instead of generic storage retry', () => {
  for (const code of ['stale_notes', 'notes_stale']) {
    assert.match(localizedFailure('en', { code, message: 'De notities zijn gewijzigd.' }), /Reload/);
    assert.match(localizedFailure('nl', { code, message: 'De notities zijn gewijzigd.' }), /Laad ze opnieuw/);
  }
});

test('unknown provider messages remain diagnostics and cannot become raw UI content', () => {
  const message = 'Potential provider content';
  const error = nativeFailure({ code: 'unrecognized_provider_code', message });
  assert.equal(error.message, message);
  assert.equal(localizedFailure('en', error).includes(message), false);
  assert.equal(localizedFailure('nl', error).includes(message), false);
});

test('saved interrupted work can be discarded and cleanup failures offer removal again', () => {
  assert.equal(recognitionActions({ stage: 'interrupted', recoverable: true, error: null }).cancel, true);
  assert.equal(recognitionActions({ stage: 'failed', recoverable: false, error: { code: 'speaker_cleanup' } }).cancel, true);
  assert.equal(recognitionActions({ stage: 'applying', recoverable: false, error: null }).cancel, false);
});
