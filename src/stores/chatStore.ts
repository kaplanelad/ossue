import { create } from "zustand";
import type { ChatMessage } from "@/types";

interface ChatState {
  messagesByItem: Record<string, ChatMessage[]>;
  loadingItems: Record<string, boolean>;
  setMessages: (itemId: string, messages: ChatMessage[]) => void;
  addMessage: (itemId: string, message: ChatMessage) => void;
  setIsLoading: (itemId: string, loading: boolean) => void;
  clearChat: (itemId: string) => void;
}

export const useChatStore = create<ChatState>((set) => ({
  messagesByItem: {},
  loadingItems: {},
  setMessages: (itemId, messages) =>
    set((state) => ({
      messagesByItem: { ...state.messagesByItem, [itemId]: messages },
    })),
  addMessage: (itemId, message) =>
    set((state) => ({
      messagesByItem: {
        ...state.messagesByItem,
        [itemId]: [...(state.messagesByItem[itemId] ?? []), message],
      },
    })),
  setIsLoading: (itemId, loading) =>
    set((state) => ({
      loadingItems: { ...state.loadingItems, [itemId]: loading },
    })),
  clearChat: (itemId) =>
    set((state) => {
      const { [itemId]: _, ...rest } = state.messagesByItem;
      return { messagesByItem: rest };
    }),
}));
