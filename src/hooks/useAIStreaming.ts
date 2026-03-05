import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "@/stores/appStore";

export function useAIStreaming() {
  useEffect(() => {
    const unlistenPromises: Promise<() => void>[] = [];

    const store = useAppStore.getState;

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
        store().endAnalysis(event.payload);
        store().addAnalyzedItemId(event.payload);
      })
    );

    return () => {
      Promise.all(unlistenPromises).then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);
}
