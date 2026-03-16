import { useState, useEffect, useCallback } from "react";
import { toast } from "sonner";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import * as api from "@/lib/tauri";
import type { Platform } from "@/types";
import { errorMessage } from "@/lib/utils";
import { cn } from "@/lib/utils";

export const SETTING_KEYS = {
  syncFromDateIssues: "sync_from_date_issues",
  syncFromDatePrs: "sync_from_date_prs",
  syncFromDateDiscussions: "sync_from_date_discussions",
  syncIssues: "sync_issues_enabled",
  syncPRs: "sync_prs_enabled",
  syncDiscussions: "sync_discussions_enabled",
  /** @deprecated Used for backward compatibility migration */
  legacySyncFromDate: "sync_from_date",
} as const;

export type DatePreset = "disabled" | "all" | "today" | "yesterday" | "last_week" | "last_month" | "custom";

export function getPresetDate(preset: DatePreset): string {
  const now = new Date();
  switch (preset) {
    case "today": {
      return now.toISOString().split("T")[0];
    }
    case "yesterday": {
      now.setDate(now.getDate() - 1);
      return now.toISOString().split("T")[0];
    }
    case "last_week": {
      now.setDate(now.getDate() - 7);
      return now.toISOString().split("T")[0];
    }
    case "last_month": {
      now.setMonth(now.getMonth() - 1);
      return now.toISOString().split("T")[0];
    }
    default:
      return "";
  }
}

function detectPreset(dateStr: string): DatePreset {
  if (!dateStr) return "all";

  const now = new Date();
  const today = now.toISOString().split("T")[0];

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const yesterdayStr = yesterday.toISOString().split("T")[0];

  const lastWeek = new Date(now);
  lastWeek.setDate(lastWeek.getDate() - 7);
  const lastWeekStr = lastWeek.toISOString().split("T")[0];

  const lastMonth = new Date(now);
  lastMonth.setMonth(lastMonth.getMonth() - 1);
  const lastMonthStr = lastMonth.toISOString().split("T")[0];

  if (dateStr === today) return "today";
  if (dateStr === yesterdayStr) return "yesterday";
  if (dateStr === lastWeekStr) return "last_week";
  if (dateStr === lastMonthStr) return "last_month";
  return "custom";
}

interface CategoryState {
  preset: DatePreset;
  date: string;
}

interface SyncFiltersProps {
  projectId: string;
  platform: Platform;
  compact?: boolean;
}

const CATEGORIES = [
  { key: "issues" as const, label: "Issues", dateKey: SETTING_KEYS.syncFromDateIssues, enabledKey: SETTING_KEYS.syncIssues },
  { key: "prs" as const, label: "Pull Requests", dateKey: SETTING_KEYS.syncFromDatePrs, enabledKey: SETTING_KEYS.syncPRs },
  { key: "discussions" as const, label: "Discussions", dateKey: SETTING_KEYS.syncFromDateDiscussions, enabledKey: SETTING_KEYS.syncDiscussions },
] as const;

type CategoryKey = typeof CATEGORIES[number]["key"];

export function SyncFilters({
  projectId,
  platform,
  compact = false,
}: SyncFiltersProps) {
  const [state, setState] = useState<Record<CategoryKey, CategoryState>>({
    issues: { preset: "all", date: "" },
    prs: { preset: "all", date: "" },
    discussions: { preset: "all", date: "" },
  });
  const [loaded, setLoaded] = useState(false);

  const loadSettings = useCallback(async () => {
    try {
      const settings = await api.getProjectSettings(projectId);
      const map = new Map(settings.map((s) => [s.key, s.value]));

      const legacyDate = map.get(SETTING_KEYS.legacySyncFromDate) ?? "";

      const loadCategory = (cat: typeof CATEGORIES[number]): CategoryState => {
        const enabled = map.get(cat.enabledKey) !== "false";
        if (!enabled) {
          return { preset: "disabled", date: "" };
        }
        const dateVal = map.get(cat.dateKey) ?? legacyDate;
        return { preset: detectPreset(dateVal), date: dateVal };
      };

      setState({
        issues: loadCategory(CATEGORIES[0]),
        prs: loadCategory(CATEGORIES[1]),
        discussions: loadCategory(CATEGORIES[2]),
      });
      setLoaded(true);
    } catch (err) {
      toast.error("Failed to load sync filters", {
        description: errorMessage(err),
      });
    }
  }, [projectId]);

  useEffect(() => {
    setLoaded(false);
    loadSettings();
  }, [loadSettings]);

  const handlePresetChange = async (cat: typeof CATEGORIES[number], preset: DatePreset) => {
    const prev = state[cat.key];
    let newDate = "";

    if (preset === "disabled") {
      setState((s) => ({ ...s, [cat.key]: { preset: "disabled", date: "" } }));
      try {
        await api.deleteProjectSetting(projectId, cat.dateKey);
        await api.updateProjectSetting(projectId, cat.enabledKey, "false");
      } catch (err) {
        toast.error("Failed to update sync filter", { description: errorMessage(err) });
        setState((s) => ({ ...s, [cat.key]: prev }));
      }
      return;
    }

    if (preset === "custom") {
      newDate = prev.date;
    } else if (preset !== "all") {
      newDate = getPresetDate(preset);
    }

    setState((s) => ({ ...s, [cat.key]: { preset, date: newDate } }));
    try {
      if (newDate) {
        await api.updateProjectSetting(projectId, cat.dateKey, newDate);
      } else {
        await api.deleteProjectSetting(projectId, cat.dateKey);
      }
      // Ensure enabled when not disabled
      await api.updateProjectSetting(projectId, cat.enabledKey, "true");
    } catch (err) {
      toast.error("Failed to update sync filter", { description: errorMessage(err) });
      setState((s) => ({ ...s, [cat.key]: prev }));
    }
  };

  const handleCustomDateChange = async (cat: typeof CATEGORIES[number], value: string) => {
    const prev = state[cat.key];
    setState((s) => ({ ...s, [cat.key]: { preset: "custom", date: value } }));
    try {
      if (value) {
        await api.updateProjectSetting(projectId, cat.dateKey, value);
      } else {
        await api.deleteProjectSetting(projectId, cat.dateKey);
      }
    } catch (err) {
      toast.error("Failed to update date filter", { description: errorMessage(err) });
      setState((s) => ({ ...s, [cat.key]: prev }));
    }
  };

  if (!loaded) return null;

  const visibleCategories = CATEGORIES.filter(
    (cat) => !(cat.key === "discussions" && platform === "gitlab")
  );

  return (
    <div className={cn("space-y-2", compact && "space-y-1.5")}>
      {visibleCategories.map((cat) => (
        <div key={cat.key} className="flex items-center gap-2">
          <Label
            className={cn(
              "text-sm w-28 shrink-0",
              compact && "text-xs w-24"
            )}
          >
            {cat.label}
          </Label>
          <Select
            value={state[cat.key].preset}
            onValueChange={(v) => handlePresetChange(cat, v as DatePreset)}
          >
            <SelectTrigger className={cn("w-auto", compact && "h-7 text-xs")} size={compact ? "sm" : "default"}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="disabled">Disabled</SelectItem>
              <SelectItem value="all">All time</SelectItem>
              <SelectItem value="today">Today</SelectItem>
              <SelectItem value="yesterday">Yesterday</SelectItem>
              <SelectItem value="last_week">Last week</SelectItem>
              <SelectItem value="last_month">Last month</SelectItem>
              <SelectItem value="custom">Custom date</SelectItem>
            </SelectContent>
          </Select>
          {state[cat.key].preset === "custom" && (
            <Input
              type="date"
              value={state[cat.key].date}
              onChange={(e) => handleCustomDateChange(cat, e.target.value)}
              className={cn("w-auto", compact && "h-7 text-xs")}
            />
          )}
        </div>
      ))}
    </div>
  );
}
