import { create } from "zustand";

export type ThemePreference = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

interface UiState {
  // Theme
  themePreference: ThemePreference;
  resolvedTheme: ResolvedTheme;
  setThemePreference: (pref: ThemePreference) => void;
  setResolvedTheme: (theme: ResolvedTheme) => void;

  // Refresh interval (seconds)
  refreshInterval: number;
  setRefreshInterval: (interval: number) => void;

  // Group by repository
  groupByRepository: boolean;
  setGroupByRepository: (value: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  themePreference: (localStorage.getItem("theme") as ThemePreference) || "system",
  resolvedTheme: "light",
  setThemePreference: (pref) => {
    localStorage.setItem("theme", pref);
    set({ themePreference: pref });
  },
  setResolvedTheme: (theme) => set({ resolvedTheme: theme }),

  refreshInterval: 1800,
  setRefreshInterval: (interval) => set({ refreshInterval: interval }),

  groupByRepository: localStorage.getItem("groupByRepository") === "true",
  setGroupByRepository: (value) => {
    localStorage.setItem("groupByRepository", String(value));
    set({ groupByRepository: value });
  },
}));
