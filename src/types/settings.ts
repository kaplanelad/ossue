import type { AiMode, AiProvider, ReviewStrictness, ResponseTone } from "./ai";

export interface AppSettings {
  ai_mode: AiMode;
  ai_provider: AiProvider;
  has_ai_api_key: boolean;
  ai_model: string;
  ai_cli_path: string | null;
  ai_focus_areas: string[];
  ai_review_strictness: ReviewStrictness;
  ai_response_tone: ResponseTone;
  ai_custom_instructions: string | null;
  attention_sensitive_paths: string[];
  refresh_interval: number;
  github_connected: boolean;
  gitlab_connected: boolean;
  log_level: string;
}

export interface AuthStatus {
  github_connected: boolean;
  gitlab_connected: boolean;
}

export interface ProjectSettingEntry {
  key: string;
  value: string;
}

export interface BackupInfo {
  filename: string;
  created_at: string;
  size_bytes: number;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
  fields: Record<string, string>;
}

export interface LogEntriesResponse {
  entries: LogEntry[];
  total: number;
  has_more: boolean;
}

export interface AppPaths {
  repo_cache_dir: string;
  repo_cache_size_bytes: number;
  database_file: string;
  log_dir: string;
}
