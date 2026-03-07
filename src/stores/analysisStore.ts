import { create } from "zustand";
import { useItemStore } from "./itemStore";

export interface AnalysisState {
  content: string;
  isStreaming: boolean;
  status: string | null;
  currentStepIndex: number;
  currentStepLabel: string | null;
}

interface AnalysisStoreState {
  // Active analyses (per-item streaming state)
  activeAnalyses: Record<string, AnalysisState>;
  startAnalysis: (itemId: string) => void;
  setAnalysisStatus: (itemId: string, status: string) => void;
  appendAnalysisContent: (itemId: string, chunk: string) => void;
  setCurrentStepLabel: (itemId: string, label: string) => void;
  endAnalysis: (itemId: string) => void;
  clearAnalysis: (itemId: string) => void;
  resetStreamingContent: (itemId: string) => void;

  // Analyzed item tracking
  analyzedItemIds: Set<string>;
  setAnalyzedItemIds: (ids: string[]) => void;
  addAnalyzedItemId: (id: string) => void;
  removeAnalyzedItemId: (id: string) => void;
}

export const useAnalysisStore = create<AnalysisStoreState>((set) => ({
  activeAnalyses: {},
  startAnalysis: (itemId) =>
    set((state) => {
      if (state.activeAnalyses[itemId]) return state;
      return {
        activeAnalyses: {
          ...state.activeAnalyses,
          [itemId]: { content: "", isStreaming: true, status: "Thinking…", currentStepIndex: 0, currentStepLabel: null },
        },
      };
    }),
  setAnalysisStatus: (itemId, status) =>
    set((state) => {
      const current = state.activeAnalyses[itemId];
      if (!current) return state;
      return {
        activeAnalyses: {
          ...state.activeAnalyses,
          [itemId]: { ...current, status },
        },
      };
    }),
  appendAnalysisContent: (itemId, chunk) =>
    set((state) => {
      const current = state.activeAnalyses[itemId];
      if (!current) return state;
      return {
        activeAnalyses: {
          ...state.activeAnalyses,
          [itemId]: { ...current, content: current.content + chunk, status: null },
        },
      };
    }),
  setCurrentStepLabel: (itemId, label) =>
    set((state) => {
      const current = state.activeAnalyses[itemId];
      if (!current) return state;
      return {
        activeAnalyses: {
          ...state.activeAnalyses,
          [itemId]: { ...current, currentStepLabel: label },
        },
      };
    }),
  endAnalysis: (itemId) =>
    set((state) => {
      const { [itemId]: _, ...rest } = state.activeAnalyses;
      return { activeAnalyses: rest };
    }),
  clearAnalysis: (itemId) =>
    set((state) => {
      const { [itemId]: _, ...rest } = state.activeAnalyses;
      return { activeAnalyses: rest };
    }),
  resetStreamingContent: (itemId) =>
    set((state) => {
      const current = state.activeAnalyses[itemId];
      if (!current) return state;
      return {
        activeAnalyses: {
          ...state.activeAnalyses,
          [itemId]: {
            ...current,
            content: "",
            isStreaming: false,
            status: null,
            currentStepIndex: current.currentStepIndex + 1,
            currentStepLabel: null,
          },
        },
      };
    }),

  analyzedItemIds: new Set<string>(),
  setAnalyzedItemIds: (ids) => set({ analyzedItemIds: new Set(ids) }),
  addAnalyzedItemId: (id) =>
    set((state) => {
      if (state.analyzedItemIds.has(id)) return state;
      const itemState = useItemStore.getState();
      const item = itemState.allItemsCache.get(id) ?? itemState.items.find((i) => i.id === id);
      if (item) {
        itemState.incrementAnalyzedCount(item.project_id, item.item_type);
      }
      return { analyzedItemIds: new Set([...state.analyzedItemIds, id]) };
    }),
  removeAnalyzedItemId: (id) =>
    set((state) => {
      if (!state.analyzedItemIds.has(id)) return state;
      const next = new Set(state.analyzedItemIds);
      next.delete(id);
      const itemState = useItemStore.getState();
      const item = itemState.allItemsCache.get(id) ?? itemState.items.find((i) => i.id === id);
      if (item) {
        itemState.decrementAnalyzedCount(item.project_id, item.item_type);
      }
      return { analyzedItemIds: next };
    }),
}));
