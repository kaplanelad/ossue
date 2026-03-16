import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Check, Copy, Loader2, FileText, Search, MessageSquare, Sparkles } from "lucide-react";
import { Markdown } from "./Markdown";
import type { ItemType } from "@/types";
import type { AnalysisStepData } from "@/hooks/useAnalysisSteps";

const STEP_ICONS: Record<string, React.ElementType> = {
  Summary: FileText,
  "Code Review": Search,
  "Suggested Review": MessageSquare,
  "Suggested Response": MessageSquare,
  Analysis: Sparkles,
};

interface AnalysisStepProps {
  step: AnalysisStepData;
  isLast: boolean;
  streamingContent?: string;
  analysisStatus?: string | null;
  itemType: ItemType;
}

export function AnalysisStep({
  step,
  isLast,
  streamingContent,
  analysisStatus,
  itemType,
}: AnalysisStepProps) {
  const [copied, setCopied] = useState(false);
  const isActive = step.status === "active" || step.status === "streaming";
  const isComplete = step.status === "complete";
  const isPending = step.status === "pending";
  const Icon = STEP_ICONS[step.displayLabel] ?? Sparkles;

  const displayContent =
    step.status === "streaming" && streamingContent
      ? streamingContent
      : step.content;

  const handleCopy = async () => {
    if (!displayContent) return;
    await navigator.clipboard.writeText(displayContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative flex gap-3">
      {/* Vertical connector line */}
      {!isLast && (
        <div
          className="absolute left-[15px] top-[30px] bottom-0 w-px bg-border"
          aria-hidden
        />
      )}

      {/* Step indicator */}
      <div className="relative z-10 flex h-[30px] w-[30px] shrink-0 items-center justify-center">
        {isActive ? (
          <div className="flex h-[30px] w-[30px] items-center justify-center rounded-full bg-primary/10">
            <Loader2 className="h-4 w-4 animate-spin text-primary" />
          </div>
        ) : isComplete ? (
          <div className="flex h-[30px] w-[30px] items-center justify-center rounded-full bg-primary/10">
            <Check className="h-4 w-4 text-primary" />
          </div>
        ) : (
          <div className="flex h-[30px] w-[30px] items-center justify-center rounded-full bg-muted">
            <span className="text-xs font-medium text-muted-foreground">
              {step.stepIndex + 1}
            </span>
          </div>
        )}
      </div>

      {/* Step content */}
      <div className="min-w-0 flex-1 pb-6">
        {/* Header */}
        <div className="flex items-center gap-2 h-[30px]">
          <Icon
            className={`h-3.5 w-3.5 ${isPending ? "text-muted-foreground/50" : "text-muted-foreground"}`}
          />
          <span
            className={`text-sm font-medium ${isPending ? "text-muted-foreground/50" : ""}`}
          >
            {step.displayLabel}
          </span>
          {isActive && analysisStatus && (
            <span className={`text-xs thinking-spinner thinking-spinner-${itemType} animate-in fade-in duration-300`}>
              {analysisStatus}
            </span>
          )}
        </div>

        {/* Content card */}
        {displayContent && (
          <div
            className={`mt-2 overflow-hidden rounded-lg border bg-card p-4 shadow-sm animate-in fade-in slide-in-from-top-1 duration-300 ${
              step.status === "streaming" ? "step-pulse" : ""
            }`}
          >
            <div className="text-sm break-words [overflow-wrap:anywhere]">
              <Markdown content={displayContent} />
            </div>
            {isComplete && (
              <div className="mt-3 flex items-center justify-between border-t pt-2">
                <div className="flex items-center gap-2">
                  {step.message?.model && (
                    <span className="text-xs text-muted-foreground">
                      {step.message.model}
                    </span>
                  )}
                  {step.message?.input_tokens != null &&
                    step.message?.output_tokens != null && (
                      <span className="text-xs text-muted-foreground">
                        {(
                          step.message.input_tokens + step.message.output_tokens
                        ).toLocaleString()}{" "}
                        tokens
                      </span>
                    )}
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 gap-1 text-xs"
                  onClick={handleCopy}
                >
                  {copied ? (
                    <>
                      <Check className="h-3 w-3" />
                      Copied
                    </>
                  ) : (
                    <>
                      <Copy className="h-3 w-3" />
                      Copy
                    </>
                  )}
                </Button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
