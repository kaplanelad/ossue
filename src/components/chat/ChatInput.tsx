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
} from "lucide-react";
import type { AnalysisAction } from "@/types";

interface ChatInputProps {
  onSend: (message: string) => void;
  disabled?: boolean;
  onAnalyzeAction: (action: AnalysisAction) => void;
  onClearChat: () => void;
  hasMessages: boolean;
}

export function ChatInput({
  onSend,
  disabled,
  onAnalyzeAction,
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
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="shrink-0 border-t p-4">
      <div className="flex min-w-0 gap-2">
        <Textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask a follow-up question..."
          className="min-h-[40px] min-w-0 max-h-[120px] flex-1 resize-none"
          rows={1}
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
            <DropdownMenuItem onClick={() => onAnalyzeAction("draft_response")}>
              <MessageSquare className="h-4 w-4" />
              Draft Response
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
