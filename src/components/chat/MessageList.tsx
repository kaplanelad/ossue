import { useEffect, useRef } from "react";
import type { ChatMessage, AnalysisAction } from "@/types";
import { AIMessage } from "./AIMessage";
import { UserMessage } from "./UserMessage";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Loader2, Sparkles } from "lucide-react";

interface MessageListProps {
  messages: ChatMessage[];
  streamingContent: string;
  isStreaming: boolean;
  isLoading: boolean;
  analysisStatus: string | null;
  onAnalyzeAction: (action: AnalysisAction) => void;
}

export function MessageList({
  messages,
  streamingContent,
  isStreaming,
  isLoading,
  analysisStatus,
  onAnalyzeAction,
}: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const isAnalyzing = (isLoading && !isStreaming) || (isStreaming && !streamingContent);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "instant" });
  }, [messages, streamingContent]);

  if (messages.length === 0 && !isStreaming && !isLoading) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-4 text-muted-foreground">
        <img src="/app-icon.png" alt="" className="h-10 w-10 opacity-40" />
        <p className="text-sm">Get a full briefing on this item</p>
        <Button variant="outline" size="sm" onClick={() => onAnalyzeAction("analyze")}>
          <Sparkles className="mr-1.5 h-3.5 w-3.5" />
          Analyze
        </Button>
      </div>
    );
  }

  return (
    <ScrollArea className="min-h-0 flex-1">
      <div className="flex min-w-0 flex-col gap-4 p-4">
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
        {isAnalyzing && (
          <div className="flex gap-2.5">
            <div className="flex h-7 w-7 shrink-0 items-center justify-center">
              <Loader2 className="h-4 w-4 animate-spin text-primary" />
            </div>
            <div className="flex items-center text-sm text-muted-foreground">
              {analysisStatus ?? "Analyzing..."}
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
