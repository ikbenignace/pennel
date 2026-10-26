import type { MeetingStatus } from "./api";
import { retainedMeetingFailure } from "./recording-processing.ts";

type RecordingStatus = Pick<MeetingStatus, "busy" | "phase" | "id" | "session_id" | "error" | "can_retry_persistence" | "persistence_error" | "terminal_error">;
export type RecordingEntry = { kind: "new" | "active" | "finishing" | "recovery"; meetingId: string | null };

export function recordingEntry(status: RecordingStatus): RecordingEntry {
  if (status.busy) return { kind: status.phase === "stopping" || status.phase === "finalizing" ? "finishing" : "active", meetingId: status.id };
  if (retainedMeetingFailure(status)) return { kind: "recovery", meetingId: status.id };
  return { kind: "new", meetingId: null };
}

export function summaryControl(configured: boolean, recording: boolean, working: boolean): { disabled: boolean; reason: "setup" | "recording" | "working" | null; canSetUp: boolean } {
  if (working) return { disabled: true, reason: "working", canSetUp: false };
  if (!configured) return { disabled: true, reason: "setup", canSetUp: true };
  if (recording) return { disabled: true, reason: "recording", canSetUp: false };
  return { disabled: false, reason: null, canSetUp: false };
}
