import type { CleanupVerificationAvailability, RuntimeSnapshot } from './domain.ts';

type VerificationTranslate = (key: string, values?: Record<string, string | number>) => string;

export function cleanupVerificationControl(snapshot: RuntimeSnapshot | null): { enabled: boolean; disabled: boolean } {
  const enabled = snapshot?.settings.cleanup_verification_enabled === true;
  return { enabled, disabled: snapshot?.platform !== 'ios' || (!enabled && snapshot?.cleanup_verification?.available !== true) };
}

export function activeCleanupVerification(snapshot: RuntimeSnapshot | null): CleanupVerificationAvailability | null {
  if (snapshot?.dictation.phase === 'listening' || snapshot?.dictation.phase === 'processing') return snapshot.dictation.cleanup_verification ?? null;
  if (snapshot?.session.phase === 'listening' || snapshot?.session.phase === 'processing') return snapshot.session.cleanup_verification ?? null;
  return null;
}

export function cleanupVerificationReason(status?: CleanupVerificationAvailability | null): string {
  if (status?.available) return 'dictation.verification.active';
  switch (status?.reason_code) {
    case 'unconfigured': return 'dictation.verification.unconfigured';
    case 'mutation_in_progress': return 'dictation.verification.mutation_in_progress';
    case 'configuration_changed': return 'dictation.verification.configuration_changed';
    default: return 'dictation.verification.unqualified';
  }
}

export function cleanupVerificationDestination(status?: CleanupVerificationAvailability | null): { endpoint: string | null; onDevice: boolean } {
  try {
    const endpoint = status?.endpoint;
    if (!endpoint) return { endpoint: null, onDevice: false };
    const url = new URL(endpoint);
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) return { endpoint: null, onDevice: false };
    const host = url.hostname.toLowerCase().replace(/\.$/, '');
    const ipv4 = host.split('.');
    const onDevice = host === 'localhost' || host === '[::1]' || (ipv4.length === 4 && ipv4[0] === '127' && ipv4.every((part) => /^\d+$/.test(part) && Number(part) <= 255));
    if (url.protocol === 'http:' && !onDevice) return { endpoint: null, onDevice: false };
    return { endpoint, onDevice };
  } catch { return { endpoint: null, onDevice: false }; }
}

function destinationText(status: CleanupVerificationAvailability | null | undefined, t: VerificationTranslate): string {
  const destination = cleanupVerificationDestination(status);
  if (!destination.endpoint) return t('dictation.verification.none');
  return destination.onDevice ? t('dictation.verification.current_iphone', { host: new URL(destination.endpoint).host }) : destination.endpoint;
}

export function cleanupVerificationDestinationLabel(status: CleanupVerificationAvailability | null | undefined, t: VerificationTranslate): string {
  return t('dictation.verification.destination', { destination: destinationText(status, t) });
}

export function cleanupVerificationCurrentState(status: CleanupVerificationAvailability, t: VerificationTranslate): string {
  const state = !status.enabled || status.reason_code === 'disabled' ? t('dictation.verification.inactive') : status.available
    ? destinationText(status, t) : `${t('dictation.verification.fallback')}. ${t(cleanupVerificationReason(status))}`;
  return t('dictation.verification.current_state', { state });
}
