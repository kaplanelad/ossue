import { Button } from "@/components/ui/button";
import { Sparkles, Trash2, X, Upload } from "lucide-react";
import type { Item } from "@/types";

interface NoteBulkActionBarProps {
  selectedNotes: Item[];
  onGenerateIssue: () => void;
  onPublish: () => void;
  onDelete: () => void;
  onClearSelection: () => void;
}

export function NoteBulkActionBar({
  selectedNotes,
  onGenerateIssue,
  onPublish,
  onDelete,
  onClearSelection,
}: NoteBulkActionBarProps) {
  if (selectedNotes.length === 0) return null;

  const hasDrafts = selectedNotes.some((n) => n.type_data.kind === "note" && n.type_data.draft_status === "draft");
  const hasReady = selectedNotes.some((n) => n.type_data.kind === "note" && n.type_data.draft_status === "ready");

  return (
    <div className="flex items-center gap-2 border-t border-t-amber-500/20 dark:border-t-amber-400/20 bg-background px-4 py-2.5">
      <span className="text-xs font-medium text-muted-foreground">
        {selectedNotes.length} selected
      </span>
      <div className="ml-auto flex items-center gap-1">
        {hasDrafts && (
          <Button variant="ghost" size="sm" onClick={onGenerateIssue}>
            <Sparkles className="mr-1 h-3.5 w-3.5" />
            Generate Issue
          </Button>
        )}
        {hasReady && (
          <Button variant="ghost" size="sm" onClick={onPublish}>
            <Upload className="mr-1 h-3.5 w-3.5" />
            Publish
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          className="text-destructive hover:text-destructive"
          onClick={onDelete}
        >
          <Trash2 className="mr-1 h-3.5 w-3.5" />
          Delete
        </Button>
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onClearSelection} aria-label="Clear selection">
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}
