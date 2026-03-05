import { useCallback, useEffect } from "react";
import { toast } from "sonner";
import { errorMessage } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { useAppStore } from "@/stores/appStore";
import * as api from "@/lib/tauri";
import type { AnalysisAction } from "@/types";

export function useChat(itemId: string | null) {
  const { messages, isLoading, setMessages, addMessage, setIsLoading, clearChat } =
    useChatStore();

  const analysis = useAppStore(
    (s) => (itemId ? s.activeAnalyses[itemId] : undefined)
  );
  const startAnalysis = useAppStore((s) => s.startAnalysis);
  const clearAnalysis = useAppStore((s) => s.clearAnalysis);

  const streamingContent = analysis?.content ?? "";
  const isStreaming = analysis?.isStreaming ?? false;
  const analysisStatus = analysis?.status ?? null;

  // Load chat history when item changes
  useEffect(() => {
    if (!itemId) {
      clearChat();
      return;
    }

    const loadMessages = async () => {
      setIsLoading(true);
      try {
        const msgs = await api.getChatMessages(itemId);
        // Only update if this item is still selected
        if (useAppStore.getState().selectedItemId === itemId) {
          setMessages(msgs);
        }
      } catch (err) {
        console.error("Failed to load chat messages:", err);
      } finally {
        if (useAppStore.getState().selectedItemId === itemId) {
          setIsLoading(false);
        }
      }
    };

    loadMessages();
  }, [itemId, clearChat, setIsLoading, setMessages]);

  const sendMessage = useCallback(
    async (message: string) => {
      if (!itemId) return;

      // Optimistically show user message immediately
      addMessage({
        id: `temp-${Date.now()}`,
        item_id: itemId,
        role: "user",
        content: message,
        created_at: new Date().toISOString(),
        input_tokens: null,
        output_tokens: null,
        model: null,
      });

      setIsLoading(true);
      startAnalysis(itemId);
      try {
        await api.sendChatMessage(itemId, message);
        const msgs = await api.getChatMessages(itemId);
        if (useAppStore.getState().selectedItemId === itemId) {
          setMessages(msgs);
        }
        clearAnalysis(itemId);
      } catch (err) {
        console.error("Failed to send message:", err);
        toast.error(errorMessage(err));
        clearAnalysis(itemId);
      } finally {
        if (useAppStore.getState().selectedItemId === itemId) {
          setIsLoading(false);
        }
      }
    },
    [itemId, addMessage, setIsLoading, setMessages, startAnalysis, clearAnalysis]
  );

  const analyzeItem = useCallback(async () => {
    if (!itemId) return;

    setIsLoading(true);
    startAnalysis(itemId);
    try {
      await api.autoAnalyzeItem(itemId);
      const msgs = await api.getChatMessages(itemId);
      if (useAppStore.getState().selectedItemId === itemId) {
        setMessages(msgs);
      }
      clearAnalysis(itemId);
    } catch (err) {
      if (errorMessage(err) !== "Already analyzed") {
        console.error("Failed to analyze item:", err);
        toast.error(errorMessage(err));
      }
      clearAnalysis(itemId);
    } finally {
      if (useAppStore.getState().selectedItemId === itemId) {
        setIsLoading(false);
      }
    }
  }, [itemId, setIsLoading, setMessages, startAnalysis, clearAnalysis]);

  const analyzeWithAction = useCallback(
    async (action: AnalysisAction) => {
      if (!itemId) return;

      setIsLoading(true);
      startAnalysis(itemId);
      try {
        await api.analyzeItemAction({ item_id: itemId, action });
        const msgs = await api.getChatMessages(itemId);
        if (useAppStore.getState().selectedItemId === itemId) {
          setMessages(msgs);
        }
        clearAnalysis(itemId);
      } catch (err) {
        console.error("Failed to analyze item:", err);
        toast.error(errorMessage(err));
        clearAnalysis(itemId);
      } finally {
        if (useAppStore.getState().selectedItemId === itemId) {
          setIsLoading(false);
        }
      }
    },
    [itemId, setIsLoading, setMessages, startAnalysis, clearAnalysis]
  );

  const clearMessages = useCallback(async () => {
    if (!itemId) return;
    await api.clearChat(itemId);
    clearChat();
    clearAnalysis(itemId);
  }, [itemId, clearChat, clearAnalysis]);

  return {
    messages,
    streamingContent,
    isStreaming,
    isLoading,
    analysisStatus,
    sendMessage,
    analyzeItem,
    analyzeWithAction,
    clearMessages,
  };
}
