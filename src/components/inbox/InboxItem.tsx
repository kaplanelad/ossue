import { useState, forwardRef } from "react";
import type { Item, Platform } from "@/types";
import { formatTimeAgo } from "@/lib/utils";
import { getLabelColor } from "@/lib/labels";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { CircleDot, GitPullRequest, MessageSquare, StickyNote, MailOpen, EyeOff, Inbox, Loader2, Sparkles, ChevronDown, ChevronUp, Star, Trash2 } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

function stripMarkdown(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, "")     // code blocks
    .replace(/`([^`]*)`/g, "$1")         // inline code
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "") // images
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1") // links
    .replace(/^#{1,6}\s+/gm, "")         // headers
    .replace(/(\*\*|__)(.*?)\1/g, "$2")  // bold
    .replace(/(\*|_)(.*?)\1/g, "$2")     // italic
    .replace(/~~(.*?)~~/g, "$1")         // strikethrough
    .replace(/^\s*[-*+]\s+/gm, "")       // unordered lists
    .replace(/^\s*\d+\.\s+/gm, "")       // ordered lists
    .replace(/^\s*>\s+/gm, "")           // blockquotes
    .replace(/---+/g, "")                // horizontal rules
    .replace(/\n+/g, " ")               // newlines to spaces
    .replace(/\s+/g, " ")               // collapse whitespace
    .trim();
}

const typeIcons: Record<string, React.ReactNode> = {
  issue: <CircleDot className="h-4 w-4 text-green-500" />,
  pr: <GitPullRequest className="h-4 w-4 text-blue-500" />,
  discussion: <MessageSquare className="h-4 w-4 text-purple-500" />,
  note: <StickyNote className="h-4 w-4 text-amber-500" />,
};

const stateColors: Record<string, string> = {
  open: "bg-green-500/10 text-green-700 dark:text-green-400",
  closed: "bg-red-500/10 text-red-700 dark:text-red-400",
  merged: "bg-purple-500/10 text-purple-700 dark:text-purple-400",
};

interface InboxItemProps {
  item: Item;
  repoName?: string;
  platform?: Platform;
  isSelected: boolean;
  isAnalyzing?: boolean;
  hasAnalysis?: boolean;
  isChecked: boolean;
  isFocused?: boolean;
  onToggleSelect: (e: React.MouseEvent) => void;
  onClick: () => void;
  onToggleStar: () => void;
  onMarkUnread: () => void;
  onDelete: () => void;
  onRestore?: () => void;
  onClearHistory?: () => void;
  isDismissedView?: boolean;
  linkedItems?: Item[];
  onNavigateToItem?: (id: string) => void;
}

export const InboxItem = forwardRef<HTMLElement, InboxItemProps>(function InboxItem({ item, repoName, platform, isSelected, isAnalyzing, hasAnalysis, isChecked, isFocused, onToggleSelect, onClick, onToggleStar, onMarkUnread, onDelete, onRestore, onClearHistory, isDismissedView, linkedItems, onNavigateToItem }, ref) {
  const [bodyExpanded, setBodyExpanded] = useState(false);
  const timeAgo = formatTimeAgo(item.updated_at);
  const strippedBody = item.body ? stripMarkdown(item.body) : "";

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <button
          ref={ref as React.Ref<HTMLButtonElement>}
          className={`block w-full border-l-2 border-b px-4 py-3.5 text-left transition-colors hover:bg-muted/50 ${
            isSelected ? "bg-muted border-l-primary" : isFocused ? "border-l-primary/30" : "border-l-transparent"
          } ${item.is_read && !isSelected ? "text-foreground/70" : ""}`}
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
            <span className="shrink-0">{typeIcons[item.item_type]}</span>
            <span className={`truncate text-sm ${item.is_read ? "font-normal" : "font-semibold"}`}>
              {item.title}
            </span>
            {isAnalyzing && (
              <Loader2 className="ml-auto h-3.5 w-3.5 shrink-0 animate-spin text-blue-500" />
            )}
            {!isAnalyzing && hasAnalysis && (
              <Sparkles className="ml-auto h-3.5 w-3.5 shrink-0 text-violet-500" />
            )}
            <button
              className={`${isAnalyzing || hasAnalysis ? "" : "ml-auto"} shrink-0 p-0.5 rounded hover:bg-muted`}
              onClick={(e) => {
                e.stopPropagation();
                onToggleStar();
              }}
            >
              <Star className={`h-3.5 w-3.5 ${item.is_starred ? "fill-yellow-400 text-yellow-400" : "text-muted-foreground/50 hover:text-yellow-400"}`} />
            </button>
            <span className="shrink-0 text-xs text-muted-foreground">{timeAgo}</span>
          </div>
          <div className={`mt-1 flex items-center gap-2 overflow-hidden text-xs ${item.is_read ? "text-muted-foreground" : "text-foreground"}`}>
            {item.type_data.kind !== "note" && <span className="shrink-0">#{item.type_data.external_id}</span>}
            {item.type_data.kind !== "note" && item.type_data.author && (
              <span
                className="truncate cursor-pointer hover:underline"
                onClick={(e) => {
                  e.stopPropagation();
                  openUrl(`https://${platform === "gitlab" ? "gitlab.com" : "github.com"}/${item.type_data.kind !== "note" ? item.type_data.author : ""}`);
                }}
              >
                {item.type_data.author}
              </span>
            )}
            {item.type_data.kind !== "note" && (
              <Badge
                variant="outline"
                className={`shrink-0 text-[10px] ${stateColors[item.type_data.state] || ""}`}
              >
                {item.type_data.state}
              </Badge>
            )}
            {item.type_data.kind !== "note" && item.type_data.labels && item.type_data.labels.length > 0 && (
              item.type_data.labels.slice(0, 3).map((label) => {
                const color = getLabelColor(label);
                return (
                  <Badge
                    key={label}
                    variant="outline"
                    className="shrink-0 text-[10px]"
                    style={color ? { color, borderColor: `${color}40` } : undefined}
                  >
                    {label}
                  </Badge>
                );
              })
            )}
            {repoName && (
              <Badge
                variant="outline"
                className="shrink-0 text-[10px] bg-blue-500/10 text-blue-700 dark:text-blue-400"
              >
                {repoName}
              </Badge>
            )}
            {linkedItems && linkedItems.length > 0 && linkedItems.map((linked) => (
              <button
                key={linked.id}
                className="inline-flex shrink-0 items-center gap-0.5 rounded bg-blue-500/10 px-1.5 py-0 text-[10px] font-medium text-blue-600 dark:text-blue-400 hover:bg-blue-500/20 transition-colors"
                onClick={(e) => { e.stopPropagation(); onNavigateToItem?.(linked.id); }}
                title={linked.title}
              >
                {linked.item_type === "pr" ? <GitPullRequest className="h-2.5 w-2.5" /> : <CircleDot className="h-2.5 w-2.5" />}
                #{linked.type_data.kind !== "note" && linked.type_data.external_id}
              </button>
            ))}
            {item.type_data.kind !== "note" && item.type_data.comments_count > 0 && (
              <span className="ml-auto flex shrink-0 items-center gap-1">
                <MessageSquare className="h-3 w-3" />
                {item.type_data.comments_count}
              </span>
            )}
          </div>
          {strippedBody && (
            <div
              className="mt-1 flex items-start gap-1 text-xs text-muted-foreground/70"
              onClick={(e) => {
                e.stopPropagation();
                setBodyExpanded(!bodyExpanded);
              }}
            >
              <p className={bodyExpanded ? "whitespace-pre-wrap break-words line-clamp-6" : "truncate"}>
                {strippedBody}
              </p>
              {bodyExpanded ? (
                <ChevronUp className="h-3 w-3 shrink-0 mt-0.5" />
              ) : (
                <ChevronDown className="h-3 w-3 shrink-0 mt-0.5" />
              )}
            </div>
          )}
        </button>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onClick={onToggleStar}>
          <Star className="mr-2 h-4 w-4" />
          {item.is_starred ? "Unstar" : "Star"}
        </ContextMenuItem>
        <ContextMenuItem onClick={onMarkUnread}>
          <MailOpen className="mr-2 h-4 w-4" />
          Mark as Unread
        </ContextMenuItem>
        {hasAnalysis && onClearHistory && (
          <ContextMenuItem onClick={onClearHistory}>
            <Trash2 className="mr-2 h-4 w-4" />
            Clear AI History
          </ContextMenuItem>
        )}
        {isDismissedView && onRestore ? (
          <ContextMenuItem onClick={onRestore}>
            <Inbox className="mr-2 h-4 w-4" />
            Move to Inbox
          </ContextMenuItem>
        ) : (
          <ContextMenuItem onClick={onDelete}>
            <EyeOff className="mr-2 h-4 w-4" />
            Dismiss
          </ContextMenuItem>
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
});

