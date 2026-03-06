import { forwardRef } from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { StickyNote, Trash2, Sparkles, Loader2, Pencil, Star } from "lucide-react";
import type { Item } from "@/types";
import { formatTimeAgo } from "@/lib/utils";

const statusConfig: Record<
  string,
  { label: string; className: string }
> = {
  draft: {
    label: "Draft",
    className: "bg-amber-500/10 text-amber-700 dark:text-amber-400",
  },
  ready: {
    label: "Ready",
    className: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
  },
  submitted: {
    label: "Submitted",
    className: "bg-purple-500/10 text-purple-700 dark:text-purple-400",
  },
};


interface NoteItemProps {
  note: Item;
  projectLabel?: string;
  isSelected: boolean;
  isChecked: boolean;
  isFocused?: boolean;
  isGenerating: boolean;
  onToggleSelect: (e: React.MouseEvent) => void;
  onClick: () => void;
  onDelete: () => void;
  onGenerate: () => void;
  onEdit: () => void;
  onToggleStar: () => void;
}

export const NoteItem = forwardRef<HTMLElement, NoteItemProps>(function NoteItem({
  note,
  projectLabel,
  isSelected,
  isChecked,
  isFocused,
  isGenerating,
  onToggleSelect,
  onClick,
  onDelete,
  onGenerate,
  onEdit,
  onToggleStar,
}, ref) {
  const noteData = note.type_data.kind === "note" ? note.type_data : null;
  const status = noteData?.draft_status || "draft";
  const config = statusConfig[status] || statusConfig.draft;
  const rawContent = noteData?.raw_content || "";
  const displayTitle =
    note.title || rawContent.slice(0, 120) + (rawContent.length > 120 ? "..." : "");
  const timeAgo = formatTimeAgo(note.updated_at || note.created_at);
  const labels = noteData?.labels ?? [];

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <button
          ref={ref as React.Ref<HTMLButtonElement>}
          className={`note-item-enter block w-full border-l-2 border-b px-4 py-3.5 text-left transition-colors hover:bg-muted/50 ${
            isSelected ? "bg-muted border-l-amber-500 dark:border-l-amber-400" : isFocused ? "border-l-primary/30" : "border-l-transparent"
          }`}
          onClick={onClick}
        >
          <div className="flex items-center gap-2 overflow-hidden">
            <Checkbox
              checked={isChecked}
              onClick={(e) => {
                e.stopPropagation();
                onToggleSelect(e as unknown as React.MouseEvent);
              }}
              className="shrink-0"
            />
            <span className="shrink-0">
              <StickyNote className="h-4 w-4 text-amber-500 dark:text-amber-400" />
            </span>
            <span className={`truncate text-sm font-medium`}>
              {displayTitle}
            </span>
            {isGenerating && (
              <Loader2 className="ml-auto h-3.5 w-3.5 shrink-0 animate-spin text-blue-500" />
            )}
            {!isGenerating && status === "ready" && (
              <Sparkles className="ml-auto h-3.5 w-3.5 shrink-0 text-emerald-500" />
            )}
            <button
              className={`${!isGenerating && status !== "ready" ? "ml-auto" : ""} shrink-0 p-0.5 rounded hover:bg-muted`}
              onClick={(e) => {
                e.stopPropagation();
                onToggleStar();
              }}
            >
              <Star className={`h-3.5 w-3.5 ${note.is_starred ? "fill-yellow-400 text-yellow-400" : "text-muted-foreground/50 hover:text-yellow-400"}`} />
            </button>
            <span className="shrink-0 text-xs text-muted-foreground">{timeAgo}</span>
          </div>
          <div className="mt-1 flex items-center gap-2 overflow-hidden text-xs text-muted-foreground">
            {projectLabel && (
              <span className="shrink-0 truncate text-[10px] text-muted-foreground/70 max-w-[140px]">
                {projectLabel}
              </span>
            )}
            <Badge
              variant="outline"
              className={`shrink-0 text-[10px] ${config.className}`}
            >
              {config.label}
            </Badge>
            {noteData?.priority && (
              <Badge
                variant="outline"
                className="shrink-0 text-[10px]"
              >
                {noteData.priority}
              </Badge>
            )}
            {labels.length > 0 && (
              <div className="flex items-center gap-1 overflow-hidden">
                {labels.slice(0, 2).map((label) => (
                  <span
                    key={label}
                    className="inline-flex shrink-0 items-center rounded-full bg-secondary px-1.5 py-0 text-[10px] font-medium text-secondary-foreground/80"
                  >
                    {label}
                  </span>
                ))}
                {labels.length > 2 && (
                  <span className="text-[10px] text-muted-foreground/50">
                    +{labels.length - 2}
                  </span>
                )}
              </div>
            )}
          </div>
          {status === "draft" && (
            <p className="mt-1 truncate text-xs text-muted-foreground/60">
              {rawContent.slice(0, 200)}
            </p>
          )}
        </button>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onClick={onToggleStar}>
          <Star className="mr-2 h-4 w-4" />
          {note.is_starred ? "Unstar" : "Star"}
        </ContextMenuItem>
        <ContextMenuItem onClick={onEdit}>
          <Pencil className="mr-2 h-4 w-4" />
          Edit
        </ContextMenuItem>
        {status === "draft" && (
          <ContextMenuItem onClick={onGenerate}>
            <Sparkles className="mr-2 h-4 w-4" />
            Generate Issue
          </ContextMenuItem>
        )}
        <ContextMenuItem onClick={onDelete} className="text-destructive focus:text-destructive">
          <Trash2 className="mr-2 h-4 w-4" />
          Delete
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
});
