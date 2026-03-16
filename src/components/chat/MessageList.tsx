import { useEffect, useRef } from "react";
import type { ChatMessage, ItemType } from "@/types";
import { AIMessage } from "./AIMessage";
import { UserMessage } from "./UserMessage";
import { AnalysisReport } from "./AnalysisReport";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Sparkles } from "lucide-react";
import { useAnalysisSteps } from "@/hooks/useAnalysisSteps";

interface MessageListProps {
  messages: ChatMessage[];
  streamingContent: string;
  isStreaming: boolean;
  isLoading: boolean;
  analysisStatus: string | null;
  currentStepIndex: number;
  onAnalyze: () => void;
  onAnalyzeWithContext: () => void;
  itemId: string;
  itemType: ItemType;
  onSendFollowUp: (message: string) => void;
}

export function MessageList({
  messages,
  streamingContent,
  isStreaming,
  isLoading,
  analysisStatus,
  currentStepIndex,
  onAnalyze,
  onAnalyzeWithContext,
  itemId,
  itemType,
  onSendFollowUp,
}: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  const { analysisSteps, followUpMessages, isAnalysisComplete, hasAnalysis } =
    useAnalysisSteps(
      messages,
      itemType,
      streamingContent,
      isStreaming,
      isLoading,
      currentStepIndex,
    );

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "instant" });
  }, [messages, streamingContent]);

  // Determine if streaming content belongs to a follow-up (not an analysis step)
  const isFollowUpStreaming =
    isStreaming && hasAnalysis && isAnalysisComplete;

  if (messages.length === 0 && !isStreaming && !isLoading) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-4 text-muted-foreground">
        <img src="/app-icon.png" alt="" className="h-10 w-10 opacity-40" />
        <p className="text-sm">Get a full briefing on this item</p>
        <Button variant="outline" size="sm" onClick={onAnalyze}>
          <Sparkles className="mr-1.5 h-3.5 w-3.5" />
          Analyze
        </Button>
        <button
          className="text-xs text-muted-foreground/60 hover:text-muted-foreground transition-colors underline underline-offset-2"
          onClick={onAnalyzeWithContext}
        >
          or add context first
        </button>
      </div>
    );
  }

  return (
    <ScrollArea className="min-h-0 flex-1">
      <div className="flex min-w-0 flex-col gap-4 p-4">
        {hasAnalysis ? (
          <>
            {/* Step-based analysis report */}
            <AnalysisReport
              steps={analysisSteps}
              streamingContent={streamingContent}
              isStreaming={isStreaming}
              analysisStatus={analysisStatus}
              isComplete={isAnalysisComplete}
              itemId={itemId}
              itemType={itemType}
              onSendFollowUp={onSendFollowUp}
              disabled={isLoading || isStreaming}
            />

            {/* Follow-up chat messages */}
            {followUpMessages.map((msg) =>
              msg.role === "assistant" ? (
                <AIMessage key={msg.id} message={msg} />
              ) : (
                <UserMessage key={msg.id} message={msg} />
              )
            )}

            {/* Streaming follow-up response */}
            {isFollowUpStreaming && streamingContent && (
              <AIMessage
                message={{
                  id: "streaming",
                  item_id: "",
                  role: "assistant",
                  content: streamingContent,
                  created_at: new Date().toISOString(),
                  input_tokens: null,
                  output_tokens: null,
                  model: null,
                }}
              />
            )}

            {/* Loading indicator for follow-up */}
            {isLoading && !streamingContent && isAnalysisComplete && (
              <div className="text-sm">
                <span className={`thinking-spinner thinking-spinner-${itemType}`}>Thinking…</span>
              </div>
            )}
          </>
        ) : (
          <>
            {/* Pure chat mode (no analysis steps detected) */}
            {messages.map((msg) =>
              msg.role === "assistant" ? (
                <AIMessage key={msg.id} message={msg} />
              ) : (
                <UserMessage key={msg.id} message={msg} />
              )
            )}
            {isStreaming && streamingContent && (
              <AIMessage
                message={{
                  id: "streaming",
                  item_id: "",
                  role: "assistant",
                  content: streamingContent,
                  created_at: new Date().toISOString(),
                  input_tokens: null,
                  output_tokens: null,
                  model: null,
                }}
              />
            )}
            {isLoading && !isStreaming && (
              <div className="text-sm">
                <span className={`thinking-spinner thinking-spinner-${itemType}`}>{analysisStatus ?? "Thinking…"}</span>
              </div>
            )}
          </>
        )}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
