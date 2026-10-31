import React from 'react';
import type { Settings } from '../../domain';
import { useRuntime } from '../../runtime';
import { Section, Toggle } from '../../components/ui';

const copy = {
  en: {
    title: 'Feedback', app: 'Haptic feedback', appHelp: 'Feel important recording actions, results, and connection changes.',
    typing: 'Keyboard typing feedback', typingHelp: 'Light taps while typing with the Pennel keyboard. System settings and device support still apply.',
  },
  nl: {
    title: 'Feedback', app: 'Haptische feedback', appHelp: 'Voel belangrijke opnameacties, resultaten en verbindingswijzigingen.',
    typing: 'Trillingen tijdens het typen', typingHelp: 'Lichte tikjes tijdens het typen met het Pennel-toetsenbord. Systeeminstellingen en toestelondersteuning blijven gelden.',
  },
};

export function FeedbackPanel() {
  const { snapshot, store, run, busy, language } = useRuntime();
  const labels = copy[language];
  const save = (patch: { haptic_feedback?: boolean; keyboard_haptics?: boolean }) => {
    const settings = store.getSnapshot().snapshot?.settings;
    if (settings) void run<Settings>('feedback-settings', 'settings.update', { settings: { ...settings, ...patch } });
  };
  return <Section title={labels.title}>
    <Toggle title={labels.app} description={labels.appHelp} value={snapshot?.settings.haptic_feedback !== false} disabled={busy.has('feedback-settings')} onChange={(haptic_feedback) => save({ haptic_feedback })} />
    <Toggle title={labels.typing} description={labels.typingHelp} value={snapshot?.settings.keyboard_haptics === true} disabled={busy.has('feedback-settings')} onChange={(keyboard_haptics) => save({ keyboard_haptics })} />
  </Section>;
}
