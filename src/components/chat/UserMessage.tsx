import { useState } from "react";
import type { ChatMessage } from "@/types";
import { Button } from "@/components/ui/button";
import { Check, Copy, User } from "lucide-react";
import { Markdown } from "./Markdown";

interface UserMessageProps {
  message: ChatMessage;
}

export function UserMessage({ message }: UserMessageProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(message.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col items-end gap-1.5 animate-in fade-in slide-in-from-bottom-2 duration-200">
      <div className="flex w-full min-w-0 justify-end gap-2.5">
        <div className="min-w-0 max-w-[85%] overflow-hidden rounded-2xl rounded-br-md bg-muted/40 px-4 py-3 shadow-sm">
          <div className="text-sm break-words [overflow-wrap:anywhere]">
            <Markdown content={message.content} />
          </div>
        </div>
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-secondary">
          <User className="h-4 w-4" />
        </div>
      </div>
      <div className="flex items-center gap-2 pr-10">
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
    </div>
  );
}
