import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu";
import { Eye, EyeOff, Inbox, Sparkles, X } from "lucide-react";
import type { Item, AnalysisAction } from "@/types";

interface BulkActionBarProps {
  selectedItems: Item[];
  onMarkRead: () => void;
  onMarkUnread: () => void;
  onDelete: () => void;
  onRestore?: () => void;
  isDismissedView?: boolean;
  onAiAction: (action: AnalysisAction) => void;
  onClearSelection: () => void;
}

const AI_ACTIONS_BY_TYPE: Record<string, AnalysisAction[]> = {
  pr: ["analyze", "draft_response"],
  issue: ["analyze", "draft_response"],
  discussion: ["analyze", "draft_response"],
};

const ACTION_LABELS: Record<AnalysisAction, string> = {
  analyze: "Analyze",
  draft_response: "Draft Response",
};

export function BulkActionBar({
  selectedItems,
  onMarkRead,
  onMarkUnread,
  onDelete,
  onRestore,
  isDismissedView,
  onAiAction,
  onClearSelection,
}: BulkActionBarProps) {
  const availableAiActions = useMemo(() => {
    if (selectedItems.length === 0) return [];

    const actionSets = selectedItems.map(
      (item) => new Set(AI_ACTIONS_BY_TYPE[item.item_type] ?? [])
    );

    // Intersection of all sets
    const first = actionSets[0];
    return [...first].filter((action) =>
      actionSets.every((s) => s.has(action))
    ) as AnalysisAction[];
  }, [selectedItems]);

  if (selectedItems.length === 0) return null;

  return (
    <div className="flex items-center gap-2 border-t border-t-primary/20 bg-background px-4 py-2.5">
      <span className="text-xs font-medium text-muted-foreground">
        {selectedItems.length} selected
      </span>
      <div className="ml-auto flex items-center gap-1">
        <Button variant="ghost" size="sm" onClick={onMarkRead}>
          <Eye className="mr-1 h-3.5 w-3.5" />
          Read
        </Button>
        <Button variant="ghost" size="sm" onClick={onMarkUnread}>
          <EyeOff className="mr-1 h-3.5 w-3.5" />
          Unread
        </Button>
        {isDismissedView && onRestore ? (
          <Button variant="ghost" size="sm" onClick={onRestore}>
            <Inbox className="mr-1 h-3.5 w-3.5" />
            Move to Inbox
          </Button>
        ) : (
          <Button variant="ghost" size="sm" onClick={onDelete}>
            <EyeOff className="mr-1 h-3.5 w-3.5" />
            Dismiss
          </Button>
        )}
        {availableAiActions.length > 0 && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <Sparkles className="mr-1 h-3.5 w-3.5" />
                AI
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {availableAiActions.map((action) => (
                <DropdownMenuItem
                  key={action}
                  onClick={() => onAiAction(action)}
                >
                  {ACTION_LABELS[action]}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClearSelection} aria-label="Clear selection">
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}
