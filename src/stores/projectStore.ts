import { create } from "zustand";
import type { Project, SyncStatus } from "@/types";

interface ProjectState {
  projects: Project[];
  selectedProjectIds: string[];
  setProjects: (projects: Project[]) => void;
  toggleProjectSelection: (id: string) => void;
  clearProjectSelection: () => void;

  // Per-project sync tracking: projectId -> phase message
  syncingProjects: Record<string, string | null>;
  setSyncingProject: (projectId: string, phase: string | null) => void;
  clearSyncingProject: (projectId: string) => void;

  // Sync status indicator
  syncStatus: SyncStatus;
  setSyncStatus: (status: Partial<SyncStatus>) => void;

  // Loading states
  isPreparingRepo: boolean;
  setIsPreparingRepo: (preparing: boolean) => void;

  // Onboarding completion flag
  onboardingJustCompleted: boolean;
  setOnboardingJustCompleted: (v: boolean) => void;
}

export const useProjectStore = create<ProjectState>((set) => ({
  projects: [],
  selectedProjectIds: [],
  setProjects: (projects) => set({ projects }),
  toggleProjectSelection: (id) =>
    set((state) => ({
      selectedProjectIds: state.selectedProjectIds.includes(id)
        ? state.selectedProjectIds.filter((pid) => pid !== id)
        : [...state.selectedProjectIds, id],
    })),
  clearProjectSelection: () => set({ selectedProjectIds: [] }),

  syncingProjects: {},
  setSyncingProject: (projectId, phase) =>
    set((state) => ({
      syncingProjects: { ...state.syncingProjects, [projectId]: phase },
    })),
  clearSyncingProject: (projectId) =>
    set((state) => {
      const { [projectId]: _, ...rest } = state.syncingProjects;
      return { syncingProjects: rest };
    }),

  syncStatus: { state: "idle", message: null, lastSyncAt: null, lastError: null },
  setSyncStatus: (status) =>
    set((state) => ({ syncStatus: { ...state.syncStatus, ...status } })),

  isPreparingRepo: false,
  setIsPreparingRepo: (preparing) => set({ isPreparingRepo: preparing }),

  onboardingJustCompleted: false,
  setOnboardingJustCompleted: (v) => set({ onboardingJustCompleted: v }),
}));
