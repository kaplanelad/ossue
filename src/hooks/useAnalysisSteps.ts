import { useMemo } from "react";
import type { ChatMessage } from "@/types";

export interface AnalysisStepData {
  stepIndex: number;
  label: string;
  displayLabel: string;
  content: string | null;
  status: "pending" | "active" | "streaming" | "complete";
  message?: ChatMessage;
}

const STEP_DISPLAY_NAMES: Record<string, string> = {
  "Summarize this pull request": "Summary",
  "Summarize this issue": "Summary",
  "Summarize this discussion": "Summary",
  "Review the code changes": "Code Review",
  "Draft a suggested review comment": "Suggested Review",
  "Draft a suggested response": "Suggested Response",
  Analyze: "Analysis",
};

const ANALYSIS_STEP_LABELS = new Set(Object.keys(STEP_DISPLAY_NAMES));

const EXPECTED_STEPS: Record<string, string[]> = {
  pr: [
    "Summarize this pull request",
    "Review the code changes",
    "Draft a suggested review comment",
  ],
  issue: ["Summarize this issue", "Draft a suggested response"],
  discussion: ["Summarize this discussion", "Draft a suggested response"],
  note: ["Summarize this issue", "Draft a suggested response"],
};

export function useAnalysisSteps(
  messages: ChatMessage[],
  itemType: "issue" | "pr" | "discussion" | "note",
  streamingContent: string,
  isStreaming: boolean,
  isLoading: boolean,
  currentStepIndex: number,
) {
  return useMemo(() => {
    // Walk messages to find analysis step pairs
    const steps: AnalysisStepData[] = [];
    const followUpMessages: ChatMessage[] = [];

    for (let i = 0; i < messages.length; i++) {
      const msg = messages[i];
      if (msg.role === "user" && ANALYSIS_STEP_LABELS.has(msg.content)) {
        const nextMsg = messages[i + 1];
        const hasResponse = nextMsg?.role === "assistant";
        steps.push({
          stepIndex: steps.length,
          label: msg.content,
          displayLabel: STEP_DISPLAY_NAMES[msg.content] ?? msg.content,
          content: hasResponse ? nextMsg.content : null,
          status: hasResponse ? "complete" : "active",
          message: hasResponse ? nextMsg : undefined,
        });
        if (hasResponse) i++; // skip the assistant message
      } else if (steps.length > 0) {
        // Messages after analysis steps are follow-up chat
        followUpMessages.push(msg);
      } else {
        // Messages before any recognized step (shouldn't happen normally, treat as follow-up)
        followUpMessages.push(msg);
      }
    }

    const hasAnalysis = steps.length > 0;

    // If we have analysis steps, fill in pending steps based on expected steps for item type
    if (hasAnalysis || (isLoading && messages.length === 0)) {
      const expectedLabels = EXPECTED_STEPS[itemType] ?? EXPECTED_STEPS.issue;
      const existingLabels = new Set(steps.map((s) => s.label));

      // During loading with no messages yet, show all expected steps as pending
      if (isLoading && steps.length === 0) {
        for (let i = 0; i < expectedLabels.length; i++) {
          steps.push({
            stepIndex: i,
            label: expectedLabels[i],
            displayLabel: STEP_DISPLAY_NAMES[expectedLabels[i]] ?? expectedLabels[i],
            content: null,
            status: i === 0 ? "active" : "pending",
            message: undefined,
          });
        }
      } else {
        // Add remaining expected steps as pending
        for (const label of expectedLabels) {
          if (!existingLabels.has(label)) {
            steps.push({
              stepIndex: steps.length,
              label,
              displayLabel: STEP_DISPLAY_NAMES[label] ?? label,
              content: null,
              status: "pending",
              message: undefined,
            });
          }
        }
      }
    }

    // Update active/streaming status for the current step
    if (isLoading || isStreaming) {
      for (let i = 0; i < steps.length; i++) {
        if (steps[i].status === "complete") continue;
        if (i === currentStepIndex || steps[i].status === "active") {
          steps[i] = {
            ...steps[i],
            status: isStreaming && streamingContent ? "streaming" : "active",
          };
          // Mark all steps after the active one as pending
          for (let j = i + 1; j < steps.length; j++) {
            if (steps[j].status !== "complete") {
              steps[j] = { ...steps[j], status: "pending" };
            }
          }
          break;
        }
      }
    }

    const isAnalysisComplete =
      hasAnalysis &&
      steps.every((s) => s.status === "complete");

    return {
      analysisSteps: steps,
      followUpMessages,
      isAnalysisComplete,
      hasAnalysis: hasAnalysis || (isLoading && steps.length > 0),
    };
  }, [messages, itemType, streamingContent, isStreaming, isLoading, currentStepIndex]);
}
