import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage } from "@/lib/utils";
import * as api from "@/lib/tauri";
import type {
  SyncProgressPayload,
  SyncItemsPayload,
  SyncCompletePayload,
  SyncErrorPayload,
  Project,
  SyncStatus,
} from "@/types";

export interface SyncCallbacks {
  onProgress: (payload: SyncProgressPayload) => void;
  onItems: (payload: SyncItemsPayload) => void;
  onComplete: (payload: SyncCompletePayload) => void;
  onError: (payload: SyncErrorPayload) => void;
  /** Return current projects list for periodic/startup sync. */
  getProjects: () => Project[];
  /** Return whether a project is currently syncing. */
  isProjectSyncing: (projectId: string) => boolean;
  /** Mark a project as syncing with a status message. */
  setSyncingProject: (projectId: string, phase: string | null) => void;
  /** Clear a project's syncing state. */
  clearSyncingProject: (projectId: string) => void;
  /** Update the global sync status indicator. */
  setSyncStatus: (status: Partial<SyncStatus>) => void;
}

/**
 * Manages Tauri sync event listeners and manual sync triggers.
 * Periodic sync and startup sync are handled by the Rust backend.
 *
 * This is a plain TypeScript class with no React dependency.
 */
export class SyncManager {
  private listeners: UnlistenFn[] = [];

  constructor(private callbacks: SyncCallbacks) {}

  /**
   * Set up all Tauri event listeners. Call once on mount.
   */
  async start(): Promise<void> {
    this.listeners.push(
      await listen<SyncProgressPayload>("sync:progress", (event) => {
        this.callbacks.onProgress(event.payload);
      }),
      await listen<SyncItemsPayload>("sync:items", (event) => {
        this.callbacks.onItems(event.payload);
      }),
      await listen<SyncCompletePayload>("sync:complete", (event) => {
        this.callbacks.onComplete(event.payload);
      }),
      await listen<SyncErrorPayload>("sync:error", (event) => {
        this.callbacks.onError(event.payload);
      }),
    );
  }

  /**
   * Sync a single project. Guards against duplicate syncs and disabled projects.
   */
  syncProject(projectId: string): void {
    if (this.callbacks.isProjectSyncing(projectId)) return;
    const projects = this.callbacks.getProjects();
    const project = projects.find((p) => p.id === projectId);
    if (!project?.sync_enabled) return;
    console.debug("[sync] starting sync for project", projectId);
    this.callbacks.setSyncingProject(projectId, "Starting sync...");
    api.syncProjectItems(projectId).catch((err) => {
      console.error("[sync] failed to start sync:", err);
      this.callbacks.setSyncStatus({ state: "error", lastError: errorMessage(err) });
      this.callbacks.clearSyncingProject(projectId);
    });
  }

  /**
   * Sync multiple projects concurrently.
   */
  syncMultipleProjects(projectIds: string[]): void {
    for (const id of projectIds) {
      this.syncProject(id);
    }
  }

  /**
   * Perform a full (re-)sync of a single project.
   */
  fullSyncProject(projectId: string): void {
    const projects = this.callbacks.getProjects();
    const project = projects.find((p) => p.id === projectId);
    if (!project?.sync_enabled) return;
    if (this.callbacks.isProjectSyncing(projectId)) return;
    console.debug("[sync] starting full sync for project", projectId);
    this.callbacks.setSyncingProject(projectId, "Starting full sync...");
    api.fullSyncProjectItems(projectId).catch((err) => {
      console.error("[sync] failed to start full sync:", err);
      this.callbacks.setSyncStatus({ state: "error", lastError: errorMessage(err) });
      this.callbacks.clearSyncingProject(projectId);
    });
  }

  /**
   * Tear down all listeners. Call on unmount.
   */
  destroy(): void {
    for (const unlisten of this.listeners) {
      unlisten();
    }
    this.listeners = [];
  }
}
