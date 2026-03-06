import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { X, Copy, Check, ChevronDown, ChevronUp } from "lucide-react";
import type { UpdateInfo } from "@/types";

const INSTALL_COMMAND =
  "curl -fsSL https://raw.githubusercontent.com/kaplanelad/ossue/main/install.sh | bash";

interface UpdateBannerProps {
  updateInfo: UpdateInfo;
  onDismiss: () => void;
}

export function UpdateBanner({ updateInfo, onDismiss }: UpdateBannerProps) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(INSTALL_COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="border-b bg-primary/10">
      <div className="flex items-center justify-between gap-3 px-4 py-2 text-sm">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 text-left hover:underline"
        >
          Update available: <strong>v{updateInfo.latest_version}</strong>
          {expanded ? (
            <ChevronUp className="h-3.5 w-3.5" />
          ) : (
            <ChevronDown className="h-3.5 w-3.5" />
          )}
        </button>
        <div className="flex items-center gap-2">
          <button
            onClick={() => openUrl(updateInfo.release_url)}
            className="rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
          >
            Release Notes
          </button>
          <button
            onClick={onDismiss}
            className="rounded-md p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
            aria-label="Dismiss"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      {expanded && (
        <div className="px-4 pb-3">
          <p className="mb-1.5 text-xs text-muted-foreground">
            Run in your terminal to update:
          </p>
          <div className="flex items-center gap-2 rounded-md bg-muted px-3 py-2 font-mono text-xs">
            <code className="flex-1 select-all truncate">{INSTALL_COMMAND}</code>
            <button
              onClick={handleCopy}
              className="shrink-0 rounded p-1 text-muted-foreground hover:bg-background hover:text-foreground"
              aria-label="Copy command"
            >
              {copied ? (
                <Check className="h-3.5 w-3.5 text-green-500" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
