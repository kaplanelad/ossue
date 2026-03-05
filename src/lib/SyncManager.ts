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
  /** Refresh the projects list (e.g. after sync completes). */
  refreshProjects: () => void;
  /** Return current count of syncing projects (after clearing one). */
  getSyncingProjectCount: () => number;
}

/**
 * Manages all sync infrastructure: Tauri event listeners, startup sync,
 * periodic sync timer, and the startup:sync backend event.
 *
 * This is a plain TypeScript class with no React dependency.
 */
export class SyncManager {
  private listeners: UnlistenFn[] = [];
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private static startupSyncDone = false;

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
      await listen<string>("startup:sync", (event) => {
        this.syncProject(event.payload);
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
   * Run startup sync once per app session for all sync-enabled projects.
   * Returns true if sync was triggered, false if already done or no projects.
   */
  startupSync(): boolean {
    if (SyncManager.startupSyncDone) return false;
    const projects = this.callbacks.getProjects();
    if (projects.length === 0) return false;
    SyncManager.startupSyncDone = true;
    for (const project of projects) {
      if (project.sync_enabled) {
        this.syncProject(project.id);
      }
    }
    return true;
  }

  /**
   * Start periodic sync on an interval (in seconds).
   */
  startPeriodicSync(intervalSecs: number): void {
    this.stopPeriodicSync();
    if (intervalSecs <= 0) return;
    this.intervalId = setInterval(() => {
      const projects = this.callbacks.getProjects();
      for (const project of projects) {
        if (project.sync_enabled) {
          this.syncProject(project.id);
        }
      }
    }, intervalSecs * 1000);
  }

  /**
   * Stop the periodic sync timer.
   */
  stopPeriodicSync(): void {
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }

  /**
   * Tear down all listeners and timers. Call on unmount.
   */
  destroy(): void {
    this.stopPeriodicSync();
    for (const unlisten of this.listeners) {
      unlisten();
    }
    this.listeners = [];
  }

  /** Reset the startup sync flag (useful for testing). */
  static resetStartupSync(): void {
    SyncManager.startupSyncDone = false;
  }
}
