import type { RuntimeSnapshot } from './domain.ts';

type MeetingSnapshot = RuntimeSnapshot['meeting'];
export class RecordingIdentity {
  private meeting: MeetingSnapshot | null = null;
  private originalId: string | null = null;
  get key(): string | null { return this.originalId; }
  observe(current: MeetingSnapshot | undefined): MeetingSnapshot | null {
    if (!this.meeting && current?.id) { this.meeting = current; this.originalId = current.id; }
    else if (current?.id && current.id === this.meeting?.id) this.meeting = current;
    if (!this.meeting) return null;
    if (current?.id !== this.meeting.id) return { ...this.meeting, phase: 'idle', pending: 0, error: current?.last_finished_id === this.meeting.id || current?.retained_id === this.meeting.id ? current.error : this.meeting.error };
    return this.meeting;
  }
  retarget(originalId: string, finishedId: string): boolean {
    if (!finishedId || this.meeting?.id !== originalId) return false;
    this.meeting = { ...this.meeting, id: finishedId, phase: 'idle' };
    return true;
  }
}

export async function stopThenSave(stop: () => Promise<boolean>, save: () => Promise<boolean>): Promise<{ stopped: boolean; saved: boolean }> {
  let stopped = false;
  let saved = false;
  try { stopped = await stop(); } catch { stopped = false; }
  try { saved = await save(); } catch { saved = false; }
  return { stopped, saved };
}
