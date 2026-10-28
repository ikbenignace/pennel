import type { InterfaceLanguage, MeetingInfo, SpeakerJob } from './domain.ts';

export function clock(seconds: number): string {
  const rounded = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(rounded / 60);
  return `${minutes}:${String(rounded % 60).padStart(2, '0')}`;
}

export function meetingGroups(meetings: MeetingInfo[], now = new Date()): { key: 'today' | 'week' | 'earlier'; meetings: MeetingInfo[] }[] {
  const today = new Date(now); today.setHours(0, 0, 0, 0);
  const week = new Date(today); week.setDate(week.getDate() - 7);
  const groups: Record<'today' | 'week' | 'earlier', MeetingInfo[]> = { today: [], week: [], earlier: [] };
  for (const meeting of [...meetings].sort((a, b) => b.start_iso.localeCompare(a.start_iso))) {
    const date = new Date(meeting.start_iso).getTime();
    groups[date >= today.getTime() ? 'today' : date >= week.getTime() ? 'week' : 'earlier'].push(meeting);
  }
  return (['today', 'week', 'earlier'] as const).filter((key) => groups[key].length).map((key) => ({ key, meetings: groups[key] }));
}

export function dateLabel(iso: string, language: InterfaceLanguage): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : new Intl.DateTimeFormat(language === 'nl' ? 'nl-BE' : 'en-GB', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(date);
}

export function recognitionActions(job: SpeakerJob): { cancel: boolean; resume: boolean; confirmRetry: boolean } {
  return { cancel: !['applying', 'completed', 'cancelled'].includes(job.stage) || job.error?.code === 'speaker_cleanup', resume: job.recoverable && ['interrupted', 'failed', 'response_unknown'].includes(job.stage), confirmRetry: job.stage === 'response_unknown' };
}

export function audioUsage(seconds: number): string { const minutes = Math.floor(seconds / 60); return minutes >= 60 ? `${Math.floor(minutes / 60)} h ${minutes % 60} min` : `${minutes} min`; }
export function bytes(value: number): string { return value >= 1_000_000_000 ? `${(value / 1_000_000_000).toFixed(1)} GB` : `${Math.round(value / 1_000_000)} MB`; }
