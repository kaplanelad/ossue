import type { Item } from './item';

// Sync event payloads (from Tauri backend)
export interface SyncProgressPayload {
  project_id: string;
  phase: "issues" | "prs" | "discussions";
  page: number;
  message: string;
}

export interface SyncItemsPayload {
  project_id: string;
  items: Item[];
}

export interface SyncCompletePayload {
  project_id: string;
  total_items: number;
}

export interface SyncErrorPayload {
  project_id: string;
  error: string;
  retry_in_secs: number | null;
}

export interface SyncStatus {
  state: "idle" | "syncing" | "error" | "done";
  message: string | null;
  lastSyncAt: string | null;
  lastError: string | null;
}
