import { create } from "zustand";
import type { PageView } from "@/types";

interface NavigationState {
  currentPage: PageView;
  setCurrentPage: (page: PageView) => void;
}

export const useNavigationStore = create<NavigationState>((set) => ({
  currentPage: "onboarding",
  setCurrentPage: (page) => set({ currentPage: page }),
}));
