import { useMemo } from "react";
import { useAppStore } from "@/stores/appStore";
import type { Item } from "@/types";

export interface AttentionItem {
  item: Item;
  reasons: string[];
}

export function useAttention(): { attentionItems: AttentionItem[]; count: number } {
  const { items } = useAppStore();

  const attentionItems = useMemo(() => {
    const now = Date.now();
    const results: AttentionItem[] = [];

    for (const item of items) {
      if (item.item_status !== "pending" || item.is_read) continue;

      const reasons: string[] = [];

      // PR waiting > 48 hours
      if (item.type_data.kind === "pr" && item.type_data.state === "open") {
        const updated = new Date(item.updated_at).getTime();
        const hoursWaiting = (now - updated) / (1000 * 60 * 60);
        if (hoursWaiting > 48) {
          const days = Math.floor(hoursWaiting / 24);
          reasons.push(`Waiting ${days} days`);
        }
      }

      // High comment activity
      if (item.type_data.kind !== "note" && item.type_data.comments_count >= 5) {
        reasons.push(`${item.type_data.comments_count} comments`);
      }

      // Only include items with reasons
      if (reasons.length > 0) {
        results.push({ item, reasons });
      }
    }

    // Sort by number of reasons (most urgent first)
    results.sort((a, b) => b.reasons.length - a.reasons.length);

    return results;
  }, [items]);

  return { attentionItems, count: attentionItems.length };
}
