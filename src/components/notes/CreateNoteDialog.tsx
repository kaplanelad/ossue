import { useState, useRef, useEffect } from "react";
import { errorMessage } from "@/lib/utils";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "sonner";
import { Checkbox } from "@/components/ui/checkbox";
import { Loader2, StickyNote, Sparkles, ArrowLeftRight } from "lucide-react";
import { useProjects } from "@/hooks/useProjects";
import { useAppStore } from "@/stores/appStore";
import { useDraftIssueStore } from "@/stores/draftIssueStore";
import * as api from "@/lib/tauri";

export function CreateNoteDialog() {
  const { projects } = useProjects();
  const {
    isCreateNoteOpen,
    editingNote,
    defaultProjectId,
    lastUsedProjectId,
    setLastUsedProjectId,
    closeCreateNote,
    setSelectedNoteId,
    setIsGenerating,
  } = useDraftIssueStore();
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [content, setContent] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [keepOpen, setKeepOpen] = useState(false);
  const [mode, setMode] = useState<"note" | "issue">("issue");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const isEditing = !!editingNote;

  // Reset state every time the dialog opens
  useEffect(() => {
    if (isCreateNoteOpen) {
      if (editingNote) {
        setContent(editingNote.type_data.kind === "note" ? editingNote.type_data.raw_content : "");
        setSelectedProjectId(editingNote.project_id);
      } else {
        setContent("");
        setSelectedProjectId(defaultProjectId ?? lastUsedProjectId ?? "");
        setMode("issue");
      }
      setIsSaving(false);
      setIsAnalyzing(false);
      setTimeout(() => textareaRef.current?.focus(), 100);
    }
  }, [isCreateNoteOpen, editingNote]);

  const handleSave = async () => {
    if (!selectedProjectId || !content.trim()) return;

    setLastUsedProjectId(selectedProjectId);
    setIsSaving(true);
    try {
      if (isEditing) {
        const updates: Record<string, string> = {};
        const editRawContent = editingNote.type_data.kind === "note" ? editingNote.type_data.raw_content : "";
        if (content.trim() !== editRawContent) {
          updates.rawContent = content.trim();
        }
        if (selectedProjectId !== editingNote.project_id) {
          updates.projectId = selectedProjectId;
        }
        if (Object.keys(updates).length > 0) {
          await api.updateDraftIssue(editingNote.id, updates);
        }
        await useAppStore.getState().refreshInbox();
        closeCreateNote();

      } else {
        await api.createDraftIssue(selectedProjectId, content);
        await useAppStore.getState().refreshInbox();
        if (keepOpen) {
          setContent("");
          setTimeout(() => textareaRef.current?.focus(), 50);
        } else {
          closeCreateNote();
        }
      }
    } catch (err) {
      toast.error(`Failed to ${isEditing ? "update" : "save"} note`, {
        description: errorMessage(err),
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleSaveAndAnalyze = async () => {
    if (!selectedProjectId || !content.trim()) return;

    setLastUsedProjectId(selectedProjectId);
    setIsAnalyzing(true);
    try {
      const created = await api.createDraftIssue(selectedProjectId, content);
      await useAppStore.getState().refreshInbox();
      closeCreateNote();
      setSelectedNoteId(created.id);

      setIsGenerating(created.id, true);
      try {
        await api.generateIssueFromDraft(created.id);
        await useAppStore.getState().refreshInbox();
      } finally {
        setIsGenerating(created.id, false);
      }
    } catch (err) {
      toast.error("Failed to save and analyze note", {
        description: errorMessage(err),
      });
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Tab" && e.shiftKey && !isEditing) {
      e.preventDefault();
      setMode((m) => (m === "note" ? "issue" : "note"));
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (isEditing) {
        handleSave();
      } else if (mode === "issue") {
        handleSaveAndAnalyze();
      } else {
        handleSave();
      }
    }
  };

  return (
    <Dialog
      open={isCreateNoteOpen}
      onOpenChange={(open) => !open && closeCreateNote()}
    >
      <DialogContent className="sm:max-w-[480px] gap-0 p-0 overflow-hidden">
        <div className="relative">
          <div className={`absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-transparent ${mode === "issue" && !isEditing ? "via-primary/50" : "via-amber-500/50 dark:via-amber-400/50"} to-transparent`} />
          <DialogHeader className="px-5 pt-5 pb-3">
            <DialogTitle className="flex items-center gap-2 text-base">
              <span className={`flex h-6 w-6 items-center justify-center rounded-md ${mode === "issue" && !isEditing ? "bg-primary/10" : "bg-amber-500/10 dark:bg-amber-400/10"}`}>
                {mode === "issue" && !isEditing ? (
                  <Sparkles className="h-3.5 w-3.5 text-primary" />
                ) : (
                  <StickyNote className="h-3.5 w-3.5 text-amber-500 dark:text-amber-400" />
                )}
              </span>
              {isEditing ? "Edit Note" : mode === "issue" ? "New Issue" : "New Note"}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {isEditing
                ? "Update your note content and project."
                : mode === "issue"
                  ? "Describe your issue. It will be saved and analyzed by AI."
                  : "Capture your thoughts. You can generate a structured issue from it later."}
            </DialogDescription>
          </DialogHeader>
        </div>

        <div className="flex flex-col gap-3 px-5 pb-5">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground shrink-0">
              Project
            </span>
            <Select
              value={selectedProjectId}
              onValueChange={setSelectedProjectId}
            >
              <SelectTrigger className="h-8 text-sm flex-1">
                <SelectValue placeholder="Choose a repository..." />
              </SelectTrigger>
              <SelectContent>
                {projects.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    <span className="text-muted-foreground">{p.owner}/</span>
                    <span className="font-medium">{p.name}</span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex flex-col gap-1">
            <Textarea
              ref={textareaRef}
              placeholder="What's on your mind? Describe a bug, feature idea, or task..."
              value={content}
              onChange={(e) => setContent(e.target.value)}
              onKeyDown={handleKeyDown}
              rows={6}
              className="draft-textarea resize-y min-h-[140px] text-[13.5px] leading-relaxed"
            />
            <p className="text-[11px] text-muted-foreground/60">
              Enter to {isEditing ? "save" : mode === "issue" ? "save & analyze" : "save as draft"} · Shift+Enter for new line{!isEditing && " · Shift+Tab to switch mode"}
            </p>
          </div>

          <div className="flex items-center justify-between pt-1 border-t border-border/50">
            {isEditing ? (
              <div />
            ) : (
              <label className="flex items-center gap-1.5 cursor-pointer select-none">
                <Checkbox
                  checked={keepOpen}
                  onCheckedChange={(v) => setKeepOpen(v === true)}
                  className="h-3.5 w-3.5"
                />
                <span className="text-[11px] text-muted-foreground">
                  Create another
                </span>
              </label>
            )}
            <div className="flex items-center gap-2">
              {!isEditing && (
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 text-xs gap-1"
                  onClick={() => setMode((m) => (m === "note" ? "issue" : "note"))}
                  disabled={isSaving || isAnalyzing}
                >
                  <ArrowLeftRight className="h-3 w-3" />
                  {mode === "issue" ? "Switch to Note" : "Switch to Issue"}
                </Button>
              )}
              <Button
                size="sm"
                className="h-8 text-xs"
                onClick={isEditing ? handleSave : mode === "issue" ? handleSaveAndAnalyze : handleSave}
                disabled={isSaving || isAnalyzing || !selectedProjectId || !content.trim()}
              >
                {isEditing ? (
                  isSaving ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <StickyNote className="h-3.5 w-3.5" />
                  )
                ) : mode === "issue" ? (
                  isAnalyzing ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Sparkles className="h-3.5 w-3.5" />
                  )
                ) : (
                  isSaving ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <StickyNote className="h-3.5 w-3.5" />
                  )
                )}
                {isEditing ? "Update Note" : mode === "issue" ? "Save & Analyze" : "Save as Draft"}
              </Button>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
