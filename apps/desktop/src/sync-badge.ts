export type SyncItemState = "synced" | "pending" | "conflict";

const badges: Record<SyncItemState, { icon: string; label: string }> = {
  synced: { icon: "✓", label: "Gesynchroniseerd" },
  pending: { icon: "↻", label: "Wacht op sync" },
  conflict: { icon: "!", label: "Conflictversie" },
};

export function badgeFor(state: SyncItemState | undefined, syncOn: boolean) {
  return syncOn && state ? badges[state] : null;
}

/** The setting can stay on after logout, account deletion or a revoked device; the report tells whether sync can still happen. */
export function syncActive(settingOn: boolean, statusKind: string | undefined): boolean {
  return settingOn && statusKind !== "off" && statusKind !== "signed_out";
}

interface MeetingKey { id: string; title: string; start_iso: string }

// A remote rename moves the meeting folder, so the id the user had open can disappear.
export function resolveOpenMeeting(
  openId: string,
  meetings: MeetingKey[],
  remembered: { title: string; start_iso: string } | null,
): string | undefined {
  if (meetings.some((meeting) => meeting.id === openId)) return openId;
  if (!remembered) return undefined;
  const sameStart = meetings.filter((meeting) => meeting.start_iso === remembered.start_iso);
  if (sameStart.length === 1) return sameStart[0].id;
  return sameStart.find((meeting) => meeting.title === remembered.title)?.id;
}
