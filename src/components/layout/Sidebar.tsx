import { useState, useEffect, useMemo } from "react";
import { errorMessage } from "@/lib/utils";
import { getVersion } from "@tauri-apps/api/app";
import { useAppStore } from "@/stores/appStore";
import { useProjects } from "@/hooks/useProjects";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
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
  const { setItems, itemTypeFilter, setItemTypeFilter, setCurrentPage, showAnalyzedOnly, setShowAnalyzedOnly, analyzedItemIds, showStarredOnly, setShowStarredOnly, showDismissedOnly, setShowDismissedOnly, items, themePreference, setThemePreference, refreshInbox } = useAppStore();
  const [version, setVersion] = useState("");
  const noteCount = useMemo(() => items.filter((i) => i.item_type === "note").length, [items]);
  const analyzedNoteCount = useMemo(() => items.filter((i) => i.item_type === "note" && i.type_data.kind === "note" && i.type_data.draft_status === "ready" && !analyzedItemIds.has(i.id)).length, [items, analyzedItemIds]);

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  const handleTypeFilterClick = (value: ItemTypeFilter) => {
    setItemTypeFilter(value);
  };

  return (
    <div className="flex h-full w-64 flex-col border-r bg-muted/40">
      <div className="flex h-14 items-center gap-2 border-b px-4">
        <img src="/app-icon.png" alt="" className="h-6 w-6" />
        <h1 className="text-sm font-bold tracking-tight">Ossue</h1>
      </div>

      <div className="px-3 py-2">
        <p className="mb-2 px-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Filters
        </p>
        <div className="flex flex-col gap-1">
          {typeFilters.map((f) => (
            <Button
              key={f.value}
              variant={itemTypeFilter === f.value ? "secondary" : "ghost"}
              size="sm"
              className={`justify-start gap-2 ${itemTypeFilter === f.value ? (f.value === "note" ? "border-l-2 border-l-amber-500 dark:border-l-amber-400" : "border-l-2 border-l-primary") : ""}`}
              onClick={() => handleTypeFilterClick(f.value)}
            >
              {f.icon}
              {f.label}
              {f.value === "note" && noteCount > 0 && (
                <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-amber-500/15 dark:bg-amber-400/15 px-1 text-[10px] font-semibold tabular-nums text-amber-600 dark:text-amber-400">
                  {noteCount}
                </span>
              )}
            </Button>
          ))}
        </div>
        <Separator className="my-3" />
        <p className="mb-2 px-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          Show Only
        </p>
        <div className="flex flex-col gap-1">
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
            {(analyzedItemIds.size + analyzedNoteCount) > 0 && (
              <span className="ml-auto text-xs text-muted-foreground">{analyzedItemIds.size + analyzedNoteCount}</span>
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
            {items.filter((i) => i.is_starred).length > 0 && (
              <span className="ml-auto text-xs text-muted-foreground">
                {items.filter((i) => i.is_starred).length}
              </span>
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
          </Button>
        </div>
      </div>

      <Separator />

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

      <Separator />

      <div className="p-3 flex flex-col gap-1">
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
      </div>
    </div>
  );
}
