import assert from 'node:assert/strict';
import test from 'node:test';
import { recordingPresentation, restoreOnboardingStep, speechReadiness, practiceSucceeded, summaryHasHeading } from '../../src/workflow-state.ts';
import { workflowCopy, workflowText } from '../../src/workflow-copy.ts';

test('an interruption is paused capture with an available stop action', () => {
  assert.deepEqual(recordingPresentation('interrupted'), { capturing: false, canStop: true, busy: false, status: 'paused' });
  assert.equal(recordingPresentation('recording').capturing, true);
  assert.equal(recordingPresentation('finishing').canStop, false);
  assert.equal(recordingPresentation('finishing').busy, true);
  assert.equal(recordingPresentation('idle', true).status, 'saveFailed');
});

test('saved first-use progress resumes supported task steps and defers old voice enrollment', () => {
  for (const step of ['engine', 'permissions', 'practice']) assert.equal(restoreOnboardingStep(step), step);
  assert.equal(restoreOnboardingStep('language'), 'welcome');
  assert.equal(restoreOnboardingStep('voice'), 'practice');
  assert.equal(restoreOnboardingStep('unexpected'), 'welcome');
});

test('task setup checks the selected task model, provider capability, availability and saved credential', () => {
  const settings = { dictation_stt: { provider_id: 'speech', model: 'configured' }, meeting_stt: { provider_id: '', model: '' }, providers: [{ id: 'speech', kind: 'openai' }] };
  const kinds = [{ kind: 'openai', stt: true, requires_key: true, availability: { status: 'available' } }];
  assert.equal(speechReadiness(settings, kinds, { speech: true }, 'dictation'), true);
  assert.equal(speechReadiness(settings, kinds, { speech: false }, 'dictation'), false);
  assert.equal(speechReadiness(settings, kinds, { speech: true }, 'meeting'), false);
  assert.equal(speechReadiness(settings, [{ ...kinds[0], stt: false }], { speech: true }, 'dictation'), false);
  assert.equal(speechReadiness(settings, [{ ...kinds[0], availability: { status: 'unsupported' } }], { speech: true }, 'dictation'), false);
});

test('practice requires a matching owned generation and nonempty successful final text', () => {
  const result = { phase: 'idle', text: 'A real result.', error: null, practice: true, result_session_id: 'owned' };
  assert.equal(practiceSucceeded('owned', result), true);
  assert.equal(practiceSucceeded(null, result), false);
  assert.equal(practiceSucceeded('other', result), false);
  assert.equal(practiceSucceeded('owned', { ...result, practice: false }), false);
  assert.equal(practiceSucceeded('owned', { ...result, session_id: 'different' }), false);
  assert.equal(practiceSucceeded('owned', { ...result, phase: 'processing' }), false);
  assert.equal(practiceSucceeded('owned', { ...result, text: '  ' }), false);
  assert.equal(practiceSucceeded('owned', { ...result, error: 'Failed' }), false);
});

test('a supplied Markdown heading is not repeated by an additional Summary heading', () => {
  assert.equal(summaryHasHeading('\n# Meeting outcome\n\nA result.'), true);
  assert.equal(summaryHasHeading('The useful outcome.'), false);
});

test('workflow copy remains complete in English and Dutch', () => {
  assert.deepEqual(Object.keys(workflowCopy.en).sort(), Object.keys(workflowCopy.nl).sort());
  for (const key of Object.keys(workflowCopy.en)) {
    assert.ok(workflowCopy.en[key].trim()); assert.ok(workflowCopy.nl[key].trim());
  }
  assert.equal(workflowText('nl', 'paused'), 'Opname onderbroken');
});
