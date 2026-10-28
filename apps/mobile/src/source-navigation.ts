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
function transcriptSource(detail: MeetingDetail, passage: Passage, cited: string): number | null {
  const segments = detail.transcript.map((segment, index) => ({ segment, index })).filter(({ segment }) => !segment.failed && segment.text.trim());
  const candidates = segments.flatMap(({ segment }, index) => passage.start_ms == null || segment.start_ms === passage.start_ms ? [index] : []);
  const names = new Map((detail.speakers ?? []).map((speaker) => [speaker.voice, speaker.name]));
  const lines = segments.map(({ segment }) => {
    // Match the core's indexed labels, which are independent of interface language.
    const label = segment.speaker === 'me' ? 'Ik' : segment.voice == null ? 'Anderen' : names.get(segment.voice) ?? `Anderen ${segment.voice}`;
    return `${label}: ${segment.text.trim()}`;
  });
  const found = candidates.find((first) => {
    const offset = normalized(lines.slice(first).join('\n')).indexOf(cited);
    return offset >= 0 && offset < normalized(lines[first]).length;
  });
  return found === undefined ? null : segments[found].index;
}
export function resolveSource(token: string, detail: MeetingDetail, now = Date.now()): { changed: boolean; passage: Passage | null; transcriptIndex: number | null } {
  const source = sources.get(token);
  if (!source || now - source.created > 30 * 60 * 1000 || source.passage.meeting_id !== detail.info.id) return { changed: true, passage: null, transcriptIndex: null };
  const cited = normalized(source.passage.text);
  const transcriptIndex = source.passage.kind === 'transcript' && cited ? transcriptSource(detail, source.passage, cited) : null;
  const valid = cited && (source.passage.kind === 'transcript' ? transcriptIndex !== null : normalized(source.passage.kind === 'notes' ? detail.notes : detail.summary ?? '').includes(cited));
  return { changed: !valid, passage: source.passage, transcriptIndex };
}
