import assert from 'node:assert/strict';
import test from 'node:test';
import { PracticeSession } from '../../src/practice-session.ts';

const dictation = (id, extra = {}) => ({ phase: 'listening', text: '', error: null, practice: true, session_id: id, result_session_id: null, ...extra });

test('practice cannot adopt an existing normal dictation or a previously observed practice generation', () => {
  const session = new PracticeSession();
  const first = session.begin(dictation('normal', { practice: false }));
  assert.equal(session.acceptStart(first, dictation('normal', { practice: false })), false);
  assert.equal(session.owns(dictation('normal', { practice: false })), false);
  const second = session.begin(dictation('previous-practice', { phase: 'idle' }));
  assert.equal(session.acceptStart(second, dictation('previous-practice')), false);
  assert.equal(session.getSnapshot().sessionId, null);
});

test('stop and cancellation target only the accepted practice session', () => {
  const session = new PracticeSession();
  const attempt = session.begin(dictation(null, { phase: 'idle', practice: false }));
  assert.equal(session.acceptStart(attempt, dictation('owned')), true);
  assert.deepEqual(session.command(dictation('owned')), { session_id: 'owned' });
  assert.equal(session.command(dictation('other')), null);
  assert.equal(session.command(dictation('owned', { practice: false })), null);
  assert.equal(session.command(dictation('owned', { phase: 'idle' })), null);
});

test('an unrelated later result cannot complete onboarding or replace the owned practice result', () => {
  const session = new PracticeSession();
  const attempt = session.begin(dictation(null, { phase: 'idle' }));
  session.acceptStart(attempt, dictation('owned'));
  session.observe(dictation('normal', { phase: 'idle', practice: false, result_session_id: 'normal', text: 'Unrelated text.' }));
  assert.equal(session.getSnapshot().result, null);
  session.observe(dictation('owned', { phase: 'idle', result_session_id: 'different', text: 'A stale result.' }));
  assert.equal(session.getSnapshot().result, null);
  session.observe(dictation('owned', { phase: 'idle', result_session_id: 'owned', text: 'My actual practice.' }));
  assert.equal(session.getSnapshot().result, 'My actual practice.');
  session.observe(dictation('normal', { phase: 'idle', practice: false, result_session_id: 'normal', text: 'A later normal dictation.' }));
  assert.equal(session.getSnapshot().result, 'My actual practice.');
});

test('pending, failed, empty and provisional practice results do not count as completion', () => {
  const session = new PracticeSession();
  const attempt = session.begin(dictation(null, { phase: 'idle' }));
  session.acceptStart(attempt, dictation('owned'));
  for (const extra of [
    { phase: 'processing', result_session_id: 'owned', text: 'Provisional.' },
    { phase: 'idle', result_session_id: 'owned', text: '  ' },
    { phase: 'idle', result_session_id: 'owned', text: 'Partial.', error: 'The request failed.' },
  ]) session.observe(dictation('owned', extra));
  assert.equal(session.getSnapshot().result, null);
});

test('previous practice can be explicitly cancelled without adopting it or exposing a normal-session cancel', () => {
  const session = new PracticeSession();
  assert.deepEqual(session.recoveryCommand(dictation('previous')), { session_id: 'previous' });
  assert.equal(session.recoveryCommand(dictation('normal', { practice: false })), null);
  assert.equal(session.recoveryCommand(dictation('previous', { phase: 'idle' })), null);
  session.observe(dictation('previous', { phase: 'idle', result_session_id: 'previous', text: 'An old result.' }));
  assert.equal(session.getSnapshot().result, null);
  const attempt = session.begin(dictation(null, { phase: 'idle' }));
  assert.equal(session.recoveryCommand(dictation('pending')), null);
  session.acceptStart(attempt, dictation('owned'));
  assert.equal(session.recoveryCommand(dictation('owned')), null);
});

test('abandoned or superseded starts cannot acquire ownership after their response arrives', () => {
  const session = new PracticeSession();
  const old = session.begin(dictation(null, { phase: 'idle' }));
  session.abandon();
  assert.equal(session.acceptStart(old, dictation('abandoned')), false);
  const current = session.begin(dictation(null, { phase: 'idle' }));
  assert.equal(session.acceptStart(old, dictation('abandoned')), false);
  assert.equal(session.getSnapshot().pending, true);
  assert.equal(session.acceptStart(current, dictation('current')), true);
  assert.equal(session.getSnapshot().sessionId, 'current');
});

test('restarting practice clears the old success and a failed start leaves no owned session', () => {
  const session = new PracticeSession();
  session.acceptStart(session.begin(dictation(null, { phase: 'idle' })), dictation('one'));
  session.observe(dictation('one', { phase: 'idle', result_session_id: 'one', text: 'First result.' }));
  const attempt = session.begin(dictation('one', { phase: 'idle' }));
  assert.equal(session.getSnapshot().result, null);
  session.rejectStart(attempt);
  assert.equal(session.getSnapshot().pending, false);
  assert.equal(session.getSnapshot().sessionId, null);
});
