import { useAppStore } from "@/stores/appStore";
import { useChat } from "@/hooks/useChat";
import { MessageList } from "@/components/chat/MessageList";
import { ChatInput } from "@/components/chat/ChatInput";
import { AnalyzeDialog } from "@/components/chat/AnalyzeDialog";
import { useState, useMemo, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { ExternalLink, Trash2, X, Copy, Check, CircleDot, GitPullRequest, Link2, Maximize2, Minimize2 } from "lucide-react";
import { findLinkedItems } from "@/lib/linkedItems";
import type { Item, AnalysisAction } from "@/types";
import {
  HoverCard,
  HoverCardTrigger,
  HoverCardContent,
} from "@/components/ui/hover-card";

interface ChatPanelProps {
  width?: number;
  isFullscreen?: boolean;
  onToggleFullscreen?: () => void;
}

export function ChatPanel({ width, isFullscreen, onToggleFullscreen }: ChatPanelProps) {
  const { selectedItemId, items, setSelectedItemId } = useAppStore();
  const selectedItem = items.find((i) => i.id === selectedItemId);

  const linkedItems = useMemo(
    () => (selectedItem ? findLinkedItems(selectedItem, items) : []),
    [selectedItem, items]
  );

  const handleNavigateToItem = async (item: Item) => {
    setSelectedItemId(item.id);
  };

  const {
    messages,
    streamingContent,
    isStreaming,
    isLoading,
    analysisStatus,
    currentStepIndex,
    sendMessage,
    analyzeWithAction,
    clearMessages,
  } = useChat(selectedItemId);

  const [analyzeDialogOpen, setAnalyzeDialogOpen] = useState(false);
  const [analyzeDialogAction, setAnalyzeDialogAction] = useState<AnalysisAction>("analyze");

  const handleRequestAnalyze = useCallback((action: AnalysisAction) => {
    setAnalyzeDialogAction(action);
    setAnalyzeDialogOpen(true);
  }, []);

  const handleConfirmAnalyze = useCallback((additionalContext?: string) => {
    analyzeWithAction(analyzeDialogAction, additionalContext);
  }, [analyzeDialogAction, analyzeWithAction]);

  if (!selectedItem) return null;

  return (
    <div
      className={`flex h-full flex-col overflow-hidden ${isFullscreen ? "flex-1" : "shrink-0"}`}
      style={isFullscreen ? undefined : { width }}
    >
      {/* Header */}
      <div className="flex h-14 shrink-0 items-center justify-between border-b px-4">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold">{selectedItem.title}</h3>
          <p className="truncate text-xs text-muted-foreground">
            {selectedItem.type_data.kind !== "note" ? `#${selectedItem.type_data.external_id}` : ""}{selectedItem.type_data.kind !== "note" && selectedItem.type_data.author ? ` by ${selectedItem.type_data.author}` : ""}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {selectedItem.type_data.kind !== "note" && (
            <>
              <HoverCard openDelay={300} closeDelay={100}>
                <HoverCardTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-8 w-8"
                    aria-label="Open in browser"
                    asChild
                  >
                    <a href={selectedItem.type_data.url} target="_blank" rel="noopener noreferrer">
                      <ExternalLink className="h-4 w-4" />
                    </a>
                  </Button>
                </HoverCardTrigger>
                <HoverCardContent align="center" side="bottom" className="w-auto p-2">
                  <p className="text-xs text-muted-foreground">Open in browser</p>
                </HoverCardContent>
              </HoverCard>
              <CopyUrlButton url={selectedItem.type_data.url} />
            </>
          )}
          {messages.length > 0 && (
            <HoverCard openDelay={300} closeDelay={100}>
              <HoverCardTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-muted-foreground hover:text-destructive"
                  onClick={clearMessages}
                  aria-label="Clear chat history"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </HoverCardTrigger>
              <HoverCardContent align="center" side="bottom" className="w-auto p-2">
                <p className="text-xs text-muted-foreground">Clear chat history</p>
              </HoverCardContent>
            </HoverCard>
          )}
          {onToggleFullscreen && (
            <HoverCard openDelay={300} closeDelay={100}>
              <HoverCardTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={onToggleFullscreen}
                  aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
                >
                  {isFullscreen ? (
                    <Minimize2 className="h-4 w-4" />
                  ) : (
                    <Maximize2 className="h-4 w-4" />
                  )}
                </Button>
              </HoverCardTrigger>
              <HoverCardContent align="center" side="bottom" className="w-auto p-2">
                <p className="text-xs text-muted-foreground">{isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}</p>
              </HoverCardContent>
            </HoverCard>
          )}
          <HoverCard openDelay={300} closeDelay={100}>
            <HoverCardTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={() => {
                  if (isFullscreen && onToggleFullscreen) onToggleFullscreen();
                  setSelectedItemId(null);
                }}
                aria-label="Close panel"
              >
                <X className="h-4 w-4" />
              </Button>
            </HoverCardTrigger>
            <HoverCardContent align="center" side="bottom" className="w-auto p-2">
              <p className="text-xs text-muted-foreground">Close panel</p>
            </HoverCardContent>
          </HoverCard>
        </div>
      </div>

      {/* Linked items */}
      {linkedItems.length > 0 && (
        <div className="flex items-center gap-1.5 border-b px-4 py-1.5 overflow-x-auto">
          <Link2 className="h-3 w-3 shrink-0 text-muted-foreground" />
          {linkedItems.map((linked) => (
            <button
              key={linked.id}
              className="inline-flex items-center gap-1 rounded-md bg-muted/60 px-2 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors shrink-0"
              onClick={() => handleNavigateToItem(linked)}
              title={linked.title}
            >
              {linked.item_type === "pr" ? (
                <GitPullRequest className="h-3 w-3" />
              ) : (
                <CircleDot className="h-3 w-3" />
              )}
              <span className="max-w-[150px] truncate">#{linked.type_data.kind !== "note" ? linked.type_data.external_id : ""} {linked.title}</span>
            </button>
          ))}
        </div>
      )}

      {/* Messages */}
      <MessageList
        messages={messages}
        streamingContent={streamingContent}
        isStreaming={isStreaming}
        isLoading={isLoading}
        analysisStatus={analysisStatus}
        currentStepIndex={currentStepIndex}
        onAnalyze={() => analyzeWithAction("analyze")}
        onAnalyzeWithContext={() => handleRequestAnalyze("analyze")}
        itemId={selectedItem.id}
        itemType={selectedItem.item_type}
        onSendFollowUp={sendMessage}
      />

      {/* Input */}
      <ChatInput
        onSend={sendMessage}
        disabled={isLoading || isStreaming}
        onAnalyzeAction={(action) => analyzeWithAction(action)}
        onRequestAnalyze={handleRequestAnalyze}
        onClearChat={clearMessages}
        hasMessages={messages.length > 0}
      />

      {/* Analyze confirmation dialog */}
      <AnalyzeDialog
        open={analyzeDialogOpen}
        onOpenChange={setAnalyzeDialogOpen}
        action={analyzeDialogAction}
        onConfirm={handleConfirmAnalyze}
      />
    </div>
  );
}

function CopyUrlButton({ url }: { url: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <HoverCard openDelay={300} closeDelay={100}>
      <HoverCardTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          aria-label="Copy URL"
          onClick={handleCopy}
        >
          {copied ? (
            <Check className="h-4 w-4 text-green-500" />
          ) : (
            <Copy className="h-4 w-4" />
          )}
        </Button>
      </HoverCardTrigger>
      <HoverCardContent align="center" side="bottom" className="w-auto p-2">
        <p className="text-xs text-muted-foreground">{copied ? "Copied!" : "Copy URL"}</p>
      </HoverCardContent>
    </HoverCard>
  );
}
