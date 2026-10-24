import { invoke } from "@tauri-apps/api/core";

export type ProviderKind =
  | "open_ai"
  | "open_router"
  | "groq"
  | "mistral"
  | "ollama"
  | "lm_studio"
  | "custom"
  | "local";

export type HotkeyTrigger = "fn" | "right_option" | "right_command" | "right_control" | "control_super";
export type Style = "casual" | "formal" | "literal" | "neutral";

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
  speaker: "me" | "others";
  text: string;
  failed: boolean;
  voice: number | null;
}

export interface MeetingStatus {
  recording: boolean;
  id: string | null;
  title: string | null;
  elapsed_secs: number;
  system_audio: boolean;
  heard_system_audio: boolean;
  pending: number;
  transcript: Segment[];
}

export interface MeetingDetail {
  info: MeetingInfo | null;
  notes: string;
  summary: string | null;
  transcript: Segment[];
  speakers: { voice: number; name: string }[];
  dir: string;
}

export interface MeetingTemplate {
  id: string;
  name: string;
  builtin: boolean;
}

export const api = {
  info: () => invoke<AppInfo>("app_info"),
  saveSettings: (settings: Settings) => invoke<AppInfo>("save_settings", { settings }),
  addProvider: (kind: ProviderKind) => invoke<AppInfo>("add_provider", { kind }),
  removeProvider: (id: string) => invoke<AppInfo>("remove_provider", { id }),
  setKey: (account: string, key: string) => invoke<AppInfo>("set_key", { account, key }),
  providerModels: (id: string) => invoke<RemoteModel[]>("provider_models", { id }),
  localModels: (language: string | null, supportedOnly: boolean) =>
    invoke<LocalModel[]>("local_models", { language, supportedOnly }),
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
  setMeetingNotes: (text: string) => invoke<void>("set_meeting_notes", { text }),
  meetings: (query: string) => invoke<MeetingInfo[]>("meetings", { query }),
  meetingDetail: (id: string) => invoke<MeetingDetail>("meeting_detail", { id }),
  meetingTemplates: () => invoke<MeetingTemplate[]>("meeting_templates"),
  summarizeMeeting: (id: string, templateId: string) =>
    invoke<string>("summarize_meeting", { id, templateId }),
  retranscribeMeeting: (id: string) => invoke<number>("retranscribe_meeting", { id }),
  renameMeeting: (id: string, title: string) => invoke<string>("rename_meeting", { id, title }),
  deleteMeeting: (id: string) => invoke<void>("delete_meeting", { id }),
};
