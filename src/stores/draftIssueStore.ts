import { create } from "zustand";
import type { Item } from "@/types";

interface NoteState {
  // Create/Edit note dialog
  isCreateNoteOpen: boolean;
  editingNote: Item | null;
  defaultProjectId: string | null;
  lastUsedProjectId: string | null;
  setLastUsedProjectId: (id: string) => void;
  openCreateNote: (defaultProjectId?: string) => void;
  closeCreateNote: () => void;
  openEditNote: (note: Item) => void;

  // Single selection (for opening panel)
  selectedNoteId: string | null;
  setSelectedNoteId: (id: string | null) => void;

  // Multi-select (for bulk actions)
  selectedNoteIds: string[];
  toggleNoteSelection: (id: string) => void;
  clearNoteSelection: () => void;
  lastClickedNoteIndex: number | null;
  setLastClickedNoteIndex: (index: number | null) => void;

  // Per-note loading states
  isGenerating: Record<string, boolean>;
  setIsGenerating: (id: string, value: boolean) => void;

  isSubmitting: Record<string, boolean>;
  setIsSubmitting: (id: string, value: boolean) => void;
}

export const useDraftIssueStore = create<NoteState>((set) => ({
  isCreateNoteOpen: false,
  editingNote: null,
  defaultProjectId: null,
  lastUsedProjectId: null,
  setLastUsedProjectId: (id) => set({ lastUsedProjectId: id }),
  openCreateNote: (defaultProjectId?: string) =>
    set({ isCreateNoteOpen: true, editingNote: null, defaultProjectId: defaultProjectId ?? null }),
  closeCreateNote: () => set({ isCreateNoteOpen: false, editingNote: null, defaultProjectId: null }),
  openEditNote: (note) => set({ isCreateNoteOpen: true, editingNote: note }),

  selectedNoteId: null,
  setSelectedNoteId: (id) => set({ selectedNoteId: id }),

  selectedNoteIds: [],
  toggleNoteSelection: (id) =>
    set((state) => ({
      selectedNoteIds: state.selectedNoteIds.includes(id)
        ? state.selectedNoteIds.filter((i) => i !== id)
        : [...state.selectedNoteIds, id],
    })),
  clearNoteSelection: () => set({ selectedNoteIds: [], lastClickedNoteIndex: null }),
  lastClickedNoteIndex: null,
  setLastClickedNoteIndex: (index) => set({ lastClickedNoteIndex: index }),

  isGenerating: {},
  setIsGenerating: (id, value) =>
    set((state) => ({
      isGenerating: { ...state.isGenerating, [id]: value },
    })),

  isSubmitting: {},
  setIsSubmitting: (id, value) =>
    set((state) => ({
      isSubmitting: { ...state.isSubmitting, [id]: value },
    })),
}));
