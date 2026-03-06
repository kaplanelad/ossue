import { useCallback, useEffect, useRef } from "react";
import { errorMessage } from "@/lib/utils";
import { toast } from "sonner";
import { useAppStore } from "@/stores/appStore";
import { useItemStore } from "@/stores/itemStore";
import * as api from "@/lib/tauri";
import { SyncManager } from "@/lib/SyncManager";

export function useItems() {
  const {
    items,
    setItems,
    selectedItemId,
    setSelectedItemId,
    selectedProjectIds,
    projects,
    itemTypeFilter,
    syncingProjects,
    onboardingJustCompleted,
    setOnboardingJustCompleted,
    updateItem,
    removeItem,
    setAnalyzedItemIds,
  } = useAppStore();

  const searchQuery = useAppStore((s) => s.searchQuery);
  const isSyncing = Object.keys(syncingProjects).length > 0;

  // --- SyncManager lifecycle ---
  const managerRef = useRef<SyncManager | null>(null);

  useEffect(() => {
    const manager = new SyncManager({
      onProgress(payload) {
        const { project_id, message } = payload;
        console.debug(`[sync] progress: ${message}`);
        useAppStore.getState().setSyncingProject(project_id, message);
        useAppStore.getState().setSyncStatus({ state: "syncing", message, lastError: null });
      },
      onItems(payload) {
        console.debug(`[sync] received ${payload.items.length} items`);
        useAppStore.getState().mergeItems(payload.items);
      },
      onComplete(payload) {
        const { project_id, total_items } = payload;
        console.debug(`[sync] complete: ${total_items} items for ${project_id}`);
        useAppStore.getState().clearSyncingProject(project_id);
        api.listProjects().then(useAppStore.getState().setProjects).catch((err) => console.error("[sync] refresh failed:", err));
        useItemStore.getState().fetchInbox().catch((err) => console.error("[sync] fetchInbox failed:", err));
        const remaining = useAppStore.getState().syncingProjects;
        if (Object.keys(remaining).length === 0) {
          useAppStore.getState().setSyncStatus({ state: "done", message: null, lastSyncAt: new Date().toISOString(), lastError: null });
        }
      },
      onError(payload) {
        const { project_id, error, retry_in_secs } = payload;
        const retryMsg = retry_in_secs
          ? ` Retrying in ${Math.round(retry_in_secs / 60)}min.`
          : "";
        console.error("[sync] error:", error);
        useAppStore.getState().setSyncStatus({ state: "error", lastError: `${error}${retryMsg}` });
        useAppStore.getState().clearSyncingProject(project_id);
        api.listProjects().then(useAppStore.getState().setProjects).catch((err) => console.error("[sync] refresh failed:", err));
      },
      getProjects() {
        return useAppStore.getState().projects;
      },
      isProjectSyncing(projectId) {
        return useAppStore.getState().syncingProjects[projectId] !== undefined;
      },
      setSyncingProject(projectId, phase) {
        useAppStore.getState().setSyncingProject(projectId, phase);
      },
      clearSyncingProject(projectId) {
        useAppStore.getState().clearSyncingProject(projectId);
      },
      setSyncStatus(status) {
        useAppStore.getState().setSyncStatus(status);
      },
      refreshProjects() {
        api.listProjects().then(useAppStore.getState().setProjects).catch((err) => console.error("[sync] refresh failed:", err));
      },
      getSyncingProjectCount() {
        return Object.keys(useAppStore.getState().syncingProjects).length;
      },
    });
    managerRef.current = manager;
    manager.start();

    return () => {
      manager.destroy();
      managerRef.current = null;
    };
  }, []);

  // --- Sync trigger helpers (delegate to manager) ---

  const singleSelectedProject =
    selectedProjectIds.length === 1
      ? projects.find((p) => p.id === selectedProjectIds[0])
      : null;
  const syncDisabled = !singleSelectedProject || !singleSelectedProject.sync_enabled;

  const syncItems = useCallback(async () => {
    if (selectedProjectIds.length === 1) {
      if (syncDisabled) {
        toast.error("Sync is disabled for this project");
        return;
      }
      managerRef.current?.syncProject(selectedProjectIds[0]);
    } else {
      const candidates = selectedProjectIds.length > 1
        ? projects.filter((p) => selectedProjectIds.includes(p.id) && p.sync_enabled)
        : projects.filter((p) => p.sync_enabled);
      if (candidates.length === 0) {
        toast.error("No projects with sync enabled");
        return;
      }
      managerRef.current?.syncMultipleProjects(candidates.map((p) => p.id));
    }
  }, [selectedProjectIds, syncDisabled, projects]);

  const fullSync = useCallback(async () => {
    if (selectedProjectIds.length === 1) {
      const projectId = selectedProjectIds[0];
      const project = useAppStore.getState().projects.find((p) => p.id === projectId);
      if (!project?.sync_enabled) {
        toast.error("Sync is disabled for this project");
        return;
      }
      managerRef.current?.fullSyncProject(projectId);
    } else {
      const candidates = selectedProjectIds.length > 1
        ? projects.filter((p) => selectedProjectIds.includes(p.id) && p.sync_enabled)
        : projects.filter((p) => p.sync_enabled);
      if (candidates.length === 0) {
        toast.error("No projects with sync enabled");
        return;
      }
      for (const p of candidates) {
        managerRef.current?.fullSyncProject(p.id);
      }
    }
  }, [selectedProjectIds, projects]);

  // --- Data fetching ---

  const showDismissedOnly = useAppStore((s) => s.showDismissedOnly);

  const fetchItems = useCallback(async () => {
    try {
      const typeFilter = itemTypeFilter === "all" ? undefined : itemTypeFilter;
      const search = searchQuery.trim() || undefined;
      if (showDismissedOnly) {
        const [response, noteCount] = await Promise.all([
          api.listDismissedItems({ itemType: typeFilter, searchQuery: search }),
          api.getDraftIssueCount(),
        ]);
        setItems(response.items);
        useItemStore.setState({ dismissedCounts: response.dismissed_counts, draftNoteCount: noteCount });
      } else {
        const [response, analyzedIds, noteCount] = await Promise.all([
          api.listItems({ itemType: typeFilter, searchQuery: search }),
          api.getAnalyzedItemIds(),
          api.getDraftIssueCount(),
        ]);
        setItems(response.items);
        setAnalyzedItemIds(analyzedIds);
        useItemStore.setState({ dismissedCounts: response.dismissed_counts, draftNoteCount: noteCount });
      }
    } catch (err) {
      toast.error("Failed to fetch items", { description: errorMessage(err) });
    }
  }, [itemTypeFilter, showDismissedOnly, searchQuery, setItems, setAnalyzedItemIds]);

  useEffect(() => {
    fetchItems();
  }, [fetchItems]);

  // --- CRUD operations ---

  const markRead = useCallback(
    async (id: string) => {
      updateItem(id, { is_read: true });
      try {
        await api.markItemRead(id, true);
      } catch (err) {
        updateItem(id, { is_read: false });
        toast.error("Failed to mark as read", {
          description: errorMessage(err),
          action: { label: "Retry", onClick: () => markRead(id) },
        });
      }
    },
    [updateItem]
  );

  const markUnread = useCallback(
    async (id: string) => {
      updateItem(id, { is_read: false });
      try {
        await api.markItemRead(id, false);
      } catch (err) {
        updateItem(id, { is_read: true });
        toast.error("Failed to mark as unread", {
          description: errorMessage(err),
          action: { label: "Retry", onClick: () => markUnread(id) },
        });
      }
    },
    [updateItem]
  );

  const deleteItem = useCallback(
    async (id: string) => {
      const item = useAppStore.getState().items.find((i) => i.id === id);
      removeItem(id);
      try {
        await api.deleteItem(id);
        // Refresh to update dismissed counts
        await useItemStore.getState().fetchInbox();
      } catch (err) {
        if (item) useAppStore.getState().mergeItems([item]);
        toast.error("Failed to delete item", {
          description: errorMessage(err),
          action: { label: "Retry", onClick: () => deleteItem(id) },
        });
      }
    },
    [removeItem]
  );

  const restoreItem = useCallback(
    async (id: string) => {
      removeItem(id);
      try {
        await api.restoreItem(id);
        // Refresh to update dismissed counts
        await useItemStore.getState().fetchInbox();
      } catch (err) {
        toast.error("Failed to restore item", {
          description: errorMessage(err),
        });
        // Refetch to restore consistent state
        await useItemStore.getState().fetchInbox();
      }
    },
    [removeItem]
  );

  // --- Startup sync (once per session) ---
  useEffect(() => {
    managerRef.current?.startupSync();
  }, [projects]);

  // --- Periodic sync ---
  const refreshInterval = useAppStore((s) => s.refreshInterval);

  useEffect(() => {
    managerRef.current?.startPeriodicSync(refreshInterval);
    return () => {
      managerRef.current?.stopPeriodicSync();
    };
  }, [refreshInterval]);

  // --- Post-onboarding sync ---
  useEffect(() => {
    if (!onboardingJustCompleted) return;
    setOnboardingJustCompleted(false);
    const syncEnabled = projects.filter((p) => p.sync_enabled);
    if (syncEnabled.length > 0) {
      managerRef.current?.syncMultipleProjects(syncEnabled.map((p) => p.id));
    }
  }, [onboardingJustCompleted, projects, setOnboardingJustCompleted]);

  return {
    items,
    selectedItemId,
    setSelectedItemId,
    fetchItems,
    syncItems,
    markRead,
    markUnread,
    deleteItem,
    restoreItem,
    fullSync,
    isSyncing,
    syncDisabled,
  };
}
