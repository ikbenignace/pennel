import type { MeetingDetail, Passage } from './domain.ts';

let sequence = 0;
const sources = new Map<string, { passage: Passage; created: number }>();
export function rememberSource(passage: Passage, now = Date.now()): string {
  const token = `source-${++sequence}`;
  sources.set(token, { passage, created: now });
  while (sources.size > 32) sources.delete(sources.keys().next().value as string);
  return token;
}

function normalized(text: string): string { return text.replace(/\s+/g, ' ').trim(); }
export function resolveSource(token: string, detail: MeetingDetail, now = Date.now()): { changed: boolean; passage: Passage | null } {
  const source = sources.get(token);
  if (!source || now - source.created > 30 * 60 * 1000 || source.passage.meeting_id !== detail.info.id) return { changed: true, passage: null };
  const text = source.passage.kind === 'notes' ? detail.notes : source.passage.kind === 'summary' ? detail.summary ?? '' : detail.transcript.map((segment) => segment.text).join(' ');
  const cited = normalized(source.passage.text);
  return { changed: !cited || !normalized(text).includes(cited), passage: source.passage };
}
