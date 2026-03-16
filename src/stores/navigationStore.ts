import { create } from "zustand";
import type { PageView } from "@/types";

interface NavigationState {
  currentPage: PageView;
  settingsTab: string | null;
  setCurrentPage: (page: PageView) => void;
  openSettings: (tab?: string) => void;
}

export const useNavigationStore = create<NavigationState>((set) => ({
  currentPage: "onboarding",
  settingsTab: null,
  setCurrentPage: (page) => set({ currentPage: page, settingsTab: null }),
  openSettings: (tab) => set({ currentPage: "settings", settingsTab: tab ?? null }),
}));
