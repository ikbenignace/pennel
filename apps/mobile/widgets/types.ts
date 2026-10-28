export type WidgetLanguage = 'en' | 'nl';
export type PennelScheme = 'pennel' | 'pennel-preview';

export interface WidgetAppearance {
  background: string;
  foreground: string;
  wave: string;
  fontFamily: string;
  fontSize: number;
  compactFontSize: number;
  labelFontSize: number;
  inset: number;
  spacing: number;
  barWidth: number;
  barSpacing: number;
}

export interface ActivityLabels {
  status: string;
  compact: string;
  unavailable: string;
  open: string;
}

export interface ActivityClock {
  mode: 'elapsed' | 'remaining' | 'none';
  start_at_ms: number;
  end_at_ms: number;
}

export interface PennelActivityProps {
  kind: 'recording' | 'session';
  appearance: WidgetAppearance;
  labels: ActivityLabels;
  clock: ActivityClock;
  wave_heights: number[];
  open_url: string;
}

export interface RecordingActivityState {
  phase: 'recording' | 'interrupted' | 'finishing' | 'stopped';
  started_at_ms: number;
  observed_at_ms: number;
  level?: number;
}

export interface SessionActivityState {
  phase: 'off' | 'ready' | 'listening' | 'processing';
  expires_at_ms: number;
  observed_at_ms: number;
  level?: number;
}

export type RecordingActivityProps = PennelActivityProps & { kind: 'recording' };
export type SessionActivityProps = PennelActivityProps & { kind: 'session' };
