import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { LoginPoll } from "./cloud-login";

export type ProviderKind =
  | "open_ai"
  | "open_router"
  | "groq"
  | "mistral"
  | "ollama"
  | "lm_studio"
  | "custom"
  | "local"
  | "apple_speech"
  | "apple_intelligence"
  | "claude_code"
  | "codex"
  | "open_code"
  | "copilot"
  | "cursor_agent"
  | "gemini_cli"
  | "pennel_cloud";

export type HotkeyTrigger = "fn" | "right_option" | "right_command" | "right_control" | "control_super";
export type Style = "casual" | "formal" | "literal" | "neutral";
export type RecordAction = "ask" | "always" | "never";
export type CapabilityStatus = "available" | "permission_required" | "unsupported" | "unavailable";
export type PermissionKind = "microphone" | "accessibility" | "calendar" | "speech" | "system_audio";
export type PermissionStatus = "granted" | "not_determined" | "denied" | "restricted" | "not_required" | "unsupported" | "unknown";
export type AppWindow = "library" | "settings" | "history" | "live" | "onboarding";
export type MeetingPhase = "idle" | "starting" | "recording" | "stopping" | "finalizing" | "error";
export type Speaker = "me" | "others";

export interface Capability {
  status: CapabilityStatus;
  reason: string | null;
}

export interface PermissionInfo {
  kind: PermissionKind;
  status: PermissionStatus;
}

export interface PlatformCapabilities {
  global_hotkey: Capability;
  record_shortcut: Capability;
  microphone: Capability;
  system_audio: Capability;
  echo_cancellation: Capability;
  call_detection: Capability;
  calendar: Capability;
  apple_speech: Capability;
  apple_intelligence: Capability;
  selection: Capability;
  rich_clipboard: Capability;
  audio_playback: Capability;
  diarization: Capability;
  local_acceleration: "metal" | "vulkan" | "cpu";
}

export interface ProviderAccount {
  id: string;
  kind: ProviderKind;
  name: string;
  base_url: string;
}

export interface ModelChoice {
  provider_id: string;
  model: string;
}

export interface Settings {
  providers: ProviderAccount[];
  dictation_stt: ModelChoice;
  meeting_stt: ModelChoice;
  llm: ModelChoice;
  mode: "fast" | "clean";
  language: string | null;
  dictionary: string[];
  style_rules: { bundle_id: string; style: Style }[];
  history_retention_days: number;
  hotkey: HotkeyTrigger;
  local_unload_minutes: number;
  meetings_dir: string;
  default_template: string;
  meeting_audio_retention_days: number;
  app_rules: { bundle_id: string; action: RecordAction }[];
  calendar_rules: { calendar_id: string; calendar_name: string; action: RecordAction }[];
  event_rules: { title: string; action: RecordAction }[];
  calendar_enabled: boolean;
  echo_cancellation: boolean;
  record_shortcut_enabled: boolean;
  consent_enabled: boolean;
  auto_detect: boolean;
  diarization: boolean;
  [key: string]: unknown;
}

export interface KindInfo {
  kind: ProviderKind;
  label: string;
  requires_key: boolean;
  default_base_url: string;
  stt: boolean;
  llm: boolean;
  default_stt_model: string;
  default_llm_model: string;
  is_cli_agent: boolean;
  availability: Capability;
}

export interface AppInfo {
  settings: Settings;
  keys: Record<string, boolean>;
  kinds: KindInfo[];
  platform: string;
  version: string;
  hotkey_error: string | null;
  log_path: string | null;
  data_dir: string;
  capabilities: PlatformCapabilities;
  diarizer_model_id: string;
  preview: boolean;
  migration_warning: string | null;
}

export interface RemoteModel {
  id: string;
  name: string | null;
  audio_input: boolean;
  is_router: boolean;
}

export type ModelState = { kind: "not_downloaded" } | { kind: "partial"; bytes: number } | { kind: "downloaded" };

export interface LocalModel {
  id: string;
  name: string;
  description: string;
  size_bytes: number;
  parameters: string | null;
  speed: number;
  accuracy: number | null;
  languages: string[];
  recommended: boolean;
  legacy: boolean;
  state: ModelState;
}

export interface HistoryEntry {
  id: string;
  created_at_ms: number;
  app_bundle_id: string | null;
  raw: string;
  text: string;
  status: "done" | "failed";
  error: string | null;
  has_audio: boolean;
}

export interface MeetingInfo {
  id: string;
  title: string;
  start_iso: string;
  duration_min: number;
  has_summary: boolean;
}

export interface Segment {
  start_ms: number;
  end_ms: number;
  speaker: Speaker;
  text: string;
  failed: boolean;
  voice: number | null;
}

export interface MeetingStatus {
  recording: boolean;
  phase: MeetingPhase;
  busy: boolean;
  error: string | null;
  notes: string;
  session_id: string | null;
  id: string | null;
  title: string | null;
  elapsed_secs: number;
  system_audio: boolean;
  heard_system_audio: boolean;
  audio_warning: string | null;
  consent_copied?: boolean;
  pending: number;
  transcript: Segment[];
}

export interface MeetingFinished {
  id: string | null;
  session_id: string;
  error: string | null;
  flush_warning: string | null;
}

export interface MeetingDetail {
  info: MeetingInfo | null;
  notes: string;
  summary: string | null;
  transcript: Segment[];
  speakers: { voice: number; name: string }[];
  dir: string;
  attendees: string[];
  audio_tracks: { speaker: Speaker; available: boolean }[];
}

export interface MeetingTemplate {
  id: string;
  name: string;
  builtin: boolean;
  customized: boolean;
}

export interface Passage {
  meeting_id: string;
  title: string;
  start_iso: string;
  kind: "notes" | "transcript" | "summary";
  start_ms: number | null;
  text: string;
  snippet: string;
}

export interface LibrarySearch {
  meetings: MeetingInfo[];
  passages: Passage[];
}

export interface ChatTurn {
  question: string;
  answer: string;
}

export interface MeetingAnswer {
  text: string;
  citations: { number: number; passage: Passage }[];
}

export interface MeetingExport {
  title: string;
  markdown: string;
  html: string;
}

export interface DetectedAgent {
  kind: ProviderKind;
  path: string;
}

export interface CalendarSnapshot {
  has_access: boolean;
  calendars: { id: string; name: string }[];
  current_event: {
    title: string;
    calendar_id: string;
    calendar_name: string;
    attendees: string[];
  } | null;
}

export interface RecordPrompt {
  id: number;
  app_id: string;
  app_name: string;
  title: string | null;
  expires_at_ms: number;
}

export type PillState =
  | { state: "hidden" | "listening" | "hands_free" | "command" | "working" }
  | { state: "error"; message: string };

export interface UpdateInfo {
  available: boolean;
  version: string | null;
  body: string | null;
}

export type SyncStatus =
  | { kind: "off" | "signed_out" | "needs_setup" | "needs_recovery_code" | "vault_reset" | "pending" | "synced" | "offline" }
  | { kind: "storage_full" | "error"; message: string };

export interface SyncReport {
  status: SyncStatus;
  brake: boolean;
  uploaded: number;
  downloaded: number;
  conflicts: number;
  last_success_ms: number | null;
}

export interface CloudLoginStart {
  user_code: string;
  verification_uri: string;
  device_code: string;
  interval_secs: number;
}

export interface CloudAccount {
  email: string;
  audio_seconds: number;
  tokens: number;
  storage_bytes: number;
  limit_audio_seconds: number;
  limit_tokens: number;
  limit_storage_bytes: number;
}

export interface CloudDevice {
  id: string;
  name: string;
  platform: string;
  last_seen_iso: string;
  current: boolean;
}

export interface MeetingSyncState {
  id: string;
  state: "synced" | "pending" | "conflict";
}

export interface CloudError {
  code: string;
  message: string;
}

export const api = {
  info: () => invoke<AppInfo>("app_info"),
  saveSettings: (settings: Settings) => invoke<AppInfo>("save_settings", { settings }),
  addProvider: (kind: ProviderKind) => invoke<AppInfo>("add_provider", { kind }),
  removeProvider: (id: string) => invoke<AppInfo>("remove_provider", { id }),
  setKey: (account: string, key: string) => invoke<AppInfo>("set_key", { account, key }),
  permissions: () => invoke<PermissionInfo[]>("get_permissions"),
  requestPermission: (kind: PermissionKind) => invoke<PermissionInfo[]>("request_permission", { kind }),
  testAppleIntelligence: () => invoke<void>("test_apple_intelligence"),
  calendar: () => invoke<CalendarSnapshot>("calendar_snapshot"),
  chooseMeetingsDir: () => invoke<string | null>("choose_meetings_dir"),
  showWindow: (kind: AppWindow, tab: string | null = null) => invoke<void>("show_window", { kind, tab }),
  copySecretText: (text: string) => invoke<void>("copy_secret_text", { text }),
  copyText: (text: string, html: string | null = null) => invoke<void>("copy_text", { text, html }),
  playMeetingAudio: (id: string, speaker: Speaker, startMs: number) =>
    invoke<void>("play_meeting_audio", { id, speaker, startMs }),
  stopAudio: () => invoke<void>("stop_audio"),
  recordPrompt: () => invoke<RecordPrompt | null>("record_prompt"),
  answerRecordPrompt: (answer: "yes" | "no" | "never", promptId: number) =>
    invoke<void>("answer_record_prompt", { answer, promptId }),
  toggleDictation: (copyOnly = true) => invoke<void>("toggle_dictation", { copyOnly }),
  dictationState: () => invoke<PillState>("dictation_state"),
  checkForUpdates: () => invoke<UpdateInfo>("check_for_updates"),
  installUpdate: () => invoke<void>("install_update"),
  frontendReady: () => invoke<void>("frontend_ready"),
  frontendFlushed: (requestId: number, success: boolean, error: string | null = null) =>
    invoke<void>("frontend_flushed", { requestId, success, error }),
  shareMeeting: (id: string, includeTranscript: boolean, action: "mail" | "share") =>
    invoke<void>("share_meeting", { id, includeTranscript, action }),
  markLines: (enhanced: string, notes: string) => invoke<{ text: string; mine: boolean }[]>("mark_lines", { enhanced, notes }),
  detectCliAgents: () => invoke<DetectedAgent[]>("detect_cli_agents"),
  checkCliAgent: (id: string, model: string) => invoke<string>("check_cli_agent", { id, model }),
  providerModels: (id: string) => invoke<RemoteModel[]>("provider_models", { id }),
  localModels: (language: string | null, supportedOnly: boolean) =>
    invoke<LocalModel[]>("local_models", { language, supportedOnly }),
  modelState: (id: string) => invoke<ModelState>("model_state", { id }),
  downloadModel: (id: string) => invoke<void>("download_model", { id }),
  cancelDownload: (id: string) => invoke<void>("cancel_download", { id }),
  deleteModel: (id: string) => invoke<AppInfo>("delete_model", { id }),
  history: (limit: number) => invoke<HistoryEntry[]>("history", { limit }),
  retryHistory: (id: string) => invoke<string>("retry_history", { id }),
  deleteHistory: (id: string) => invoke<void>("delete_history", { id }),
  openLog: () => invoke<void>("open_log"),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  meetingStatus: () => invoke<MeetingStatus>("meeting_status"),
  startMeeting: (title: string) => invoke<string>("start_meeting", { title }),
  stopMeeting: () => invoke<void>("stop_meeting"),
  setMeetingNotes: (text: string, sessionId: string) => invoke<void>("set_meeting_notes", { text, sessionId }),
  setMeetingTitle: (title: string, sessionId: string) => invoke<void>("set_meeting_title", { title, sessionId }),
  meetings: (query: string) => invoke<MeetingInfo[]>("meetings", { query }),
  searchLibrary: (query: string) => invoke<LibrarySearch>("search_library", { query }),
  askMeetings: (question: string, meetingId: string | null, history: ChatTurn[]) =>
    invoke<MeetingAnswer>("ask_meetings", { question, meetingId, history }),
  meetingDetail: (id: string) => invoke<MeetingDetail>("meeting_detail", { id }),
  meetingTemplates: () => invoke<MeetingTemplate[]>("meeting_templates"),
  templateSource: (id: string) => invoke<string | null>("template_source", { id }),
  saveTemplate: (id: string | null, name: string, body: string) =>
    invoke<string>("save_template", { id, name, body }),
  deleteTemplate: (id: string) => invoke<void>("delete_template", { id }),
  summarizeMeeting: (id: string, templateId: string) =>
    invoke<string>("summarize_meeting", { id, templateId }),
  retranscribeMeeting: (id: string) => invoke<number>("retranscribe_meeting", { id }),
  saveMeetingNotes: (id: string, text: string) => invoke<void>("save_meeting_notes", { id, text }),
  setMeetingAttendees: (id: string, attendees: string[]) => invoke<void>("set_meeting_attendees", { id, attendees }),
  renameSpeaker: (id: string, voice: number, name: string) => invoke<void>("rename_speaker", { id, voice, name }),
  diarizeMeeting: (id: string) => invoke<number>("diarize_meeting", { id }),
  exportMeeting: (id: string, includeTranscript: boolean) =>
    invoke<MeetingExport>("export_meeting", { id, includeTranscript }),
  renameMeeting: (id: string, title: string) => invoke<string>("rename_meeting", { id, title }),
  deleteMeeting: (id: string) => invoke<void>("delete_meeting", { id }),
  cloudLoginStart: () => invoke<CloudLoginStart>("cloud_login_start"),
  cloudLoginPoll: (deviceCode: string) => invoke<LoginPoll>("cloud_login_poll", { deviceCode }),
  cloudLogout: () => invoke<void>("cloud_logout"),
  cloudAccount: () => invoke<CloudAccount>("cloud_account"),
  cloudDevices: () => invoke<CloudDevice[]>("cloud_devices"),
  cloudRenameDevice: (id: string, name: string) => invoke<void>("cloud_rename_device", { id, name }),
  cloudRevokeDevice: (id: string) => invoke<void>("cloud_revoke_device", { id }),
  cloudDeleteAccount: () => invoke<void>("cloud_delete_account"),
  cloudSyncEnable: () => invoke<string>("cloud_sync_enable"),
  cloudSyncUnlock: (code: string) => invoke<void>("cloud_sync_unlock", { code }),
  cloudSyncNewRecoveryCode: () => invoke<string>("cloud_sync_new_recovery_code"),
  cloudSyncReset: () => invoke<string>("cloud_sync_reset"),
  cloudSyncDisable: () => invoke<void>("cloud_sync_disable"),
  cloudSyncNow: () => invoke<SyncReport>("cloud_sync_now"),
  cloudSyncLast: () => invoke<SyncReport>("cloud_sync_last"),
  cloudSyncConfirmDeletes: () => invoke<void>("cloud_sync_confirm_deletes"),
  meetingSyncStates: () => invoke<MeetingSyncState[]>("meeting_sync_states"),
  meetingConflictFiles: (id: string) => invoke<string[]>("meeting_conflict_files", { id }),
  deleteConflictCopy: (id: string, name: string) => invoke<void>("delete_conflict_copy", { id, name }),
  cloudFolderWarning: () => invoke<string | null>("cloud_folder_warning"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  onSyncChanged: (handler: (report: SyncReport) => void): Promise<UnlistenFn> =>
    listen<SyncReport>("sync-changed", (event) => handler(event.payload)),
};
