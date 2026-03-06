// DEPRECATED: Use domain-specific stores instead.
// This file re-exports a combined useAppStore hook for backward compatibility.
// Prefer importing from the specific domain store directly:
//   - useNavigationStore (navigationStore.ts)
//   - useProjectStore (projectStore.ts)
//   - useItemStore (itemStore.ts)
//   - useAnalysisStore (analysisStore.ts)
//   - useUiStore (uiStore.ts)

import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { Project, Item, ItemTypeFilter, PageView, SyncStatus } from "@/types";

import { useNavigationStore } from "./navigationStore";
import { useProjectStore } from "./projectStore";
import { useItemStore } from "./itemStore";
import { useAnalysisStore } from "./analysisStore";
import type { AnalysisState } from "./analysisStore";
import { useUiStore } from "./uiStore";
import type { ThemePreference, ResolvedTheme } from "./uiStore";

export type { ThemePreference, ResolvedTheme };
export { useNavigationStore } from "./navigationStore";
export { useProjectStore } from "./projectStore";
export { useItemStore } from "./itemStore";
export { useAnalysisStore } from "./analysisStore";
export { useUiStore } from "./uiStore";

// Combined state interface matching the original AppState
interface AppState {
  // Navigation
  currentPage: PageView;
  setCurrentPage: (page: PageView) => void;

  // Projects
  projects: Project[];
  selectedProjectIds: string[];
  setProjects: (projects: Project[]) => void;
  toggleProjectSelection: (id: string) => void;
  clearProjectSelection: () => void;

  // Items
  items: Item[];
  selectedItemId: string | null;
  itemTypeFilter: ItemTypeFilter;
  setItems: (items: Item[]) => void;
  setSelectedItemId: (id: string | null) => void;
  setItemTypeFilter: (filter: ItemTypeFilter) => void;
  updateItem: (id: string, updates: Partial<Item>) => void;
  removeItem: (id: string) => void;

  // Selection
  selectedItemIds: string[];
  toggleItemSelection: (id: string) => void;
  selectAllItems: (ids: string[]) => void;
  clearSelection: () => void;

  // Loading states
  isPreparingRepo: boolean;
  setIsPreparingRepo: (preparing: boolean) => void;

  // Per-project sync tracking
  syncingProjects: Record<string, string | null>;
  setSyncingProject: (projectId: string, phase: string | null) => void;
  clearSyncingProject: (projectId: string) => void;

  // Sync status indicator
  syncStatus: SyncStatus;
  setSyncStatus: (status: Partial<SyncStatus>) => void;

  // Onboarding completion flag
  onboardingJustCompleted: boolean;
  setOnboardingJustCompleted: (v: boolean) => void;

  mergeItems: (newItems: Item[]) => void;

  // Pagination
  nextCursor: string | null;
  hasMore: boolean;
  isLoadingMore: boolean;
  fetchInbox: (filters?: { projectId?: string; itemType?: string; starredOnly?: boolean }) => Promise<void>;
  fetchMore: () => Promise<void>;
  refreshInbox: () => Promise<void>;

  // Active analyses
  activeAnalyses: Record<string, AnalysisState>;
  startAnalysis: (itemId: string) => void;
  setAnalysisStatus: (itemId: string, status: string) => void;
  appendAnalysisContent: (itemId: string, chunk: string) => void;
  endAnalysis: (itemId: string) => void;
  clearAnalysis: (itemId: string) => void;

  // Analyzed item tracking
  analyzedItemIds: Set<string>;
  setAnalyzedItemIds: (ids: string[]) => void;
  addAnalyzedItemId: (id: string) => void;

  // Search
  searchQuery: string;
  setSearchQuery: (query: string) => void;

  // AI chat filter
  showAnalyzedOnly: boolean;
  setShowAnalyzedOnly: (show: boolean) => void;

  // Starred filter
  showStarredOnly: boolean;
  setShowStarredOnly: (show: boolean) => void;

  // Dismissed filter
  showDismissedOnly: boolean;
  setShowDismissedOnly: (show: boolean) => void;
  dismissedCounts: import("@/types").DismissedCount[];
  itemTypeCounts: import("@/types").ItemTypeCount[];

  // Persistent note count
  draftNoteCount: number;

  // Refresh interval
  refreshInterval: number;
  setRefreshInterval: (interval: number) => void;

  // Theme
  themePreference: ThemePreference;
  resolvedTheme: ResolvedTheme;
  setThemePreference: (pref: ThemePreference) => void;
  setResolvedTheme: (theme: ResolvedTheme) => void;
}

function getCompositeState(): AppState {
  const nav = useNavigationStore.getState();
  const proj = useProjectStore.getState();
  const item = useItemStore.getState();
  const analysis = useAnalysisStore.getState();
  const ui = useUiStore.getState();

  return {
    // Navigation
    currentPage: nav.currentPage,
    setCurrentPage: nav.setCurrentPage,

    // Projects - wrap to handle cross-store side effects
    projects: proj.projects,
    selectedProjectIds: proj.selectedProjectIds,
    setProjects: proj.setProjects,
    toggleProjectSelection: (id: string) => {
      proj.toggleProjectSelection(id);
      item.clearSelection();
    },
    clearProjectSelection: () => {
      proj.clearProjectSelection();
      item.clearSelection();
    },

    // Items
    items: item.items,
    selectedItemId: item.selectedItemId,
    itemTypeFilter: item.itemTypeFilter,
    setItems: item.setItems,
    setSelectedItemId: item.setSelectedItemId,
    setItemTypeFilter: item.setItemTypeFilter,
    updateItem: item.updateItem,
    removeItem: item.removeItem,
    mergeItems: item.mergeItems,

    // Selection
    selectedItemIds: item.selectedItemIds,
    toggleItemSelection: item.toggleItemSelection,
    selectAllItems: item.selectAllItems,
    clearSelection: item.clearSelection,

    // Loading / sync
    isPreparingRepo: proj.isPreparingRepo,
    setIsPreparingRepo: proj.setIsPreparingRepo,
    syncingProjects: proj.syncingProjects,
    setSyncingProject: proj.setSyncingProject,
    clearSyncingProject: proj.clearSyncingProject,
    syncStatus: proj.syncStatus,
    setSyncStatus: proj.setSyncStatus,
    onboardingJustCompleted: proj.onboardingJustCompleted,
    setOnboardingJustCompleted: proj.setOnboardingJustCompleted,

    // Pagination
    nextCursor: item.nextCursor,
    hasMore: item.hasMore,
    isLoadingMore: item.isLoadingMore,
    fetchInbox: item.fetchInbox,
    fetchMore: item.fetchMore,
    refreshInbox: item.refreshInbox,

    // Analysis
    activeAnalyses: analysis.activeAnalyses,
    startAnalysis: analysis.startAnalysis,
    setAnalysisStatus: analysis.setAnalysisStatus,
    appendAnalysisContent: analysis.appendAnalysisContent,
    endAnalysis: analysis.endAnalysis,
    clearAnalysis: analysis.clearAnalysis,
    analyzedItemIds: analysis.analyzedItemIds,
    setAnalyzedItemIds: analysis.setAnalyzedItemIds,
    addAnalyzedItemId: analysis.addAnalyzedItemId,

    // Search
    searchQuery: item.searchQuery,
    setSearchQuery: item.setSearchQuery,

    // Filters
    showAnalyzedOnly: item.showAnalyzedOnly,
    setShowAnalyzedOnly: item.setShowAnalyzedOnly,
    showStarredOnly: item.showStarredOnly,
    setShowStarredOnly: item.setShowStarredOnly,

    // Dismissed
    showDismissedOnly: item.showDismissedOnly,
    setShowDismissedOnly: item.setShowDismissedOnly,
    dismissedCounts: item.dismissedCounts,
    itemTypeCounts: item.itemTypeCounts,

    // Note count
    draftNoteCount: item.draftNoteCount,

    // UI
    refreshInterval: ui.refreshInterval,
    setRefreshInterval: ui.setRefreshInterval,
    themePreference: ui.themePreference,
    resolvedTheme: ui.resolvedTheme,
    setThemePreference: ui.setThemePreference,
    setResolvedTheme: ui.setResolvedTheme,
  };
}

// Create a composite store that syncs with all domain stores
export const useAppStore = create<AppState>()(
  subscribeWithSelector(() => getCompositeState())
);

// Sync domain store changes into the composite store
const syncComposite = () => useAppStore.setState(getCompositeState());

useNavigationStore.subscribe(syncComposite);
useProjectStore.subscribe(syncComposite);
useItemStore.subscribe(syncComposite);
useAnalysisStore.subscribe(syncComposite);
useUiStore.subscribe(syncComposite);
