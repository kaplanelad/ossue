import { Sparkles } from "lucide-react";
import { AnalysisStep } from "./AnalysisStep";
import { AnalyzeActions } from "./AnalyzeActions";
import type { ItemType } from "@/types";
import type { AnalysisStepData } from "@/hooks/useAnalysisSteps";

interface AnalysisReportProps {
  steps: AnalysisStepData[];
  streamingContent: string;
  isStreaming: boolean;
  analysisStatus: string | null;
  isComplete: boolean;
  itemId: string;
  itemType: ItemType;
  onSendFollowUp: (message: string) => void;
  disabled: boolean;
}

export function AnalysisReport({
  steps,
  streamingContent,
  isStreaming,
  analysisStatus,
  isComplete,
  itemId,
  itemType,
  onSendFollowUp,
  disabled,
}: AnalysisReportProps) {
  const lastCompleteStep = [...steps]
    .reverse()
    .find((s) => s.status === "complete");

  return (
    <div className="space-y-1">
      {/* Header */}
      <div className="flex items-center gap-2 pb-2">
        <Sparkles className="h-4 w-4 text-primary" />
        <span className="text-sm font-semibold">Analysis</span>
      </div>

      {/* Steps */}
      <div>
        {steps.map((step, i) => (
          <AnalysisStep
            key={step.label}
            step={step}
            isLast={i === steps.length - 1}
            streamingContent={
              (step.status === "streaming" || step.status === "active") &&
              isStreaming
                ? streamingContent
                : undefined
            }
            analysisStatus={
              step.status === "active" || step.status === "streaming"
                ? analysisStatus
                : null
            }
            itemType={itemType}
          />
        ))}
      </div>

      {/* Action buttons */}
      {isComplete && lastCompleteStep?.content && (
        <div className="pt-2 pl-[42px]">
          <AnalyzeActions
            itemId={itemId}
            itemType={itemType}
            lastMessageContent={lastCompleteStep.content}
            onSendFollowUp={onSendFollowUp}
            disabled={disabled}
          />
        </div>
      )}
    </div>
  );
}
