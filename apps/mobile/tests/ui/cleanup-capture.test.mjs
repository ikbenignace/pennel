import assert from 'node:assert/strict';
import test from 'node:test';
import { checkCleanupLabels } from '../e2e/check-cleanup-labels.mjs';
import { translate } from '../../src/localization.ts';
import { cleanupVerificationDestinationLabel } from '../../src/cleanup-verification.ts';

test('owned iPhone accessibility capture checker requires localized controls and fully formatted destination', () => {
  for (const language of ['en', 'nl']) {
    const t = (key, values) => translate(language, key, values);
    const labels = [t('dictation.verification.title'), t('dictation.verification.saved_disabled'), t('dictation.verification.unqualified'), cleanupVerificationDestinationLabel(null, t)];
    const capture = { elements: labels.map((label) => ({ label })) };
    assert.deepEqual(checkCleanupLabels(capture, language), { language, saved_enabled: false, labels_checked: labels.length, valid: true });
    assert.throws(() => checkCleanupLabels({ elements: [...capture.elements, { label: 'Current dictation: {path}' }] }, language), /unresolved/);
    assert.throws(() => checkCleanupLabels({ elements: capture.elements.slice(1) }, language), /missing/);
  }
});
