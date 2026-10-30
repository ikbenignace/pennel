import { useCallback } from 'react';
import PennelMobile from '../modules/pennel-mobile';
import { useRuntime } from './runtime';

export type HapticEvent = 'selection' | 'recordingStarted' | 'recordingStopped' | 'recordingCancelled'
  | 'resultDelivered' | 'meetingSaved' | 'deviceConnected' | 'warning' | 'error';

export async function feedback(event: HapticEvent): Promise<void> {
  try { await PennelMobile.invoke('ui.feedback', { event }); }
  catch { /* Optional tactile feedback must never change the operation's outcome. */ }
}

export function useHapticFeedback(): (event: HapticEvent) => void {
  const { snapshot } = useRuntime();
  const enabled = snapshot?.settings.haptic_feedback !== false;
  return useCallback((event: HapticEvent) => { if (enabled) void feedback(event); }, [enabled]);
}
