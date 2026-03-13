import { useState, useRef, useEffect } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Sparkles, MessageSquare } from "lucide-react";
import type { AnalysisAction } from "@/types";

interface AnalyzeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  action: AnalysisAction;
  onConfirm: (additionalContext?: string) => void;
}

const ACTION_CONFIG = {
  analyze: {
    title: "Analyze",
    description: "Run a full analysis on this item.",
    icon: Sparkles,
    buttonLabel: "Run Analysis",
  },
  draft_response: {
    title: "Draft Response",
    description: "Generate a draft response for this item.",
    icon: MessageSquare,
    buttonLabel: "Draft Response",
  },
} as const;

export function AnalyzeDialog({
  open,
  onOpenChange,
  action,
  onConfirm,
}: AnalyzeDialogProps) {
  const [context, setContext] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const config = ACTION_CONFIG[action];
  const Icon = config.icon;

  // Reset textarea when dialog opens
  useEffect(() => {
    if (open) {
      setContext("");
      setTimeout(() => textareaRef.current?.focus(), 100);
    }
  }, [open]);

  const handleConfirm = () => {
    onConfirm(context.trim() || undefined);
    onOpenChange(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleConfirm();
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[480px] gap-0 p-0 overflow-hidden">
        <div className="relative">
          <div className="absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-transparent via-primary/50 to-transparent" />
          <DialogHeader className="px-5 pt-5 pb-3">
            <DialogTitle className="flex items-center gap-2 text-base">
              <span className="flex h-6 w-6 items-center justify-center rounded-md bg-primary/10">
                <Icon className="h-3.5 w-3.5 text-primary" />
              </span>
              {config.title}
            </DialogTitle>
            <DialogDescription className="text-xs">
              {config.description}
            </DialogDescription>
          </DialogHeader>
        </div>

        <div className="flex flex-col gap-3 px-5 pb-5">
          <div className="flex flex-col gap-1">
            <Textarea
              ref={textareaRef}
              placeholder="Add related tickets, requirements, or notes to give the AI more context..."
              value={context}
              onChange={(e) => setContext(e.target.value)}
              onKeyDown={handleKeyDown}
              rows={4}
              className="resize-y min-h-[100px] text-[13.5px] leading-relaxed"
            />
            <p className="text-[11px] text-muted-foreground/60">
              Optional context for the AI. Press Cmd+Enter to run.
            </p>
          </div>

          <div className="flex items-center justify-end gap-2 pt-1 border-t border-border/50">
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-xs"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button
              size="sm"
              className="h-8 text-xs"
              onClick={handleConfirm}
            >
              <Icon className="h-3.5 w-3.5" />
              {config.buttonLabel}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
