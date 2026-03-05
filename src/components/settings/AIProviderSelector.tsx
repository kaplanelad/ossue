import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Eye, EyeOff, Zap, Terminal, Monitor } from "lucide-react";
import { Button } from "@/components/ui/button";

type AiMode = "api" | "claude_cli" | "cursor_cli";

interface Props {
  mode: AiMode;
  hasApiKey: boolean;
  model: string;
  customInstructions: string | null;
  onModeChange: (mode: AiMode) => void;
  onApiKeyChange: (key: string) => void;
  onModelChange: (model: string) => void;
  onCustomInstructionsChange: (instructions: string) => void;
}

const PROVIDER_CARDS: {
  mode: AiMode;
  title: string;
  description: string;
  speed: string;
  reviewQuality: string;
  codebaseAccess: string;
  bestFor: string;
  requires: string;
  icon: typeof Zap;
}[] = [
  {
    mode: "api",
    title: "API (Anthropic/OpenAI)",
    description: "Fast cloud-based analysis",
    speed: "2-5 seconds",
    reviewQuality: "Good for simple PRs",
    codebaseAccess: "PR diff + comments only",
    bestFor: "Quick triage & summaries",
    requires: "API key",
    icon: Zap,
  },
  {
    mode: "claude_cli",
    title: "Claude Code CLI",
    description: "Deep analysis with full codebase",
    speed: "30s-2min",
    reviewQuality: "Excellent — full repo",
    codebaseAccess: "Full project files",
    bestFor: "Thorough code review",
    requires: "claude CLI installed",
    icon: Terminal,
  },
  {
    mode: "cursor_cli",
    title: "Cursor CLI",
    description: "Deep analysis via Cursor",
    speed: "30s-2min",
    reviewQuality: "Excellent — full repo",
    codebaseAccess: "Full project files",
    bestFor: "Cursor users",
    requires: "cursor CLI installed",
    icon: Monitor,
  },
];

export function AIProviderSelector({
  mode,
  hasApiKey,
  model,
  customInstructions,
  onModeChange,
  onApiKeyChange,
  onModelChange,
  onCustomInstructionsChange,
}: Props) {
  const [showApiKey, setShowApiKey] = useState(false);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeyDirty, setApiKeyDirty] = useState(false);

  return (
    <div className="space-y-6">
      {/* Provider Cards */}
      <div className="grid grid-cols-3 gap-3">
        {PROVIDER_CARDS.map((card) => {
          const Icon = card.icon;
          const isSelected = mode === card.mode;
          return (
            <button
              key={card.mode}
              onClick={() => onModeChange(card.mode)}
              className={`flex flex-col rounded-lg border-2 p-4 text-left transition-all hover:border-primary/50 ${
                isSelected
                  ? "border-primary bg-primary/5"
                  : "border-border"
              }`}
            >
              <div className="flex items-center gap-2 mb-3">
                <div className={`flex h-8 w-8 items-center justify-center rounded-lg ${
                  isSelected ? "bg-primary/10" : "bg-muted"
                }`}>
                  <Icon className={`h-4 w-4 ${isSelected ? "text-primary" : "text-muted-foreground"}`} />
                </div>
                <span className="text-sm font-semibold">{card.title}</span>
              </div>
              <p className="text-xs text-muted-foreground mb-3">{card.description}</p>
              <div className="space-y-1.5 text-xs">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Speed</span>
                  <span className="font-medium">{card.speed}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Review quality</span>
                  <span className="font-medium">{card.reviewQuality}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Codebase</span>
                  <span className="font-medium">{card.codebaseAccess}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Best for</span>
                  <span className="font-medium">{card.bestFor}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Requires</span>
                  <span className="font-medium">{card.requires}</span>
                </div>
              </div>
            </button>
          );
        })}
      </div>

      {/* Provider-specific config */}
      {mode === "api" && (
        <div className="space-y-4 rounded-lg border p-4">
          <div className="space-y-2">
            <Label>API Key</Label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  type={showApiKey ? "text" : "password"}
                  placeholder="sk-ant-..."
                  value={apiKeyDirty ? apiKeyInput : (hasApiKey ? "••••••••" : "")}
                  onChange={(e) => { setApiKeyDirty(true); setApiKeyInput(e.target.value); }}
                  onBlur={() => {
                    if (apiKeyDirty && apiKeyInput) {
                      onApiKeyChange(apiKeyInput);
                    }
                  }}
                />
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setShowApiKey(!showApiKey)}
                aria-label="Toggle API key visibility"
              >
                {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </Button>
            </div>
          </div>
        </div>
      )}

      {mode !== "api" && (
        <div className="rounded-lg border p-4">
          <p className="text-sm text-muted-foreground">
            {mode === "claude_cli"
              ? "Claude Code CLI will be used for deep analysis. Make sure the 'claude' command is available in your PATH."
              : "Cursor CLI will be used for deep analysis. Make sure the 'cursor' command is available in your PATH."}
          </p>
        </div>
      )}

      {/* Model selection */}
      <div className="space-y-2">
        <Label>Model</Label>
        <Select
          value={model || "auto"}
          onValueChange={(v) => onModelChange(v === "auto" ? "" : v)}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="auto">Auto (default)</SelectItem>
            <SelectItem value="claude-sonnet-4-6">Claude Sonnet 4.6</SelectItem>
            <SelectItem value="claude-opus-4-6">Claude Opus 4.6</SelectItem>
            <SelectItem value="claude-haiku-4-5-20251001">Claude Haiku 4.5</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Custom Instructions */}
      <div className="space-y-2">
        <Label>Custom Instructions</Label>
        <textarea
          className="flex min-h-[100px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-0"
          placeholder="Add project-specific rules for the AI (e.g., 'We use conventional commits', 'All public APIs need JSDoc')"
          value={customInstructions || ""}
          onChange={(e) => onCustomInstructionsChange(e.target.value)}
          rows={4}
        />
      </div>
    </div>
  );
}
