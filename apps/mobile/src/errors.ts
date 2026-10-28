import type { InterfaceLanguage, NativeFailure } from './domain.ts';
import { translate } from './localization.ts';

function envelope(message: string): NativeFailure | null {
  const text = message.slice(0, 32_768);
  let candidates = 0;
  for (let start = text.indexOf('{'); start >= 0 && candidates++ < 32; start = text.indexOf('{', start + 1)) {
    let depth = 0; let quoted = false; let escaped = false;
    for (let end = start; end < text.length; end += 1) {
      const character = text[end];
      if (quoted) { if (escaped) escaped = false; else if (character === '\\') escaped = true; else if (character === '"') quoted = false; continue; }
      if (character === '"') { quoted = true; continue; }
      if (character === '{') depth += 1;
      if (character !== '}' || --depth !== 0) continue;
      try {
        const value: unknown = JSON.parse(text.slice(start, end + 1));
        if (!value || typeof value !== 'object' || !('code' in value) || !('message' in value)) break;
        const data = value as { code: unknown; message: unknown; recoverable?: unknown };
        if (typeof data.code !== 'string' || !/^[a-z][a-z0-9_]*$/.test(data.code) || typeof data.message !== 'string') break;
        return { code: data.code, message: data.message, recoverable: data.recoverable === true };
      } catch { break; }
    }
  }
  return null;
}

export function nativeFailure(error: unknown): NativeFailure {
  if (error && typeof error === 'object' && 'message' in error) {
    const value = error as { code?: unknown; message: unknown; recoverable?: unknown };
    const message = String(value.message);
    return envelope(message) ?? { code: typeof value.code === 'string' ? value.code : 'operation_failed', message, recoverable: value.recoverable === true };
  }
  if (typeof error === 'string') return envelope(error) ?? { code: 'operation_failed', message: error };
  return { code: 'operation_failed', message: 'The operation could not be completed.' };
}

const aliases: Record<string, string> = {
  speaker_stale: 'speaker_stale', speaker_checkpoint_version: 'speaker_checkpoint_version', speaker_checkpoint_invalid: 'speaker_checkpoint_invalid',
  speaker_retry_confirmation: 'speaker_retry_confirmation', speaker_cleanup: 'speaker_cleanup', speaker_model: 'speaker_model',
  stale_notes: 'stale_notes', notes_stale: 'stale_notes', quota: 'quota', quota_exceeded: 'quota', audio_quota: 'quota',
  auth: 'auth', unauthorized: 'auth', unauthenticated: 'auth', signed_out: 'auth', not_signed_in: 'auth', token_expired: 'auth', invalid_token: 'auth', http_401: 'auth',
  forbidden: 'access', http_403: 'access', vault_locked: 'vault_locked', needs_recovery_code: 'vault_locked', vault_reset: 'vault_locked', storage: 'storage', storage_full: 'storage', speaker_storage: 'storage',
  network: 'network', offline: 'network', timeout: 'network', http_408: 'network', http_504: 'network', cancelled: 'cancelled', canceled: 'cancelled',
  interrupted: 'interrupted', recognition: 'recognition', consent_required: 'consent_required', microphone_denied: 'microphone_denied',
  unsupported: 'unsupported', unsupported_operation: 'unsupported', meeting_busy: 'meeting_busy', credential_store: 'credential_store',
  not_found: 'not_found', invalid_request: 'request', request: 'request', http_400: 'request', invalid_conflict: 'not_found',
  too_large: 'audio', invalid_audio: 'audio', audio_too_long: 'audio', http_413: 'audio',
  overloaded: 'overloaded', rate_limited: 'overloaded', http_429: 'overloaded', upstream: 'overloaded', speakers_unavailable: 'overloaded', speaker_unavailable: 'overloaded', http_500: 'overloaded', http_502: 'overloaded', http_503: 'overloaded',
  session_unavailable: 'session_unavailable', last_provider: 'last_provider', audio_unavailable: 'audio_unavailable', operation_busy: 'operation_busy',
  provider_changed: 'model_changed', provider_not_selected: 'model_changed', meeting_changed: 'meeting_busy',
};

export function localizedFailure(language: InterfaceLanguage, error: NativeFailure): string {
  return translate(language, `mobile.errors.${aliases[error.code.toLowerCase()] ?? 'operation_failed'}`);
}
