import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  GitMerge,
  CircleX,
  Pencil,
  Send,
  Microscope,
  Loader2,
  Check,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { errorMessage } from "@/lib/utils";
import * as api from "@/lib/tauri";

interface AnalyzeActionsProps {
  itemId: string;
  itemType: "issue" | "pr" | "discussion" | "note";
  lastMessageContent: string;
  onSendFollowUp: (message: string) => void;
  disabled: boolean;
}

export function AnalyzeActions({
  itemId,
  itemType,
  lastMessageContent,
  onSendFollowUp,
  disabled,
}: AnalyzeActionsProps) {
  const [editMode, setEditMode] = useState(false);
  const [editInput, setEditInput] = useState("");
  const [researchMode, setResearchMode] = useState(false);
  const [researchInput, setResearchInput] = useState("");
  const [posting, setPosting] = useState(false);
  const [merging, setMerging] = useState(false);
  const [closing, setClosing] = useState(false);
  const [posted, setPosted] = useState(false);
  const [merged, setMerged] = useState(false);
  const [closed, setClosed] = useState(false);

  const canMerge =
    itemType === "pr" && /CAN MERGE/i.test(lastMessageContent);

  const canClose =
    itemType === "issue" && /Can close/i.test(lastMessageContent);

  // Extract the last ## section that looks like a suggested comment/response
  const extractSuggestedComment = (): string | null => {
    // Match any heading containing "suggest" or "response" or "comment" or "reply"
    const pattern = /^##\s+.*(?:suggest|response|comment|reply).*$/gim;
    let lastMatch: RegExpExecArray | null = null;
    let m: RegExpExecArray | null;
    while ((m = pattern.exec(lastMessageContent)) !== null) {
      lastMatch = m;
    }
    if (!lastMatch || lastMatch.index === undefined) return null;
    const start = lastMatch.index + lastMatch[0].length;
    const rest = lastMessageContent.slice(start);
    // Take everything until the next ## header or end
    const nextHeader = rest.search(/\n##\s/);
    const text = nextHeader !== -1 ? rest.slice(0, nextHeader) : rest;
    return text.trim() || null;
  };

  const handlePost = async () => {
    const comment = extractSuggestedComment();
    if (!comment) {
      toast.error("Could not find a suggested response in the analysis. Use Edit Response to write one.");
      return;
    }
    setPosting(true);
    try {
      await api.postItemComment(itemId, comment);
      setPosted(true);
      toast.success("Comment posted");
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setPosting(false);
    }
  };

  const handleMerge = async () => {
    setMerging(true);
    try {
      await api.mergePullRequest(itemId);
      setMerged(true);
      toast.success("Pull request merged");
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setMerging(false);
    }
  };

  const handleClose = async () => {
    setClosing(true);
    try {
      await api.closeItem(itemId);
      setClosed(true);
      toast.success("Issue closed");
    } catch (err) {
      toast.error(errorMessage(err));
    } finally {
      setClosing(false);
    }
  };

  const handleEditSubmit = () => {
    const trimmed = editInput.trim();
    if (!trimmed) return;
    const section = itemType === "pr" ? "Suggested Review Comment" : "Suggested Response";
    onSendFollowUp(
      `Revise the ${section} based on this feedback: ${trimmed}`
    );
    setEditMode(false);
    setEditInput("");
  };

  const handleResearchSubmit = () => {
    const trimmed = researchInput.trim();
    if (!trimmed) return;
    onSendFollowUp(
      `I need a deeper analysis. ${trimmed}`
    );
    setResearchMode(false);
    setResearchInput("");
  };

  if (editMode) {
    return (
      <div className="flex flex-col gap-2 rounded-xl border border-border/60 bg-muted/30 p-3">
        <p className="text-xs font-medium text-muted-foreground">
          What would you like to change?
        </p>
        <Textarea
          value={editInput}
          onChange={(e) => setEditInput(e.target.value)}
          placeholder="e.g. Make it more concise, ask about test coverage..."
          className="min-h-[60px] max-h-[120px] resize-none text-sm"
          rows={2}
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              handleEditSubmit();
            }
          }}
        />
        <div className="flex gap-2">
          <Button
            size="sm"
            onClick={handleEditSubmit}
            disabled={!editInput.trim() || disabled}
          >
            <Send className="mr-1.5 h-3 w-3" />
            Revise
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setEditMode(false);
              setEditInput("");
            }}
          >
            <X className="mr-1.5 h-3 w-3" />
            Cancel
          </Button>
        </div>
      </div>
    );
  }

  if (researchMode) {
    return (
      <div className="flex flex-col gap-2 rounded-xl border border-border/60 bg-muted/30 p-3">
        <p className="text-xs font-medium text-muted-foreground">
          What concerns do you have? What should be investigated further?
        </p>
        <Textarea
          value={researchInput}
          onChange={(e) => setResearchInput(e.target.value)}
          placeholder="e.g. I'm worried about the database migration safety, check edge cases for..."
          className="min-h-[60px] max-h-[120px] resize-none text-sm"
          rows={2}
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              handleResearchSubmit();
            }
          }}
        />
        <div className="flex gap-2">
          <Button
            size="sm"
            onClick={handleResearchSubmit}
            disabled={!researchInput.trim() || disabled}
          >
            <Microscope className="mr-1.5 h-3 w-3" />
            Research
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setResearchMode(false);
              setResearchInput("");
            }}
          >
            <X className="mr-1.5 h-3 w-3" />
            Cancel
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap gap-2">
      {canMerge && (
        <Button
          size="sm"
          variant="outline"
          onClick={handleMerge}
          disabled={merging || merged || disabled}
          className={merged ? "border-green-500/40 text-green-600" : ""}
        >
          {merging ? (
            <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
          ) : merged ? (
            <Check className="mr-1.5 h-3.5 w-3.5" />
          ) : (
            <GitMerge className="mr-1.5 h-3.5 w-3.5" />
          )}
          {merged ? "Merged" : "Merge PR"}
        </Button>
      )}
      {canClose && (
        <Button
          size="sm"
          variant="outline"
          onClick={handleClose}
          disabled={closing || closed || disabled}
          className={closed ? "border-green-500/40 text-green-600" : ""}
        >
          {closing ? (
            <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
          ) : closed ? (
            <Check className="mr-1.5 h-3.5 w-3.5" />
          ) : (
            <CircleX className="mr-1.5 h-3.5 w-3.5" />
          )}
          {closed ? "Closed" : "Close Issue"}
        </Button>
      )}
      <Button
        size="sm"
        variant="outline"
        onClick={() => setEditMode(true)}
        disabled={disabled}
      >
        <Pencil className="mr-1.5 h-3.5 w-3.5" />
        Edit Response
      </Button>
      <Button
        size="sm"
        onClick={handlePost}
        disabled={posting || posted || disabled}
        className={posted ? "border-green-500/40 bg-green-500/10 text-green-600" : ""}
      >
        {posting ? (
          <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
        ) : posted ? (
          <Check className="mr-1.5 h-3.5 w-3.5" />
        ) : (
          <Send className="mr-1.5 h-3.5 w-3.5" />
        )}
        {posted ? "Posted" : "Post Comment"}
      </Button>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => setResearchMode(true)}
        disabled={disabled}
      >
        <Microscope className="mr-1.5 h-3.5 w-3.5" />
        Deep Research
      </Button>
    </div>
  );
}
