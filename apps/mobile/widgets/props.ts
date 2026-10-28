import tokens from '../../../assets/brand/tokens.json';
import dictationEn from '../../../assets/locales/dictation.en.json';
import dictationNl from '../../../assets/locales/dictation.nl.json';
import meetingsEn from '../../../assets/locales/meetings.en.json';
import meetingsNl from '../../../assets/locales/meetings.nl.json';

import type {
  ActivityLabels,
  PennelScheme,
  RecordingActivityProps,
  RecordingActivityState,
  SessionActivityProps,
  SessionActivityState,
  WidgetAppearance,
  WidgetLanguage,
} from './types';

const mobileCopy = {
  en: {
    ready: 'Pennel is ready',
    off: 'Session off',
    stopped: 'Recording stopped',
    unavailable: 'Open Pennel to check the status.',
    open: 'Open Pennel',
  },
  nl: {
    ready: 'Pennel staat klaar',
    off: 'Sessie uit',
    stopped: 'Opname gestopt',
    unavailable: 'Open Pennel om de status te bekijken.',
    open: 'Open Pennel',
  },
} as const;

const appearance: WidgetAppearance = {
  background: tokens.colors['pill-background'].light,
  foreground: tokens.colors['pill-text'].light,
  wave: tokens.colors.wave.light,
  fontFamily: `${tokens.typography.ui}-Medium`,
  fontSize: tokens.typography.sizes.section,
  compactFontSize: tokens.typography.sizes.label,
  labelFontSize: tokens.typography.sizes.body,
  inset: tokens.spacing['4'],
  spacing: tokens.spacing['2'],
  barWidth: tokens.spacing['1'],
  barSpacing: tokens.spacing['1'] / 2,
};

function timestamp(value: number): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > 8_640_000_000_000_000) {
    throw new RangeError('A native activity timestamp is invalid.');
  }
  return value;
}

function waveHeights(level = 0): number[] {
  const amplitude = Number.isFinite(level) ? Math.min(Math.max(level, 0), 1) : 0;
  // Reuse PennelMobileKit Brand.Wave's five-bar silhouette.
  return [0.35, 0.6, 0.85, 0.6, 0.35].map((resting) =>
    tokens.spacing['6'] * (resting + (1 - resting) * amplitude));
}

function labels(status: string, compact: string, language: WidgetLanguage): ActivityLabels {
  return { status, compact, unavailable: mobileCopy[language].unavailable, open: mobileCopy[language].open };
}

export function recordingActivityProps(
  state: RecordingActivityState,
  language: WidgetLanguage,
  scheme: PennelScheme,
): RecordingActivityProps {
  const start = timestamp(state.started_at_ms);
  const observed = timestamp(state.observed_at_ms);
  if (observed < start) throw new RangeError('A recording observation precedes its start.');
  const copy = language === 'nl' ? meetingsNl : meetingsEn;
  const status = state.phase === 'recording' ? copy['meetings.recording_active']
    : state.phase === 'finishing' ? copy['meetings.finishing_recording']
      : state.phase === 'interrupted' ? copy['meetings.recording_needs_attention']
        : mobileCopy[language].stopped;
  return {
    kind: 'recording',
    appearance,
    labels: labels(status, status, language),
    clock: { mode: state.phase === 'recording' ? 'elapsed' : 'none', start_at_ms: start, end_at_ms: observed },
    wave_heights: waveHeights(state.phase === 'recording' ? state.level : 0),
    open_url: `${scheme}://`,
  };
}

export function sessionActivityProps(
  state: SessionActivityState,
  language: WidgetLanguage,
  scheme: PennelScheme,
): SessionActivityProps {
  const observed = timestamp(state.observed_at_ms);
  const expires = timestamp(state.expires_at_ms);
  const copy = language === 'nl' ? dictationNl : dictationEn;
  const phase = state.phase === 'ready' && expires <= observed ? 'off' : state.phase;
  const status = phase === 'ready' ? mobileCopy[language].ready
    : phase === 'listening' ? copy['dictation.pill.listening']
      : phase === 'processing' ? copy['dictation.pad.processing'] : mobileCopy[language].off;
  return {
    kind: 'session',
    appearance,
    labels: labels(status, status, language),
    clock: { mode: phase === 'ready' ? 'remaining' : 'none', start_at_ms: Math.min(observed, expires), end_at_ms: expires },
    wave_heights: waveHeights(phase === 'listening' ? state.level : 0),
    open_url: `${scheme}://`,
  };
}

export const recordingControlAuthority = 'native-activitykit' as const;
