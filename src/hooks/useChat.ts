import { useCallback, useEffect } from "react";
import { toast } from "sonner";
import { errorMessage } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { useAppStore } from "@/stores/appStore";
import * as api from "@/lib/tauri";
import type { AnalysisAction, ChatMessage } from "@/types";

const EMPTY_MESSAGES: ChatMessage[] = [];

export function useChat(itemId: string | null) {
  const messages = useChatStore((s) => s.messagesByItem[itemId ?? ""] ?? EMPTY_MESSAGES);
  const isLoading = useChatStore((s) => s.loadingItems[itemId ?? ""] ?? false);

  const analysis = useAppStore(
    (s) => (itemId ? s.activeAnalyses[itemId] : undefined)
  );
  const startAnalysis = useAppStore((s) => s.startAnalysis);
  const clearAnalysis = useAppStore((s) => s.clearAnalysis);

  const streamingContent = analysis?.content ?? "";
  const isStreaming = analysis?.isStreaming ?? false;
  const analysisStatus = analysis?.status ?? null;
  const currentStepIndex = analysis?.currentStepIndex ?? 0;
  const currentStepLabel = analysis?.currentStepLabel ?? null;

  // Load chat history when item changes
  useEffect(() => {
    if (!itemId) return;

    // Skip DB reload if messages already cached
    if (useChatStore.getState().messagesByItem[itemId] !== undefined) return;

    const loadMessages = async () => {
      useChatStore.getState().setIsLoading(itemId, true);
      try {
        const msgs = await api.getChatMessages(itemId);
        useChatStore.getState().setMessages(itemId, msgs);
      } catch (err) {
        console.error("Failed to load chat messages:", err);
      } finally {
        useChatStore.getState().setIsLoading(itemId, false);
      }
    };

    loadMessages();
  }, [itemId]);

  const sendMessage = useCallback(
    async (message: string) => {
      if (!itemId) return;

      // Optimistically show user message immediately
      useChatStore.getState().addMessage(itemId, {
        id: `temp-${Date.now()}`,
        item_id: itemId,
        role: "user",
        content: message,
        created_at: new Date().toISOString(),
        input_tokens: null,
        output_tokens: null,
        model: null,
      });

      useChatStore.getState().setIsLoading(itemId, true);
      startAnalysis(itemId);
      try {
        await api.sendChatMessage(itemId, message);
        const msgs = await api.getChatMessages(itemId);
        useChatStore.getState().setMessages(itemId, msgs);
        clearAnalysis(itemId);
      } catch (err) {
        console.error("Failed to send message:", err);
        toast.error(errorMessage(err));
      } finally {
        useChatStore.getState().setIsLoading(itemId, false);
      }
    },
    [itemId, startAnalysis, clearAnalysis]
  );

  const analyzeItem = useCallback(async () => {
    if (!itemId) return;

    useChatStore.getState().setIsLoading(itemId, true);
    startAnalysis(itemId);
    try {
      await api.autoAnalyzeItem(itemId);
      const msgs = await api.getChatMessages(itemId);
      useChatStore.getState().setMessages(itemId, msgs);
      clearAnalysis(itemId);
    } catch (err) {
      if (errorMessage(err) !== "Already analyzed") {
        console.error("Failed to analyze item:", err);
        toast.error(errorMessage(err));
      }
      clearAnalysis(itemId);
    } finally {
      useChatStore.getState().setIsLoading(itemId, false);
    }
  }, [itemId, startAnalysis, clearAnalysis]);

  const analyzeWithAction = useCallback(
    async (action: AnalysisAction) => {
      if (!itemId) return;

      useChatStore.getState().setIsLoading(itemId, true);
      startAnalysis(itemId);
      try {
        // Messages arrive incrementally via events during the command execution
        await api.analyzeItemAction({ item_id: itemId, action });
        // Reload from DB for consistency (catches any missed events)
        const msgs = await api.getChatMessages(itemId);
        useChatStore.getState().setMessages(itemId, msgs);
        // For multi-step analyze, endAnalysis is handled by "ai-analysis-complete" event.
        // For single-step draft_response, clear here.
        if (action !== "analyze") {
          clearAnalysis(itemId);
          useAppStore.getState().addAnalyzedItemId(itemId);
        }
      } catch (err) {
        console.error("Failed to analyze item:", err);
        toast.error(errorMessage(err));
        clearAnalysis(itemId);
      } finally {
        useChatStore.getState().setIsLoading(itemId, false);
      }
    },
    [itemId, startAnalysis, clearAnalysis]
  );

  const removeAnalyzedItemId = useAppStore((s) => s.removeAnalyzedItemId);

  const clearMessages = useCallback(async () => {
    if (!itemId) return;
    await api.clearChat(itemId);
    useChatStore.getState().clearChat(itemId);
    clearAnalysis(itemId);
    removeAnalyzedItemId(itemId);
  }, [itemId, clearAnalysis, removeAnalyzedItemId]);

  return {
    messages,
    streamingContent,
    isStreaming,
    isLoading,
    analysisStatus,
    currentStepIndex,
    currentStepLabel,
    sendMessage,
    analyzeItem,
    analyzeWithAction,
    clearMessages,
  };
}
