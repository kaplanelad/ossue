import { useState } from "react";
import type { ChatMessage } from "@/types";
import { Button } from "@/components/ui/button";
import { Check, Copy, Bot } from "lucide-react";
import { Markdown } from "./Markdown";

interface AIMessageProps {
  message: ChatMessage;
}

export function AIMessage({ message }: AIMessageProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(message.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const hasTokens = message.input_tokens != null && message.output_tokens != null;
  const totalTokens = hasTokens ? (message.input_tokens! + message.output_tokens!) : 0;

  return (
    <div className="group flex max-w-[85%] gap-2.5 animate-in fade-in slide-in-from-bottom-2 duration-200">
      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary/10">
        <Bot className="h-6 w-6 text-primary" />
      </div>
      <div className="min-w-0 flex-1 space-y-1.5">
        <div className="overflow-hidden rounded-2xl rounded-tl-md bg-muted/70 px-4 py-3 shadow-sm">
          <div className="text-sm break-words [overflow-wrap:anywhere]">
            <Markdown content={message.content} />
          </div>
        </div>
        <div className="flex items-center gap-2 pl-1">
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
          {(hasTokens || message.model) && (
            <span
              className="text-xs text-muted-foreground"
              title={hasTokens ? `In: ${message.input_tokens} / Out: ${message.output_tokens}` : undefined}
            >
              {message.model && <>{message.model} · </>}
              {hasTokens && <>{totalTokens.toLocaleString()} tokens</>}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
