import { useState, useRef, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import {
  Send,
  Ellipsis,
  Sparkles,
  MessageSquare,
  Trash2,
  TextCursorInput,
} from "lucide-react";
import type { AnalysisAction } from "@/types";

interface ChatInputProps {
  onSend: (message: string) => void;
  disabled?: boolean;
  onAnalyzeAction: (action: AnalysisAction) => void;
  onRequestAnalyze: (action: AnalysisAction) => void;
  onClearChat: () => void;
  hasMessages: boolean;
}

export function ChatInput({
  onSend,
  disabled,
  onAnalyzeAction,
  onRequestAnalyze,
  onClearChat,
  hasMessages,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSend = useCallback(() => {
    const trimmed = input.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setInput("");
  }, [input, disabled, onSend]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="shrink-0 border-t">
      <div className="flex min-w-0 gap-2 p-4">
        <Textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask a follow-up question..."
          className="min-w-0 flex-1 resize-none"
          disabled={disabled}
        />
        <Button
          size="icon"
          onClick={handleSend}
          disabled={!input.trim() || disabled}
          aria-label="Send message"
        >
          <Send className="h-4 w-4" />
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="icon" aria-label="More actions">
              <Ellipsis className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent side="top" align="end">
            <DropdownMenuItem onClick={() => onAnalyzeAction("analyze")}>
              <Sparkles className="h-4 w-4" />
              Analyze
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => onRequestAnalyze("analyze")}>
              <TextCursorInput className="h-4 w-4" />
              Analyze with context...
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => onAnalyzeAction("draft_response")}>
              <MessageSquare className="h-4 w-4" />
              Draft Response
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => onRequestAnalyze("draft_response")}>
              <TextCursorInput className="h-4 w-4" />
              Draft Response with context...
            </DropdownMenuItem>
            {hasMessages && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuItem variant="destructive" onClick={onClearChat}>
                  <Trash2 className="h-4 w-4" />
                  Clear Chat
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
