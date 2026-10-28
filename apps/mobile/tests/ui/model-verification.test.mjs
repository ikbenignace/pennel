import assert from 'node:assert/strict';
import test from 'node:test';
import { ModelVerification, selectedVerification, verificationPayload } from '../../src/model-verification.ts';

const selection = (extra = {}) => ({ task: 'dictation_stt', provider_id: 'chosen-provider', model: 'speech-model', provider_kind: 'custom', base_url: 'https://example.invalid/v1', language: 'nl', ...extra });
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };

test('task verification explicitly requests real inference for the selected saved model', () => {
  assert.deepEqual(verificationPayload(selection()), { provider_id: 'chosen-provider', task: 'dictation_stt', expected_model: 'speech-model', verify_task: true });
  assert.equal(verificationPayload(selection({ task: 'meeting_stt' })).task, 'meeting_stt');
  assert.equal(verificationPayload(selection({ task: 'llm' })).task, 'llm');
});

test('provider availability alone cannot make an unconfigured or unauthenticated task ready to verify', () => {
  const snapshot = { settings: { dictation_stt: { provider_id: 'one', model: 'speech' }, language: 'nl', providers: [{ id: 'one', kind: 'custom', base_url: 'https://example.invalid' }] }, kinds: [{ kind: 'custom', stt: true, llm: false, requires_key: true, availability: { status: 'available' } }], keys: {}, cloud: { signed_in: false } };
  assert.equal(selectedVerification(snapshot, 'dictation_stt'), null);
  snapshot.keys.one = true;
  assert.equal(selectedVerification(snapshot, 'dictation_stt').model, 'speech');
  snapshot.settings.dictation_stt.model = '';
  assert.equal(selectedVerification(snapshot, 'dictation_stt'), null);
});

test('only an explicit verified result marks a model test as passed', async () => {
  const operation = deferred(); const verification = new ModelVerification(() => operation.promise);
  verification.configure(selection()); const result = verification.verify();
  assert.equal(verification.getSnapshot().running, true); assert.equal(verification.getSnapshot().result, null);
  operation.resolve({ ok: false, message: 'The sample returned no text.' }); await result;
  assert.equal(verification.getSnapshot().running, false); assert.equal(verification.getSnapshot().result.ok, false);
  const successful = new ModelVerification(async () => ({ ok: true, message: 'The sample was processed.' }));
  successful.configure(selection()); await successful.verify(); assert.equal(successful.getSnapshot().result.ok, true);
});

test('a result cannot verify a replacement model or another provider', async () => {
  for (const changed of [{ model: 'replacement' }, { provider_id: 'different' }, { base_url: 'https://another.invalid/v1' }, { language: 'en' }]) {
    const operation = deferred(); const verification = new ModelVerification(() => operation.promise);
    verification.configure(selection()); const result = verification.verify();
    verification.configure(selection(changed));
    assert.equal(verification.getSnapshot().changed, true);
    operation.resolve({ ok: true, message: 'The old selection was processed.' }); await result;
    assert.equal(verification.getSnapshot().result, null);
    assert.equal(verification.getSnapshot().running, false);
  }
});

test('leaving the screen ignores late results without claiming to cancel native work', async () => {
  const operation = deferred(); let calls = 0;
  const verification = new ModelVerification(() => { calls += 1; return operation.promise; });
  verification.configure(selection()); const first = verification.verify(); const duplicate = verification.verify();
  verification.invalidate(); assert.equal(verification.getSnapshot().running, true);
  operation.resolve({ ok: true, message: 'Finished.' }); await Promise.all([first, duplicate]);
  assert.equal(calls, 1); assert.equal(verification.getSnapshot().result, null);
});

test('native failures retain structured actionable status and never become verified', async () => {
  const verification = new ModelVerification(async () => { throw { code: 'ERR_NATIVE', message: JSON.stringify({ code: 'provider_changed', message: 'The selected model changed.' }) }; });
  verification.configure(selection()); await verification.verify();
  assert.equal(verification.getSnapshot().running, false); assert.equal(verification.getSnapshot().result, null);
  assert.equal(verification.getSnapshot().error.code, 'provider_changed');
});
