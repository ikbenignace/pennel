import type { MeetingAnswer, MeetingDetail, Passage, SpeakerJob } from './domain.ts';

export function meetingDetail(value: MeetingDetail): MeetingDetail {
  const raw = value as MeetingDetail & { markdown?: string | null; audio_available?: boolean };
  const interruption = (value as unknown as { interruption?: unknown }).interruption;
  return { ...raw, notes: raw.notes ?? '', summary: raw.summary ?? raw.markdown ?? null, transcript: raw.transcript ?? [], speakers: raw.speakers ?? [], attendees: raw.attendees ?? [], audio_tracks: raw.audio_tracks ?? [{ speaker: 'others', available: raw.audio_available === true }], conflicts: raw.conflicts ?? [], interruption: typeof interruption === 'string' ? interruption : interruption && typeof interruption === 'object' && 'stopped_early' in interruption && typeof interruption.stopped_early === 'string' ? interruption.stopped_early : null };
}

export function meetingAnswer(value: MeetingAnswer): MeetingAnswer {
  const raw = value as MeetingAnswer & { sources?: Passage[] };
  return { text: raw.text, citations: raw.citations ?? (raw.sources ?? []).map((passage, index) => ({ number: index + 1, passage })) };
}

export function speakerJob(value: SpeakerJob): SpeakerJob {
  const raw = value as SpeakerJob & { job_id?: string; upload_bytes?: number };
  return { ...raw, id: raw.id ?? raw.job_id ?? '', uploaded_bytes: raw.uploaded_bytes ?? raw.upload_bytes ?? 0, stage: raw.stage === ('response-unknown' as string) ? 'response_unknown' : raw.stage };
}
