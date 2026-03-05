import { useState } from "react";
import { Loader2, Sparkles } from "lucide-react";
import type { Project, Item } from "@/types";

interface SyncProgressBarProps {
  entries: [string, string | null][];
  projects: Project[];
  analysisEntries?: [string, string | null][];
  items?: Item[];
  variant?: "default" | "centered";
}

const MAX_VISIBLE = 3;

function SyncRow({
  projectId,
  phase,
  project,
  size = "sm",
}: {
  projectId: string;
  phase: string | null;
  project: { owner: string; name: string } | undefined;
  size?: "sm" | "md";
}) {
  const isMd = size === "md";
  return (
    <div key={projectId} className="flex items-center gap-2">
      <Loader2
        className={`${isMd ? "h-5 w-5" : "h-3.5 w-3.5"} animate-spin ${isMd ? "" : "text-primary"} shrink-0`}
      />
      <p
        className={`${isMd ? "text-sm" : "text-xs text-muted-foreground"} truncate`}
      >
        {project ? (
          <span className="font-medium">
            {project.owner}/{project.name}
          </span>
        ) : null}
        {project && phase ? " — " : ""}
        {phase ?? "Syncing..."}
      </p>
    </div>
  );
}

function AnalysisRow({
  itemId,
  status,
  item,
  size = "sm",
}: {
  itemId: string;
  status: string | null;
  item: Item | undefined;
  size?: "sm" | "md";
}) {
  const isMd = size === "md";
  return (
    <div key={itemId} className="flex items-center gap-2">
      <Sparkles
        className={`${isMd ? "h-5 w-5" : "h-3.5 w-3.5"} shrink-0 animate-pulse text-blue-500`}
      />
      <p
        className={`${isMd ? "text-sm" : "text-xs text-muted-foreground"} truncate`}
      >
        {item ? (
          <span className="font-medium truncate">
            {item.title}
          </span>
        ) : null}
        {item && status ? " — " : ""}
        {status ?? "Analyzing..."}
      </p>
    </div>
  );
}

export function SyncProgressBar({
  entries,
  projects,
  analysisEntries = [],
  items = [],
  variant = "default",
}: SyncProgressBarProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  const projectMap = new Map(projects.map((p) => [p.id, p]));
  const itemMap = new Map(items.map((i) => [i.id, i]));

  const allRows = [
    ...entries.map(([id, phase]) => ({ type: "sync" as const, id, message: phase })),
    ...analysisEntries.map(([id, status]) => ({ type: "analysis" as const, id, message: status })),
  ];

  const hasOverflow = allRows.length > MAX_VISIBLE;
  const hiddenCount = allRows.length - MAX_VISIBLE;

  const alwaysVisible = allRows.slice(0, MAX_VISIBLE);
  const overflowRows = allRows.slice(MAX_VISIBLE);

  const isCentered = variant === "centered";
  const rowSize = isCentered ? "md" : "sm";

  function renderRow(row: (typeof allRows)[number]) {
    if (row.type === "sync") {
      return (
        <SyncRow
          key={`sync-${row.id}`}
          projectId={row.id}
          phase={row.message}
          project={projectMap.get(row.id)}
          size={rowSize}
        />
      );
    }
    return (
      <AnalysisRow
        key={`analysis-${row.id}`}
        itemId={row.id}
        status={row.message}
        item={itemMap.get(row.id)}
        size={rowSize}
      />
    );
  }

  return (
    <div
      className={
        isCentered
          ? "flex flex-col items-center gap-3"
          : "border-b bg-muted/50 px-4 py-2 space-y-1"
      }
    >
      {alwaysVisible.map(renderRow)}

      {hasOverflow && (
        <>
          <div
            className="grid transition-[grid-template-rows] duration-200 ease-in-out"
            style={{
              gridTemplateRows: isExpanded ? "1fr" : "0fr",
            }}
          >
            <div className="overflow-hidden">
              <div
                className={`${isCentered ? "flex flex-col items-center gap-3" : "space-y-1"} ${isExpanded ? "max-h-[200px] overflow-y-auto" : ""}`}
              >
                {overflowRows.map(renderRow)}
              </div>
            </div>
          </div>

          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-xs text-primary hover:underline"
          >
            {isExpanded ? "Show less" : `Show ${hiddenCount} more\u2026`}
          </button>
        </>
      )}
    </div>
  );
}
