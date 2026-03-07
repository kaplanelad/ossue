import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "@/stores/appStore";
import { useChatStore } from "@/stores/chatStore";
import type { ChatMessage } from "@/types";

export function useAIStreaming() {
  useEffect(() => {
    const unlistenPromises: Promise<() => void>[] = [];

    const store = useAppStore.getState;
    const chatStore = useChatStore.getState;

    unlistenPromises.push(
      listen<{ item_id: string; status: string }>(
        "ai-analysis-progress",
        (event) => {
          store().setAnalysisStatus(
            event.payload.item_id,
            event.payload.status
          );
        }
      )
    );

    unlistenPromises.push(
      listen<string>("ai-stream-start", (event) => {
        store().startAnalysis(event.payload);
      })
    );

    unlistenPromises.push(
      listen<{ item_id: string; chunk: string }>(
        "ai-stream-chunk",
        (event) => {
          store().appendAnalysisContent(
            event.payload.item_id,
            event.payload.chunk
          );
        }
      )
    );

    unlistenPromises.push(
      listen<string>("ai-stream-end", (event) => {
        // Reset streaming content but keep analysis active for next step
        store().resetStreamingContent(event.payload);
      })
    );

    // Multi-step events
    unlistenPromises.push(
      listen<{ item_id: string; message: ChatMessage }>(
        "ai-step-user-message",
        (event) => {
          // Only add if the currently selected item matches
          if (store().selectedItemId === event.payload.item_id) {
            chatStore().addMessage(event.payload.message);
          }
        }
      )
    );

    unlistenPromises.push(
      listen<{ item_id: string; message: ChatMessage }>(
        "ai-step-assistant-message",
        (event) => {
          if (store().selectedItemId === event.payload.item_id) {
            chatStore().addMessage(event.payload.message);
          }
        }
      )
    );

    unlistenPromises.push(
      listen<string>("ai-analysis-complete", (event) => {
        store().endAnalysis(event.payload);
        store().addAnalyzedItemId(event.payload);
      })
    );

    return () => {
      Promise.all(unlistenPromises).then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);
}
