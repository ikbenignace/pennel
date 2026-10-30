import assert from 'node:assert/strict';
import test from 'node:test';
import { activeCleanupVerification, cleanupVerificationControl, cleanupVerificationCurrentState, cleanupVerificationDestination, cleanupVerificationDestinationLabel, cleanupVerificationReason } from '../../src/cleanup-verification.ts';
import { translate } from '../../src/localization.ts';

const status = (extra = {}) => ({ enabled: false, available: false, reason_code: 'disabled', provider_id: 'one', model: 'fixed', endpoint: 'https://selected.invalid/v1/decisions', ...extra });
const snapshot = (extra = {}) => ({ platform: 'ios', settings: { cleanup_verification_enabled: false, decision: { provider_id: 'one' } }, cleanup_verification: status(), dictation: { phase: 'idle' }, session: { phase: 'ready' }, ...extra });

test('a configured decision connection does not enable or qualify cleanup verification', () => {
  assert.deepEqual(cleanupVerificationControl(snapshot()), { enabled: false, disabled: true });
  assert.equal(cleanupVerificationReason(status({ reason_code: 'unqualified' })), 'dictation.verification.unqualified');
  assert.equal(cleanupVerificationReason(status({ reason_code: 'unconfigured' })), 'dictation.verification.unconfigured');
  assert.deepEqual(cleanupVerificationControl(null), { enabled: false, disabled: true });
});

test('saved enablement remains visible and can always be disabled when qualification is unavailable', () => {
  const saved = snapshot({ settings: { cleanup_verification_enabled: true }, cleanup_verification: status({ enabled: true, reason_code: 'unqualified' }) });
  assert.deepEqual(cleanupVerificationControl(saved), { enabled: true, disabled: false });
  saved.platform = 'android';
  assert.deepEqual(cleanupVerificationControl(saved), { enabled: true, disabled: true });
});

test('only current native qualification can admit an opt-in and mutation removes admission', () => {
  const state = snapshot({ cleanup_verification: status({ available: true }) });
  assert.deepEqual(cleanupVerificationControl(state), { enabled: false, disabled: false });
  state.cleanup_verification = status({ reason_code: 'mutation_in_progress' });
  assert.deepEqual(cleanupVerificationControl(state), { enabled: false, disabled: true });
  assert.equal(cleanupVerificationReason(state.cleanup_verification), 'dictation.verification.mutation_in_progress');
});

test('active status uses the frozen recorder or keyboard session rather than changed saved settings', () => {
  const frozen = status({ enabled: true, endpoint: 'https://original.invalid/v1/decisions' });
  const state = snapshot({ dictation: { phase: 'processing', cleanup_verification: frozen }, cleanup_verification: status({ endpoint: 'https://replacement.invalid/v1/decisions' }) });
  assert.equal(activeCleanupVerification(state), frozen);
  state.dictation = { phase: 'listening' };
  assert.equal(activeCleanupVerification(state), null);
  state.dictation = { phase: 'idle' };
  state.session = { phase: 'listening', cleanup_verification: frozen };
  assert.equal(activeCleanupVerification(state), frozen);
  state.session.phase = 'ready';
  assert.equal(activeCleanupVerification(state), null);
});

test('destination explains actual loopback processing and suppresses invalid or credential-bearing URLs', () => {
  for (const endpoint of ['http://127.0.0.1:8080/v1/decisions', 'http://localhost:8080/v1/decisions', 'http://[::1]:8080/v1/decisions']) {
    assert.deepEqual(cleanupVerificationDestination(status({ endpoint })), { endpoint, onDevice: true });
  }
  assert.deepEqual(cleanupVerificationDestination(status()), { endpoint: status().endpoint, onDevice: false });
  for (const endpoint of [null, 'unfinished', 'https://secret@server.invalid/v1', 'https://server.invalid/v1?key=secret', 'file:///private/example']) {
    assert.deepEqual(cleanupVerificationDestination(status({ endpoint })), { endpoint: null, onDevice: false });
  }
});

test('verification controls explain qualification and on-device processing in both interface languages', () => {
  for (const language of ['en', 'nl']) {
    for (const key of ['title', 'help', 'saved_enabled', 'saved_disabled', 'active', 'unavailable', 'unqualified', 'unconfigured', 'mutation_in_progress', 'configuration_changed', 'destination', 'none', 'fallback', 'current_processing']) {
      const full = `dictation.verification.${key}`;
      assert.notEqual(translate(language, full), full);
    }
    assert.match(translate(language, 'dictation.verification.current_iphone', { host: '127.0.0.1:8080' }), /iPhone.*127\.0\.0\.1:8080/);
  }
});

test('rendered settings and active-session copy interpolates every shared template in English and Dutch', () => {
  for (const language of ['en', 'nl']) {
    const t = (key, values) => translate(language, key, values);
    for (const selected of [status(), status({ endpoint: null }), status({ enabled: true, reason_code: 'unqualified' }), status({ enabled: true, reason_code: 'mutation_in_progress' }), status({ enabled: true, available: true, endpoint: 'http://127.0.0.1:8080/v1/decisions' })]) {
      const destination = cleanupVerificationDestinationLabel(selected, t);
      const current = cleanupVerificationCurrentState(selected, t);
      assert.doesNotMatch(destination, /\{(?:destination|host|path|state)\}/);
      assert.doesNotMatch(current, /\{(?:destination|host|path|state)\}/);
      assert.doesNotMatch(destination, /dictation\.verification\./);
      assert.doesNotMatch(current, /dictation\.verification\./);
    }
    assert.match(cleanupVerificationDestinationLabel(status(), t), /selected\.invalid/);
    assert.match(cleanupVerificationDestinationLabel(status({ endpoint: 'http://127.0.0.1:8080/v1/decisions' }), t), /iPhone.*127\.0\.0\.1:8080/);
    const fast = cleanupVerificationCurrentState(status({ enabled: true, reason_code: 'disabled' }), t);
    assert.ok(fast.includes(t('dictation.verification.inactive')));
    assert.ok(!fast.includes(t('dictation.verification.fallback')));
  }
});
