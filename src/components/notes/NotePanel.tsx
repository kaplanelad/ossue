import { useState, useRef, useCallback } from "react";
import { errorMessage } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useDraftIssueStore } from "@/stores/draftIssueStore";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Markdown } from "@/components/chat/Markdown";
import { toast } from "sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  X,
  Sparkles,
  Loader2,
  Send,
  RefreshCw,
  StickyNote,
  ChevronDown,
  Pencil,
} from "lucide-react";
import * as api from "@/lib/tauri";
import { LABEL_OPTIONS } from "@/lib/labels";
import type { Item } from "@/types";


interface NotePanelProps {
  width: number;
}

export function NotePanel({ width }: NotePanelProps) {
  const {
    selectedNoteId,
    setSelectedNoteId,
    isGenerating,
    setIsGenerating,
    isSubmitting,
    setIsSubmitting,
  } = useDraftIssueStore();

  const items = useAppStore((s) => s.items);
  const refreshInbox = useAppStore((s) => s.refreshInbox);

  const note = items.find((i) => i.id === selectedNoteId && i.item_type === "note");

  if (!note) return null;

  const generating = isGenerating[note.id] || false;
  const submitting = isSubmitting[note.id] || false;
  const isBusy = generating || submitting;
  const noteData = note.type_data.kind === "note" ? note.type_data : null;
  const status = noteData?.draft_status || "draft";

  const handleGenerate = async () => {
    setIsGenerating(note.id, true);
    try {
      await api.generateIssueFromDraft(note.id);
      await refreshInbox();
    } catch (err) {
      toast.error("Generation failed", { description: errorMessage(err) });
    } finally {
      setIsGenerating(note.id, false);
    }
  };

  const handleRegenerate = async () => {
    setIsGenerating(note.id, true);
    try {
      await api.generateIssueFromDraft(note.id);
      await refreshInbox();
    } catch (err) {
      toast.error("Regeneration failed", { description: errorMessage(err) });
    } finally {
      setIsGenerating(note.id, false);
    }
  };

  const handleSubmit = async () => {
    setIsSubmitting(note.id, true);
    try {
      const result = await api.submitDraftToProvider(note.id);
      await refreshInbox();
      setSelectedNoteId(null);
      toast.success(`Issue #${result.number} created`, {
        description: result.url,
        action: {
          label: "Open",
          onClick: () => openUrl(result.url),
        },
      });
    } catch (err) {
      toast.error("Failed to create issue", { description: errorMessage(err) });
    } finally {
      setIsSubmitting(note.id, false);
    }
  };

  return (
    <div className="flex h-full shrink-0 flex-col overflow-hidden" style={{ width }}>
      {/* Header */}
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div className="min-w-0 flex-1 flex items-center gap-2">
          <StickyNote className="h-4 w-4 shrink-0 text-amber-500 dark:text-amber-400" />
          <h3 className="truncate text-sm font-semibold">
            {note.title || "Note"}
          </h3>
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 shrink-0"
          onClick={() => setSelectedNoteId(null)}
          aria-label="Close notes"
        >
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {generating ? (
          <ProcessingView />
        ) : status === "draft" ? (
          <DraftView note={note} />
        ) : (
          <ReadyView note={note} />
        )}
      </div>

      {/* Actions footer */}
      {status === "draft" && (
        <DraftActions
          onGenerate={handleGenerate}
          generating={generating}
          isBusy={isBusy}
        />
      )}
      {status === "ready" && (
        <ReadyActions
          note={note}
          onRegenerate={handleRegenerate}
          onSubmit={handleSubmit}
          generating={generating}
          submitting={submitting}
          isBusy={isBusy}
        />
      )}
    </div>
  );
}

function DraftView({ note }: { note: Item }) {
  const { openEditNote } = useDraftIssueStore();
  const rawContent = note.type_data.kind === "note" ? note.type_data.raw_content : "";

  return (
    <div className="p-4">
      <div className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
            Original Note
          </p>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-xs text-muted-foreground"
            onClick={() => openEditNote(note)}
          >
            <Pencil className="h-3 w-3" />
            Edit
          </Button>
        </div>
        <div className="rounded-lg bg-muted/50 border border-border/40 p-4">
          <p className="text-sm leading-relaxed whitespace-pre-wrap">{rawContent}</p>
        </div>
      </div>
      <div className="flex flex-col items-center gap-2 py-8 text-center">
        <div className="flex h-10 w-10 items-center justify-center rounded-full bg-amber-500/10 dark:bg-amber-400/10">
          <Sparkles className="h-5 w-5 text-amber-500 dark:text-amber-400" />
        </div>
        <p className="text-sm font-medium text-muted-foreground">Ready to generate?</p>
        <p className="text-xs text-muted-foreground/60 max-w-[240px]">
          AI will structure this note into a proper issue with title, description, labels, and priority.
        </p>
      </div>
    </div>
  );
}

function ProcessingView() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
      <Loader2 className="h-8 w-8 animate-spin text-blue-500" />
      <p className="text-sm text-muted-foreground">Generating issue...</p>
      <p className="text-xs text-muted-foreground/60">AI is structuring your note</p>
    </div>
  );
}

function ReadyView({ note }: { note: Item }) {
  const [showOriginal, setShowOriginal] = useState(false);
  const noteData = note.type_data.kind === "note" ? note.type_data : null;
  const labels = noteData?.labels ?? [];
  const rawContent = noteData?.raw_content || "";

  return (
    <div className="p-4 flex flex-col gap-4">
      {/* Generated title */}
      {note.title && (
        <div>
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground mb-1.5">
            Title
          </p>
          <p className="text-sm font-semibold">{note.title}</p>
        </div>
      )}

      {/* Generated body */}
      {note.body && (
        <div>
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground mb-1.5">
            Description
          </p>
          <div className="rounded-lg bg-muted/30 border border-border/40 p-3 text-xs leading-relaxed">
            <Markdown content={note.body} />
          </div>
        </div>
      )}

      {/* Labels */}
      {labels.length > 0 && (
        <div>
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground mb-1.5">
            Labels
          </p>
          <div className="flex flex-wrap gap-1.5">
            {labels.map((label) => {
              const opt = LABEL_OPTIONS.find((l) => l.value === label);
              return (
                <span
                  key={label}
                  className="rounded-full border px-2 py-0.5 text-[11px] font-medium"
                  style={opt ? { color: opt.color, borderColor: `${opt.color}40` } : undefined}
                >
                  {label}
                </span>
              );
            })}
          </div>
        </div>
      )}

      {/* Priority & Area */}
      <div className="grid grid-cols-2 gap-4">
        {noteData?.priority && (
          <div>
            <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground mb-1">
              Priority
            </p>
            <p className={`text-sm font-medium draft-priority-${noteData.priority}`}>
              {noteData.priority.charAt(0).toUpperCase() + noteData.priority.slice(1)}
            </p>
          </div>
        )}
        {noteData?.area && (
          <div>
            <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground mb-1">
              Area
            </p>
            <p className="text-sm font-medium">
              {noteData.area.charAt(0).toUpperCase() + noteData.area.slice(1)}
            </p>
          </div>
        )}
      </div>

      {/* Empty state fallback */}
      {!note.title && !note.body && labels.length === 0 && !noteData?.priority && !noteData?.area && (
        <div className="flex flex-col items-center gap-2 py-8 text-center text-muted-foreground">
          <p className="text-sm">No structured content generated</p>
          <p className="text-xs">Try regenerating with more detailed notes.</p>
        </div>
      )}

      {/* Original notes collapsible */}
      <div>
        <button
          onClick={() => setShowOriginal(!showOriginal)}
          className="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60 hover:text-muted-foreground transition-colors"
        >
          <ChevronDown
            className={`h-3 w-3 transition-transform ${showOriginal ? "rotate-180" : ""}`}
          />
          Original note
        </button>
        {showOriginal && (
          <pre className="mt-2 rounded-lg bg-muted/50 border border-border/40 p-3 text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap">
            {rawContent}
          </pre>
        )}
      </div>
    </div>
  );
}

function DraftActions({
  onGenerate,
  generating,
  isBusy,
}: {
  onGenerate: () => void;
  generating: boolean;
  isBusy: boolean;
}) {
  return (
    <div className="shrink-0 border-t bg-card/80 backdrop-blur-sm px-4 py-3">
      <div className="flex items-center justify-end">
        <Button
          size="sm"
          className="text-xs"
          onClick={onGenerate}
          disabled={isBusy}
        >
          {generating ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Sparkles className="h-3.5 w-3.5" />
          )}
          Generate Issue
        </Button>
      </div>
    </div>
  );
}

function ReadyActions({
  note,
  onRegenerate,
  onSubmit,
  generating,
  submitting,
  isBusy,
}: {
  note: Item;
  onRegenerate: () => void;
  onSubmit: () => void;
  generating: boolean;
  submitting: boolean;
  isBusy: boolean;
}) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const refreshInbox = useAppStore((s) => s.refreshInbox);
  const rawContent = note.type_data.kind === "note" ? note.type_data.raw_content : "";

  const handleSend = useCallback(async () => {
    const trimmed = input.trim();
    if (!trimmed || isBusy) return;

    const updatedContent = `${rawContent}\n\n---\nFollow-up: ${trimmed}`;
    try {
      await api.updateDraftIssue(note.id, { rawContent: updatedContent });
      await refreshInbox();
      setInput("");
      onRegenerate();
    } catch (err) {
      toast.error("Failed to update", { description: errorMessage(err) });
    }
  }, [input, isBusy, note.id, rawContent, refreshInbox, onRegenerate]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="shrink-0 border-t bg-card/80 backdrop-blur-sm px-4 py-3 flex flex-col gap-2">
      <div className="flex min-w-0 gap-2">
        <Textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask for changes..."
          className="min-h-[40px] min-w-0 max-h-[120px] flex-1 resize-none"
          rows={1}
          disabled={isBusy}
        />
        <Button
          size="icon"
          variant="ghost"
          onClick={handleSend}
          disabled={!input.trim() || isBusy}
        >
          <Send className="h-4 w-4" />
        </Button>
      </div>
      <div className="flex items-center justify-between">
        <Button
          variant="ghost"
          size="sm"
          className="text-xs"
          onClick={onRegenerate}
          disabled={isBusy}
        >
          {generating ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <RefreshCw className="h-3.5 w-3.5" />
          )}
          Re-generate
        </Button>
        <Button
          size="sm"
          className="text-xs"
          onClick={onSubmit}
          disabled={isBusy || !note.title?.trim()}
        >
          {submitting ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Send className="h-3.5 w-3.5" />
          )}
          Publish
        </Button>
      </div>
    </div>
  );
}
