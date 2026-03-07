import { useAppStore } from "@/stores/appStore";
import { useChat } from "@/hooks/useChat";
import { MessageList } from "@/components/chat/MessageList";
import { ChatInput } from "@/components/chat/ChatInput";
import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { ExternalLink, Trash2, X, Copy, Check, CircleDot, GitPullRequest, Link2 } from "lucide-react";
import { findLinkedItems } from "@/lib/linkedItems";
import type { Item } from "@/types";

interface ChatPanelProps {
  width: number;
}

export function ChatPanel({ width }: ChatPanelProps) {
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
    sendMessage,
    analyzeWithAction,
    clearMessages,
  } = useChat(selectedItemId);

  if (!selectedItem) return null;

  return (
    <div className="flex h-full shrink-0 flex-col overflow-hidden" style={{ width }}>
      {/* Header */}
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold">{selectedItem.title}</h3>
          <p className="truncate text-xs text-muted-foreground">
            {selectedItem.type_data.kind !== "note" ? `#${selectedItem.type_data.external_id}` : ""}{selectedItem.type_data.kind !== "note" && selectedItem.type_data.author ? ` by ${selectedItem.type_data.author}` : ""}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {selectedItem.type_data.kind !== "note" && (
            <>
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
              <CopyUrlButton url={selectedItem.type_data.url} />
            </>
          )}
          {messages.length > 0 && (
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-muted-foreground hover:text-destructive"
              onClick={clearMessages}
              aria-label="Clear chat history"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={() => setSelectedItemId(null)}
            aria-label="Close panel"
          >
            <X className="h-4 w-4" />
          </Button>
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
        onAnalyzeAction={analyzeWithAction}
        itemId={selectedItem.id}
        itemType={selectedItem.item_type}
        onSendFollowUp={sendMessage}
      />

      {/* Input */}
      <ChatInput
        onSend={sendMessage}
        disabled={isLoading || isStreaming}
        onAnalyzeAction={analyzeWithAction}
        onClearChat={clearMessages}
        hasMessages={messages.length > 0}
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
  );
}
