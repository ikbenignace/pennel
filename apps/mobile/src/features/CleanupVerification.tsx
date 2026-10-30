import React from 'react';
import type { CleanupVerificationAvailability } from '../domain';
import { useRuntime } from '../runtime';
import { Body, Notice, Section, Toggle } from '../components/ui';
import { activeCleanupVerification, cleanupVerificationControl, cleanupVerificationCurrentState, cleanupVerificationDestinationLabel, cleanupVerificationReason } from '../cleanup-verification';

function VerificationDestination({ status }: { status?: CleanupVerificationAvailability | null }) {
  const { t } = useRuntime();
  return <Body muted>{cleanupVerificationDestinationLabel(status, t)}</Body>;
}

export function CleanupVerificationPanel() {
  const { snapshot, busy, run, t } = useRuntime();
  const status = snapshot?.cleanup_verification;
  const control = cleanupVerificationControl(snapshot);
  return <Section title={t('dictation.verification.title')} description={t('dictation.verification.help')}>
    <Toggle title={t('dictation.verification.title')} value={control.enabled} disabled={control.disabled || busy.has('save-cleanup-verification')} onChange={(cleanup_verification_enabled) => { void run('save-cleanup-verification', 'settings.update', { patch: { cleanup_verification_enabled } }); }} />
    <Body muted>{t(control.enabled ? 'dictation.verification.saved_enabled' : 'dictation.verification.saved_disabled')}</Body>
    <Notice>{t(control.enabled && status?.available ? 'dictation.verification.active' : 'dictation.verification.unavailable')}</Notice>
    {!status?.available && <Notice>{t(cleanupVerificationReason(status))}</Notice>}
    <VerificationDestination status={status} />
    <Notice>{t('dictation.verification.fallback')}</Notice>
  </Section>;
}

export function ActiveCleanupVerification() {
  const { snapshot, t } = useRuntime();
  const status = activeCleanupVerification(snapshot);
  if (!status) return null;
  return <>
    <Notice>{cleanupVerificationCurrentState(status, t)}</Notice>
    {status.enabled && status.reason_code !== 'disabled' && <VerificationDestination status={status} />}
  </>;
}
