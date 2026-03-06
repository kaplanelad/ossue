import { useState, useEffect, useMemo } from "react";
import { errorMessage } from "@/lib/utils";
import { getVersion } from "@tauri-apps/api/app";
import { useAppStore } from "@/stores/appStore";
import { useProjects } from "@/hooks/useProjects";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { toast } from "sonner";
import * as api from "@/lib/tauri";
import {
  Inbox,
  GitPullRequest,
  MessageSquare,
  CircleDot,
  Settings,
  FolderGit2,
  MoreVertical,
  FolderX,
  Pause,
  Play,
  AlertTriangle,
  Sparkles,
  Star,
  EyeOff,
  Check,
  Sun,
  Moon,
  Monitor,
  StickyNote,
  PanelLeftClose,
} from "lucide-react";
import type { ItemTypeFilter } from "@/types";
import type { ThemePreference } from "@/stores/appStore";

const themeOptions: { value: ThemePreference; label: string; icon: React.ReactNode }[] = [
  { value: "light", label: "Light", icon: <Sun className="h-4 w-4" /> },
  { value: "dark", label: "Dark", icon: <Moon className="h-4 w-4" /> },
  { value: "system", label: "System", icon: <Monitor className="h-4 w-4" /> },
];

const themeIcons: Record<ThemePreference, React.ReactNode> = {
  light: <Sun className="h-4 w-4" />,
  dark: <Moon className="h-4 w-4" />,
  system: <Monitor className="h-4 w-4" />,
};

const typeFilters: { value: ItemTypeFilter; label: string; icon: React.ReactNode }[] = [
  { value: "all", label: "All", icon: <Inbox className="h-4 w-4" /> },
  { value: "note", label: "Notes", icon: <StickyNote className="h-4 w-4" /> },
  { value: "issue", label: "Issues", icon: <CircleDot className="h-4 w-4" /> },
  { value: "pr", label: "PRs", icon: <GitPullRequest className="h-4 w-4" /> },
  { value: "discussion", label: "Discussions", icon: <MessageSquare className="h-4 w-4" /> },
];

export function Sidebar() {
  const { projects, selectedProjectIds, toggleProjectSelection, clearProjectSelection, fetchProjects } = useProjects();
  const { setItems, itemTypeFilter, setItemTypeFilter, setCurrentPage, showAnalyzedOnly, setShowAnalyzedOnly, analyzedItemIds, showStarredOnly, setShowStarredOnly, showDismissedOnly, setShowDismissedOnly, items, themePreference, setThemePreference, refreshInbox, dismissedCounts, draftNoteCount } = useAppStore();
  const [version, setVersion] = useState("");
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem("sidebar-collapsed") === "true");

  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      const next = !prev;
      localStorage.setItem("sidebar-collapsed", String(next));
      return next;
    });
  };
  // Items filtered by project + type filter (for "Show Only" counters)
  const baseFilteredItems = useMemo(() => {
    let result = items;
    if (selectedProjectIds.length > 0) {
      const selected = new Set(selectedProjectIds);
      result = result.filter((i) => selected.has(i.project_id));
    }
    if (itemTypeFilter !== "all") {
      result = result.filter((i) => i.item_type === itemTypeFilter);
    }
    return result;
  }, [items, selectedProjectIds, itemTypeFilter]);

  const noteCount = draftNoteCount;
  const projectFilteredItems = useMemo(() => {
    if (selectedProjectIds.length === 0) return items;
    const selected = new Set(selectedProjectIds);
    return items.filter((i) => selected.has(i.project_id));
  }, [items, selectedProjectIds]);
  const issueCount = useMemo(() => projectFilteredItems.filter((i) => i.item_type === "issue").length, [projectFilteredItems]);
  const prCount = useMemo(() => projectFilteredItems.filter((i) => i.item_type === "pr").length, [projectFilteredItems]);
  const starredCount = useMemo(() => baseFilteredItems.filter((i) => i.is_starred).length, [baseFilteredItems]);
  const analyzedCount = useMemo(() => {
    const analyzed = baseFilteredItems.filter((i) => analyzedItemIds.has(i.id));
    const readyNotes = baseFilteredItems.filter((i) => i.item_type === "note" && i.type_data.kind === "note" && i.type_data.draft_status === "ready" && !analyzedItemIds.has(i.id));
    return analyzed.length + readyNotes.length;
  }, [baseFilteredItems, analyzedItemIds]);
  const dismissedCount = useMemo(() => {
    const selectedSet = selectedProjectIds.length > 0 ? new Set(selectedProjectIds) : null;
    return dismissedCounts
      .filter((c) => {
        if (selectedSet && !selectedSet.has(c.project_id)) return false;
        if (itemTypeFilter !== "all" && c.item_type !== itemTypeFilter) return false;
        return true;
      })
      .reduce((sum, c) => sum + c.count, 0);
  }, [dismissedCounts, selectedProjectIds, itemTypeFilter]);

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  const handleTypeFilterClick = (value: ItemTypeFilter) => {
    setItemTypeFilter(value);
  };

  return (
    <div className={`flex h-full flex-col border-r bg-muted/40 transition-[width] duration-200 ${collapsed ? "w-14" : "w-64"}`}>
      <div className="flex h-14 items-center border-b px-3">
        {collapsed ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <button className="mx-auto flex h-8 w-8 items-center justify-center" onClick={toggleCollapsed}>
                <img src="/app-icon.png" alt="" className="h-6 w-6" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">Expand sidebar</TooltipContent>
          </Tooltip>
        ) : (
          <>
            <img src="/app-icon.png" alt="" className="h-6 w-6" />
            <h1 className="ml-2 text-sm font-bold tracking-tight">Ossue</h1>
            <button
              className="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground"
              onClick={toggleCollapsed}
              aria-label="Collapse sidebar"
            >
              <PanelLeftClose className="h-4 w-4" />
            </button>
          </>
        )}
      </div>

      <div className={collapsed ? "px-1.5 py-2" : "px-3 py-2"}>
        {!collapsed && (
          <p className="mb-2 px-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Filters
          </p>
        )}
        <div className="flex flex-col gap-1">
          {typeFilters.map((f) =>
            collapsed ? (
              <Tooltip key={f.value}>
                <TooltipTrigger asChild>
                  <Button
                    variant={itemTypeFilter === f.value ? "secondary" : "ghost"}
                    size="icon"
                    className={`h-8 w-8 mx-auto ${itemTypeFilter === f.value ? (f.value === "note" ? "border-l-2 border-l-amber-500 dark:border-l-amber-400" : "border-l-2 border-l-primary") : ""}`}
                    onClick={() => handleTypeFilterClick(f.value)}
                  >
                    {f.icon}
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">{f.label}</TooltipContent>
              </Tooltip>
            ) : (
              <Button
                key={f.value}
                variant={itemTypeFilter === f.value ? "secondary" : "ghost"}
                size="sm"
                className={`w-full !justify-start gap-2 ${itemTypeFilter === f.value ? (f.value === "note" ? "border-l-2 border-l-amber-500 dark:border-l-amber-400" : "border-l-2 border-l-primary") : ""}`}
                onClick={() => handleTypeFilterClick(f.value)}
              >
                {f.icon}
                {f.label}
                {f.value === "note" && noteCount > 0 && (
                  <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-amber-500/15 dark:bg-amber-400/15 px-1 text-[10px] font-semibold tabular-nums text-amber-600 dark:text-amber-400">
                    {noteCount}
                  </span>
                )}
                {f.value === "issue" && issueCount > 0 && (
                  <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-purple-500/15 dark:bg-purple-400/15 px-1 text-[10px] font-semibold tabular-nums text-purple-600 dark:text-purple-400">
                    {issueCount}
                  </span>
                )}
                {f.value === "pr" && prCount > 0 && (
                  <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-blue-500/15 dark:bg-blue-400/15 px-1 text-[10px] font-semibold tabular-nums text-blue-600 dark:text-blue-400">
                    {prCount}
                  </span>
                )}
              </Button>
            )
          )}
        </div>
        <Separator className="my-3" />
        {!collapsed && (
          <p className="mb-2 px-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Show Only
          </p>
        )}
        <div className="flex flex-col gap-1">
          {collapsed ? (
            <>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant={showAnalyzedOnly ? "secondary" : "ghost"}
                    size="icon"
                    className={`h-8 w-8 mx-auto ${showAnalyzedOnly ? "border-l-2 border-l-primary" : ""}`}
                    onClick={() => {
                      const next = !showAnalyzedOnly;
                      setShowAnalyzedOnly(next);
                      if (next) { setShowStarredOnly(false); setShowDismissedOnly(false); }
                    }}
                  >
                    <Sparkles className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">AI Analyzed</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant={showStarredOnly ? "secondary" : "ghost"}
                    size="icon"
                    className={`h-8 w-8 mx-auto ${showStarredOnly ? "border-l-2 border-l-primary" : ""}`}
                    onClick={() => {
                      const next = !showStarredOnly;
                      setShowStarredOnly(next);
                      if (next) { setShowAnalyzedOnly(false); setShowDismissedOnly(false); }
                    }}
                  >
                    <Star className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">Starred</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant={showDismissedOnly ? "secondary" : "ghost"}
                    size="icon"
                    className={`h-8 w-8 mx-auto ${showDismissedOnly ? "border-l-2 border-l-primary" : ""}`}
                    onClick={() => {
                      const next = !showDismissedOnly;
                      setShowDismissedOnly(next);
                      if (next) { setShowAnalyzedOnly(false); setShowStarredOnly(false); }
                      refreshInbox();
                    }}
                  >
                    <EyeOff className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">Dismissed</TooltipContent>
              </Tooltip>
            </>
          ) : (
            <>
              <Button
                variant={showAnalyzedOnly ? "secondary" : "ghost"}
                size="sm"
                className={`justify-start gap-2 ${showAnalyzedOnly ? "border-l-2 border-l-primary" : ""}`}
                onClick={() => {
                  const next = !showAnalyzedOnly;
                  setShowAnalyzedOnly(next);
                  if (next) { setShowStarredOnly(false); setShowDismissedOnly(false); }
                }}
              >
                <Sparkles className="h-4 w-4" />
                AI Analyzed
                {analyzedCount > 0 && (
                  <span className="ml-auto text-xs text-muted-foreground">{analyzedCount}</span>
                )}
              </Button>
              <Button
                variant={showStarredOnly ? "secondary" : "ghost"}
                size="sm"
                className={`justify-start gap-2 ${showStarredOnly ? "border-l-2 border-l-primary" : ""}`}
                onClick={() => {
                  const next = !showStarredOnly;
                  setShowStarredOnly(next);
                  if (next) { setShowAnalyzedOnly(false); setShowDismissedOnly(false); }
                }}
              >
                <Star className="h-4 w-4" />
                Starred
                {starredCount > 0 && (
                  <span className="ml-auto text-xs text-muted-foreground">{starredCount}</span>
                )}
              </Button>
              <Button
                variant={showDismissedOnly ? "secondary" : "ghost"}
                size="sm"
                className={`justify-start gap-2 ${showDismissedOnly ? "border-l-2 border-l-primary" : ""}`}
                onClick={() => {
                  const next = !showDismissedOnly;
                  setShowDismissedOnly(next);
                  if (next) { setShowAnalyzedOnly(false); setShowStarredOnly(false); }
                  refreshInbox();
                }}
              >
                <EyeOff className="h-4 w-4" />
                Dismissed
                {dismissedCount > 0 && (
                  <span className="ml-auto text-xs text-muted-foreground">{dismissedCount}</span>
                )}
              </Button>
            </>
          )}
        </div>
      </div>

      <Separator />

      {!collapsed && (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden px-3 py-2">
          <p className="mb-2 shrink-0 px-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Projects
          </p>
          <ScrollArea className="min-h-0 flex-1">
            <div className="flex flex-col gap-1">
              <Button
                variant={selectedProjectIds.length === 0 ? "secondary" : "ghost"}
                size="sm"
                className={`justify-start gap-2 ${selectedProjectIds.length === 0 ? "border-l-2 border-l-primary" : ""}`}
                onClick={() => clearProjectSelection()}
              >
                <Inbox className="h-4 w-4" />
                All Projects
              </Button>
              {projects.map((project) => {
                const isSelected = selectedProjectIds.includes(project.id);
                return (
                  <Dialog key={project.id}>
                    <div
                      className={`group relative flex h-8 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium cursor-pointer ${
                        isSelected
                          ? "bg-secondary text-secondary-foreground"
                          : "hover:bg-accent hover:text-accent-foreground"
                      }${!project.sync_enabled ? " opacity-50" : ""}`}
                      onClick={() => toggleProjectSelection(project.id)}
                    >
                      {isSelected ? (
                        <Check className="h-4 w-4 shrink-0 text-primary" />
                      ) : (
                        <FolderGit2 className="h-4 w-4 shrink-0" />
                      )}
                      <span className="truncate">
                        {project.owner}/{project.name}
                      </span>
                      {project.last_sync_error && (
                        <span
                          title={
                            /401|Unauthorized/i.test(project.last_sync_error)
                              ? "Authentication failed — update token in Settings"
                              : project.last_sync_error
                          }
                        >
                          <AlertTriangle className="h-3 w-3 text-yellow-500 shrink-0" />
                        </span>
                      )}
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <button
                            className="ml-auto flex h-5 w-5 shrink-0 items-center justify-center rounded opacity-0 group-hover:opacity-100 hover:bg-accent-foreground/10"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <MoreVertical className="h-3.5 w-3.5" />
                          </button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            onClick={async () => {
                              try {
                                await api.toggleProjectSync(project.id, !project.sync_enabled);
                                fetchProjects();
                              } catch (err) {
                                toast.error("Failed to toggle sync", { description: errorMessage(err) });
                              }
                            }}
                          >
                            {project.sync_enabled ? (
                              <><Pause className="h-4 w-4" /> Disable sync</>
                            ) : (
                              <><Play className="h-4 w-4" /> Enable sync</>
                            )}
                          </DropdownMenuItem>
                          <DropdownMenuSeparator />
                          <DialogTrigger asChild>
                            <DropdownMenuItem variant="destructive">
                              <FolderX className="h-4 w-4" />
                              Clear data
                            </DropdownMenuItem>
                          </DialogTrigger>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>Clear Project Data</DialogTitle>
                        <DialogDescription>
                          This will delete all synced items and chat history for{" "}
                          <span className="font-medium">{project.owner}/{project.name}</span>.
                          The project will remain tracked and can be re-synced.
                        </DialogDescription>
                      </DialogHeader>
                      <DialogFooter>
                        <DialogClose asChild>
                          <Button variant="outline">Cancel</Button>
                        </DialogClose>
                        <DialogClose asChild>
                          <Button
                            variant="destructive"
                            onClick={async () => {
                              try {
                                await api.clearProjectData(project.id);
                                const currentItems = useAppStore.getState().items;
                                setItems(currentItems.filter((i) => i.project_id !== project.id));
                              } catch (err) {
                                toast.error("Failed to clear project data", { description: errorMessage(err) });
                              }
                            }}
                          >
                            <FolderX className="h-4 w-4" />
                            Clear Data
                          </Button>
                        </DialogClose>
                      </DialogFooter>
                    </DialogContent>
                  </Dialog>
                );
              })}
            </div>
          </ScrollArea>
        </div>
      )}

      {collapsed && <div className="flex-1" />}

      <Separator />

      <div className={`flex flex-col gap-1 ${collapsed ? "items-center p-2" : "p-3"}`}>
        {collapsed ? (
          <>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  onClick={() => setCurrentPage("settings")}
                >
                  <Settings className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="right">Settings</TooltipContent>
            </Tooltip>
            <DropdownMenu>
              <Tooltip>
                <TooltipTrigger asChild>
                  <DropdownMenuTrigger asChild>
                    <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="Toggle theme">
                      {themeIcons[themePreference]}
                    </Button>
                  </DropdownMenuTrigger>
                </TooltipTrigger>
                <TooltipContent side="right">Theme</TooltipContent>
              </Tooltip>
              <DropdownMenuContent align="end">
                {themeOptions.map((opt) => (
                  <DropdownMenuItem
                    key={opt.value}
                    onClick={() => setThemePreference(opt.value)}
                  >
                    {opt.icon}
                    {opt.label}
                    {themePreference === opt.value && <Check className="ml-auto h-4 w-4" />}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        ) : (
          <>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                className="flex-1 justify-start gap-2"
                onClick={() => setCurrentPage("settings")}
              >
                <Settings className="h-4 w-4" />
                Settings
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-8 w-8 shrink-0" aria-label="Toggle theme">
                    {themeIcons[themePreference]}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  {themeOptions.map((opt) => (
                    <DropdownMenuItem
                      key={opt.value}
                      onClick={() => setThemePreference(opt.value)}
                    >
                      {opt.icon}
                      {opt.label}
                      {themePreference === opt.value && <Check className="ml-auto h-4 w-4" />}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
            {version && (
              <span className="px-2 text-[10px] text-muted-foreground select-none">v{version}</span>
            )}
          </>
        )}
      </div>
    </div>
  );
}
