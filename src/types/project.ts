import type { Platform } from "./connector";

export interface Project {
  id: string;
  name: string;
  owner: string;
  platform: Platform;
  url: string;
  clone_path: string | null;
  api_token: string | null;
  connector_id: string | null;
  external_project_id: number | null;
  sync_enabled: boolean;
  last_sync_at: string | null;
  last_sync_error: string | null;
  full_reconciliation_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ProjectNote {
  id: string;
  project_id: string;
  note_type: "auto" | "manual";
  content: string;
  created_at: string;
  updated_at: string;
}
