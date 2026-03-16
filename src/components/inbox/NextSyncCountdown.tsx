import { useState, useEffect } from "react";
import { Timer } from "lucide-react";
import { useAppStore, useNavigationStore } from "@/stores/appStore";
import {
  HoverCard,
  HoverCardTrigger,
  HoverCardContent,
} from "@/components/ui/hover-card";

function formatRemaining(seconds: number): string {
  if (seconds <= 0) return "soon";
  if (seconds < 60) return `${seconds}s`;
  const m = Math.ceil(seconds / 60);
  return `${m}m`;
}

function formatInterval(seconds: number): string {
  if (seconds < 60) return `${seconds} seconds`;
  const m = Math.floor(seconds / 60);
  return m === 1 ? "1 minute" : `${m} minutes`;
}

export function NextSyncCountdown() {
  const lastSyncAt = useAppStore((s) => s.syncStatus.lastSyncAt);
  const refreshInterval = useAppStore((s) => s.refreshInterval);
  const isSyncing =
    Object.keys(useAppStore((s) => s.syncingProjects)).length > 0;

  const [remaining, setRemaining] = useState<number | null>(null);

  useEffect(() => {
    if (!lastSyncAt || refreshInterval <= 0 || isSyncing) {
      setRemaining(null);
      return;
    }

    function tick() {
      const elapsed = Math.floor(
        (Date.now() - new Date(lastSyncAt!).getTime()) / 1000,
      );
      const left = Math.max(0, refreshInterval - elapsed);
      setRemaining(left);
    }

    tick();
    const id = setInterval(
      tick,
      remaining !== null && remaining <= 60 ? 1000 : 10000,
    );
    return () => clearInterval(id);
  }, [lastSyncAt, refreshInterval, isSyncing, remaining !== null && remaining <= 60]);

  if (remaining === null || isSyncing) return null;

  return (
    <HoverCard openDelay={300} closeDelay={100}>
      <HoverCardTrigger asChild>
        <div className="flex cursor-default items-center gap-1 mr-1 text-[11px] text-muted-foreground/50 select-none">
          <Timer className="h-2.5 w-2.5" />
          <span>{formatRemaining(remaining)}</span>
        </div>
      </HoverCardTrigger>
      <HoverCardContent
        align="end"
        side="bottom"
        className="w-auto max-w-56 p-2.5"
      >
        <p className="text-xs text-muted-foreground">
          Auto-sync runs every{" "}
          <span className="font-medium text-foreground">
            {formatInterval(refreshInterval)}
          </span>
          . You can change this in{" "}
          <button
            onClick={() => useNavigationStore.getState().openSettings("sync")}
            className="font-medium text-primary hover:underline"
          >
            Settings
          </button>
          .
        </p>
      </HoverCardContent>
    </HoverCard>
  );
}
