import { create } from "zustand";
import { listItems as listItemsApi, listDismissedItems as listDismissedItemsApi, getDraftIssueCount } from "@/lib/tauri";
import type { Item, ItemTypeFilter, DismissedCount } from "@/types";
import { useProjectStore } from "./projectStore";

interface ItemState {
  // Items
  items: Item[];
  selectedItemId: string | null;
  itemTypeFilter: ItemTypeFilter;
  setItems: (items: Item[]) => void;
  setSelectedItemId: (id: string | null) => void;
  setItemTypeFilter: (filter: ItemTypeFilter) => void;
  updateItem: (id: string, updates: Partial<Item>) => void;
  removeItem: (id: string) => void;
  mergeItems: (newItems: Item[]) => void;

  // Multi-selection
  selectedItemIds: string[];
  toggleItemSelection: (id: string) => void;
  selectAllItems: (ids: string[]) => void;
  clearSelection: () => void;

  // Focus & range selection
  lastClickedIndex: number | null;
  focusedIndex: number | null;
  setLastClickedIndex: (index: number | null) => void;
  setFocusedIndex: (index: number | null) => void;

  // Filters
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  showAnalyzedOnly: boolean;
  setShowAnalyzedOnly: (show: boolean) => void;
  showStarredOnly: boolean;
  setShowStarredOnly: (show: boolean) => void;
  showDismissedOnly: boolean;
  setShowDismissedOnly: (show: boolean) => void;

  // Dismissed counts (per project+type)
  dismissedCounts: DismissedCount[];

  // Persistent note count (independent of type filter)
  draftNoteCount: number;

  // Pagination
  nextCursor: string | null;
  hasMore: boolean;
  isLoadingMore: boolean;
  fetchInbox: (filters?: { projectId?: string; itemType?: string; starredOnly?: boolean }) => Promise<void>;
  fetchMore: () => Promise<void>;
  refreshInbox: () => Promise<void>;
}

export const useItemStore = create<ItemState>((set) => ({
  items: [],
  selectedItemId: null,
  itemTypeFilter: "all",
  setItems: (items) => set({ items }),
  setSelectedItemId: (id) => set({ selectedItemId: id }),
  setItemTypeFilter: (filter) => set({ itemTypeFilter: filter, selectedItemIds: [], selectedItemId: null }),
  updateItem: (id, updates) =>
    set((state) => ({
      items: state.items.map((item) =>
        item.id === id ? { ...item, ...updates } : item
      ),
    })),
  removeItem: (id) =>
    set((state) => ({
      items: state.items.filter((item) => item.id !== id),
      selectedItemId: state.selectedItemId === id ? null : state.selectedItemId,
      selectedItemIds: state.selectedItemIds.filter((i) => i !== id),
    })),

  mergeItems: (newItems) =>
    set((state) => {
      const itemMap = new Map(state.items.map((item) => [item.id, item]));
      const removedIds = new Set<string>();
      for (const item of newItems) {
        if (item.item_status !== "pending") {
          itemMap.delete(item.id);
          removedIds.add(item.id);
        } else {
          itemMap.set(item.id, item);
        }
      }
      const merged = Array.from(itemMap.values());
      merged.sort((a, b) => b.updated_at.localeCompare(a.updated_at));
      return {
        items: merged,
        selectedItemId:
          state.selectedItemId && removedIds.has(state.selectedItemId)
            ? null
            : state.selectedItemId,
        selectedItemIds:
          removedIds.size > 0
            ? state.selectedItemIds.filter((id) => !removedIds.has(id))
            : state.selectedItemIds,
      };
    }),

  selectedItemIds: [],
  toggleItemSelection: (id) =>
    set((state) => ({
      selectedItemIds: state.selectedItemIds.includes(id)
        ? state.selectedItemIds.filter((i) => i !== id)
        : [...state.selectedItemIds, id],
    })),
  selectAllItems: (ids) => set({ selectedItemIds: ids }),
  clearSelection: () => set({ selectedItemIds: [], lastClickedIndex: null }),

  lastClickedIndex: null,
  focusedIndex: null,
  setLastClickedIndex: (index) => set({ lastClickedIndex: index }),
  setFocusedIndex: (index) => set({ focusedIndex: index }),

  searchQuery: "",
  setSearchQuery: (query) => set({ searchQuery: query, selectedItemIds: [] }),

  showAnalyzedOnly: false,
  setShowAnalyzedOnly: (show) => set({ showAnalyzedOnly: show, selectedItemIds: [] }),

  showStarredOnly: false,
  setShowStarredOnly: (show) => set({ showStarredOnly: show, selectedItemIds: [] }),

  showDismissedOnly: false,
  setShowDismissedOnly: (show) => set({ showDismissedOnly: show, selectedItemIds: [] }),

  dismissedCounts: [],

  draftNoteCount: 0,

  nextCursor: null,
  hasMore: false,
  isLoadingMore: false,
  fetchInbox: async (filters) => {
    try {
      const itemState = useItemStore.getState();
      const projectState = useProjectStore.getState();
      const projectIds = filters?.projectId
        ? [filters.projectId]
        : projectState.selectedProjectIds.length > 0
          ? projectState.selectedProjectIds
          : undefined;
      const itemType = filters?.itemType ?? (itemState.itemTypeFilter !== "all" ? itemState.itemTypeFilter : undefined);
      const starredOnly = filters?.starredOnly ?? itemState.showStarredOnly;

      const searchQuery = itemState.searchQuery.trim() || undefined;
      const [response, noteCount] = await Promise.all([
        itemState.showDismissedOnly
          ? listDismissedItemsApi({
              projectId: projectIds?.[0],
              itemType,
              searchQuery,
              pageSize: 50,
            })
          : listItemsApi({
              projectId: projectIds?.[0],
              itemType,
              starredOnly: starredOnly || undefined,
              searchQuery,
              pageSize: 50,
            }),
        getDraftIssueCount(),
      ]);
      set({
        items: response.items,
        nextCursor: response.next_cursor,
        hasMore: response.has_more,
        dismissedCounts: response.dismissed_counts,
        draftNoteCount: noteCount,
      });
    } catch (e) {
      console.error("Failed to fetch inbox:", e);
    }
  },
  fetchMore: async () => {
    const state = useItemStore.getState();
    if (!state.hasMore || state.isLoadingMore || !state.nextCursor) return;

    set({ isLoadingMore: true });
    try {
      const projectState = useProjectStore.getState();
      const itemType = state.itemTypeFilter !== "all" ? state.itemTypeFilter : undefined;
      const projectId = projectState.selectedProjectIds.length === 1
        ? projectState.selectedProjectIds[0]
        : undefined;

      const searchQuery = state.searchQuery.trim() || undefined;
      const response = state.showDismissedOnly
        ? await listDismissedItemsApi({
            projectId,
            itemType,
            searchQuery,
            cursor: state.nextCursor,
            pageSize: 50,
          })
        : await listItemsApi({
            projectId,
            itemType,
            starredOnly: state.showStarredOnly || undefined,
            searchQuery,
            cursor: state.nextCursor,
            pageSize: 50,
          });
      set((prev) => ({
        items: [...prev.items, ...response.items],
        nextCursor: response.next_cursor,
        hasMore: response.has_more,
        dismissedCounts: response.dismissed_counts,
        isLoadingMore: false,
      }));
    } catch (e) {
      console.error("Failed to fetch more items:", e);
      set({ isLoadingMore: false });
    }
  },
  refreshInbox: async () => {
    await useItemStore.getState().fetchInbox();
  },
}));
