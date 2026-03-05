import { useState, useEffect, useCallback, Fragment } from "react";
import { formatTimeAgo, errorMessage } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from "@/components/ui/tabs";
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
import { useAppStore } from "@/stores/appStore";
import { useProjects } from "@/hooks/useProjects";
import * as api from "@/lib/tauri";
import type { AppSettings, AppPaths, Connector, BackupInfo, LogEntriesResponse, ProjectNote, AiSettings, ProjectSettingEntry } from "@/types";
import { SettingHeader } from "./SettingHeader";
import { AddProjectsDialog } from "./AddProjectsDialog";
import { AIProviderSelector } from "./AIProviderSelector";
import { AIPreferencesForm } from "./AIPreferencesForm";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import {
  ArrowLeft,
  Github,
  Trash2,
  FolderX,
  Loader2,
  Plus,
  Database,
  Search,
  RefreshCw,
  Download,
  RotateCcw,
  AlertTriangle,
  GitlabIcon,
  Link,
  ExternalLink,
  ChevronRight,
  Pause,
  Play,
  FolderGit2,
  ScrollText,
  Sparkles,
  Link2,
} from "lucide-react";

function formatLogTimestamp(ts: string): string {
  if (!ts) return "";
  const d = new Date(ts);
  if (!isNaN(d.getTime())) {
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" }) + " " + d.toLocaleTimeString();
  }
  return "—";
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function SettingsPage() {
  const { items, setItems, setSelectedItemId, clearProjectSelection, setCurrentPage } = useAppStore();
  const { projects, removeProject, fetchProjects } = useProjects();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [saving, setSaving] = useState<string | null>(null);
  const [addProjectsOpen, setAddProjectsOpen] = useState(false);

  // Connectors
  const [connectors, setConnectors] = useState<Connector[]>([]);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [newConnName, setNewConnName] = useState("");
  const [newConnPlatform, setNewConnPlatform] = useState<"github" | "gitlab">("github");
  const [newConnToken, setNewConnToken] = useState("");
  const [newConnBaseUrl, setNewConnBaseUrl] = useState("");
  const [addingConn, setAddingConn] = useState(false);

  // Database
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [confirmRestore, setConfirmRestore] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  // Logs
  const [logLevel, setLogLevelState] = useState("ERROR");
  const [displayLevel, setDisplayLevel] = useState("TRACE");
  const [logEntries, setLogEntries] = useState<LogEntriesResponse | null>(null);
  const [logSearch, setLogSearch] = useState("");
  const [logOffset, setLogOffset] = useState(0);
  const [expandedLogIndex, setExpandedLogIndex] = useState<number | null>(null);

  // Project notes
  const [projectNotes, setProjectNotes] = useState<Record<string, ProjectNote[]>>({});
  const [newNoteText, setNewNoteText] = useState<Record<string, string>>({});
  const [expandedNotes, setExpandedNotes] = useState<Set<string>>(new Set());

  // AI settings
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);

  // Per-project AI preferences
  const [projectAiSettings, setProjectAiSettings] = useState<Record<string, ProjectSettingEntry[]>>({});
  const [expandedProjectAi, setExpandedProjectAi] = useState<Set<string>>(new Set());

  const loadConnectors = useCallback(async () => {
    try {
      const c = await api.listConnectors();
      setConnectors(c);
    } catch (err) {
      console.error("Failed to load connectors:", err);
    }
  }, []);

  const loadAiSettings = useCallback(async () => {
    try {
      const s = await api.getAiSettings();
      setAiSettings(s);
    } catch (err) {
      toast.error("Failed to load AI settings", { description: errorMessage(err) });
    }
  }, []);

  const handleAiSave = async (key: string, value: string) => {
    setSaving(key);
    try {
      await api.updateSetting(key, value);
      await loadAiSettings();
    } catch (err) {
      toast.error("Failed to save setting", { description: errorMessage(err) });
    } finally {
      setSaving(null);
    }
  };

  const loadProjectAiSettings = async (projectId: string) => {
    try {
      const settings = await api.getProjectSettings(projectId);
      setProjectAiSettings(prev => ({ ...prev, [projectId]: settings }));
    } catch (err) {
      console.error("Failed to load project AI settings:", err);
    }
  };

  const handleProjectAiSave = async (projectId: string, key: string, value: string) => {
    try {
      await api.updateProjectSetting(projectId, key, value);
      await loadProjectAiSettings(projectId);
    } catch (err) {
      toast.error("Failed to save project setting", { description: errorMessage(err) });
    }
  };

  const handleProjectAiDelete = async (projectId: string, key: string) => {
    try {
      await api.deleteProjectSetting(projectId, key);
      await loadProjectAiSettings(projectId);
    } catch (err) {
      toast.error("Failed to delete project setting", { description: errorMessage(err) });
    }
  };

  const toggleProjectAi = (projectId: string) => {
    setExpandedProjectAi(prev => {
      const next = new Set(prev);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
        loadProjectAiSettings(projectId);
        if (!aiSettings) loadAiSettings();
      }
      return next;
    });
  };

  const getProjectSetting = (projectId: string, key: string): string | undefined => {
    return projectAiSettings[projectId]?.find(s => s.key === key)?.value;
  };

  useEffect(() => {
    const load = async () => {
      const s = await api.getSettings();
      setSettings(s);
      // Initialize log level from actual backend state
      const level = await api.getLogLevel();
      setLogLevelState(level);
      // Load file paths
      try {
        const paths = await api.getAppPaths();
        setAppPaths(paths);
      } catch (err) {
        console.error("Failed to load app paths:", err);
      }
    };
    load();
    loadConnectors();
  }, [loadConnectors]);

  const handleSave = async (key: string, value: string) => {
    setSaving(key);
    try {
      await api.updateSetting(key, value);
      const s = await api.getSettings();
      setSettings(s);
    } catch (err) {
      toast.error("Failed to save setting", { description: errorMessage(err) });
    } finally {
      setSaving(null);
    }
  };

  const handleAddConnector = async () => {
    if (!newConnName.trim() || !newConnToken.trim()) return;
    setAddingConn(true);
    try {
      await api.addConnector({
        name: newConnName,
        platform: newConnPlatform,
        token: newConnToken,
        base_url: newConnBaseUrl.trim() || undefined,
      });
      setAddDialogOpen(false);
      setNewConnName("");
      setNewConnToken("");
      setNewConnBaseUrl("");
      await loadConnectors();
    } catch (err) {
      toast.error("Failed to add connection", { description: errorMessage(err) });
    } finally {
      setAddingConn(false);
    }
  };

  const handleRemoveConnector = async (id: string) => {
    try {
      await api.removeConnector(id);
      await loadConnectors();
    } catch (err) {
      toast.error("Failed to remove connection", { description: errorMessage(err) });
    }
  };

  // Database functions
  const loadBackups = async () => {
    try {
      const b = await api.listBackups();
      setBackups(b);
    } catch (err) {
      console.error("Failed to load backups:", err);
    }
  };

  const handleCreateBackup = async () => {
    setSaving("backup");
    try {
      await api.createBackup();
      await loadBackups();
    } catch (err) {
      toast.error("Failed to create backup", { description: errorMessage(err) });
    } finally {
      setSaving(null);
    }
  };

  const handleRestoreBackup = async (filename: string) => {
    setSaving("restore");
    try {
      await api.restoreBackup(filename);
      setItems([]);
      setSelectedItemId(null);
      clearProjectSelection();
      setConfirmRestore(null);
      const s = await api.getSettings();
      setSettings(s);
      await loadConnectors();
    } catch (err) {
      toast.error("Failed to restore backup", { description: errorMessage(err) });
    } finally {
      setSaving(null);
    }
  };

  const handleResetDatabase = async () => {
    setSaving("reset");
    try {
      await api.resetDatabase();
      setItems([]);
      setSelectedItemId(null);
      clearProjectSelection();
      setConfirmReset(false);
      setCurrentPage("onboarding");
    } catch (err) {
      toast.error("Failed to reset database", { description: errorMessage(err) });
    } finally {
      setSaving(null);
    }
  };

  const handleDeleteBackup = async (filename: string) => {
    try {
      await api.deleteBackup(filename);
      await loadBackups();
    } catch (err) {
      toast.error("Failed to delete backup", { description: errorMessage(err) });
    }
  };

  // Log functions
  const loadLogs = useCallback(
    async (offset = 0) => {
      try {
        const entries = await api.getLogEntries(
          displayLevel,
          logSearch || undefined,
          200,
          offset
        );
        setLogEntries(entries);
        setLogOffset(offset);
      } catch (err) {
        console.error("Failed to load logs:", err);
      }
    },
    [displayLevel, logSearch]
  );

  const handleSetLogLevel = async (level: string) => {
    try {
      await api.setLogLevel(level);
      setLogLevelState(level);
    } catch (err) {
      toast.error("Failed to set log level", { description: errorMessage(err) });
    }
  };

  const handleClearLogs = async () => {
    try {
      await api.clearLogs();
      setLogEntries(null);
    } catch (err) {
      toast.error("Failed to clear logs", { description: errorMessage(err) });
    }
  };

  const connectorForProject = (connectorId: string | null) => {
    if (!connectorId) return null;
    return connectors.find((c) => c.id === connectorId) ?? null;
  };

  const loadProjectNotes = async (projectId: string) => {
    try {
      const notes = await api.listProjectNotes(projectId);
      setProjectNotes(prev => ({ ...prev, [projectId]: notes }));
    } catch (err) {
      console.error("Failed to load project notes:", err);
    }
  };

  const handleAddNote = async (projectId: string) => {
    const text = newNoteText[projectId];
    if (!text?.trim()) return;
    try {
      await api.addProjectNote(projectId, text.trim());
      setNewNoteText(prev => ({ ...prev, [projectId]: "" }));
      await loadProjectNotes(projectId);
    } catch (err) {
      toast.error(errorMessage(err));
    }
  };

  const handleRemoveNote = async (noteId: string, projectId: string) => {
    try {
      await api.removeProjectNote(noteId);
      await loadProjectNotes(projectId);
    } catch (err) {
      toast.error(errorMessage(err));
    }
  };

  const toggleNotes = (projectId: string) => {
    setExpandedNotes(prev => {
      const next = new Set(prev);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
        loadProjectNotes(projectId);
      }
      return next;
    });
  };


  const levelBadge = (level: string) => {
    switch (level.toUpperCase()) {
      case "ERROR":
        return "bg-red-500/15 text-red-600 dark:text-red-400";
      case "WARN":
        return "bg-yellow-500/15 text-yellow-600 dark:text-yellow-400";
      case "INFO":
        return "bg-blue-500/15 text-blue-600 dark:text-blue-400";
      case "DEBUG":
        return "bg-green-500/15 text-green-600 dark:text-green-400";
      case "TRACE":
        return "bg-gray-500/15 text-gray-500";
      default:
        return "bg-muted text-foreground";
    }
  };

  return (
    <div className="flex h-full w-full flex-col">
      <div className="flex items-center gap-3 border-b px-4 py-3">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setCurrentPage("main")}
          aria-label="Back to inbox"
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <img src="/app-icon.png" alt="" className="h-5 w-5" />
        <h2 className="text-base font-bold tracking-tight">Settings</h2>
      </div>

      <Tabs
        defaultValue="accounts"
        orientation="vertical"
        className="flex-1 flex min-h-0 gap-0"
      >
        <div className="w-[160px] border-r shrink-0 bg-muted/20">
          <TabsList variant="line" className="w-full py-2 pl-2 pr-[3px] gap-1 items-start">
            <TabsTrigger value="accounts" className="px-3 py-2 gap-2">
              <Link2 className="h-4 w-4 shrink-0" />
              Accounts
            </TabsTrigger>
            <TabsTrigger value="projects" className="px-3 py-2 gap-2">
              <FolderGit2 className="h-4 w-4 shrink-0" />
              Projects
            </TabsTrigger>
            <TabsTrigger value="sync" className="px-3 py-2 gap-2">
              <RefreshCw className="h-4 w-4 shrink-0" />
              Sync
            </TabsTrigger>
            <TabsTrigger
              value="database"
              className="px-3 py-2 gap-2"
              onClick={loadBackups}
            >
              <Database className="h-4 w-4 shrink-0" />
              Storage
            </TabsTrigger>
            <TabsTrigger
              value="logs"
              className="px-3 py-2 gap-2"
              onClick={() => loadLogs(0)}
            >
              <ScrollText className="h-4 w-4 shrink-0" />
              Logs
            </TabsTrigger>
            <TabsTrigger
              value="ai-provider"
              className="px-3 py-2 gap-2"
              onClick={loadAiSettings}
            >
              <Sparkles className="h-4 w-4 shrink-0" />
              AI Provider
            </TabsTrigger>
            <TabsTrigger
              value="ai-preferences"
              className="px-3 py-2 gap-2"
              onClick={loadAiSettings}
            >
              <Sparkles className="h-4 w-4 shrink-0" />
              AI Preferences
            </TabsTrigger>
          </TabsList>
        </div>

        <div className="flex-1 overflow-hidden">
          <ScrollArea className="h-full">
            {/* ===== ACCOUNTS TAB ===== */}
            <TabsContent value="accounts" className="p-6">
              <div className="space-y-4">
                <SettingHeader
                  title="Connections"
                  subtitle="Manage your GitHub and GitLab connections"
                  action={
                    <Dialog open={addDialogOpen} onOpenChange={(open) => {
                      setAddDialogOpen(open);
                      if (!open) {
                        setNewConnName("");
                        setNewConnPlatform("github");
                        setNewConnToken("");
                        setNewConnBaseUrl("");
                      }
                    }}>
                      <DialogTrigger asChild>
                        <Button variant="outline" size="sm" className="gap-2">
                          <Plus className="h-4 w-4" /> Add Connection
                        </Button>
                      </DialogTrigger>
                      <DialogContent>
                        <DialogHeader>
                          <DialogTitle>Add Connection</DialogTitle>
                          <DialogDescription>
                            Connect a GitHub or GitLab account. The token is
                            validated before saving.
                          </DialogDescription>
                        </DialogHeader>
                        <div className="space-y-4 py-2">
                          <div className="space-y-2">
                            <Label>Name</Label>
                            <Input
                              placeholder="e.g. Work GitHub"
                              value={newConnName}
                              onChange={(e) => setNewConnName(e.target.value)}
                            />
                          </div>
                          <div className="space-y-2">
                            <Label>Platform</Label>
                            <Select
                              value={newConnPlatform}
                              onValueChange={(v) =>
                                setNewConnPlatform(v as "github" | "gitlab")
                              }
                            >
                              <SelectTrigger>
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="github">GitHub</SelectItem>
                                <SelectItem value="gitlab">GitLab</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-2">
                            <Label>Token</Label>
                            <Input
                              type="password"
                              placeholder={
                                newConnPlatform === "github"
                                  ? "ghp_... or github_pat_..."
                                  : "glpat-..."
                              }
                              value={newConnToken}
                              onChange={(e) => setNewConnToken(e.target.value)}
                            />
                            {newConnPlatform === "github" ? (
                              <p className="text-xs text-muted-foreground">
                                Create a{" "}
                                <a
                                  href="https://github.com/settings/personal-access-tokens/new?name=Ossue&description=Read-only+access+for+Ossue+app&issues=read&pull_requests=read"
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  className="underline inline-flex items-center gap-0.5"
                                >
                                  fine-grained personal access token
                                  <ExternalLink className="h-3 w-3" />
                                </a>{" "}
                                with <strong>read-only</strong> access to Issues and Pull requests.
                              </p>
                            ) : (
                              <p className="text-xs text-muted-foreground">
                                Create a{" "}
                                <a
                                  href="https://gitlab.com/-/user_settings/personal_access_tokens?name=Ossue&scopes=read_api"
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  className="underline inline-flex items-center gap-0.5"
                                >
                                  personal access token
                                  <ExternalLink className="h-3 w-3" />
                                </a>{" "}
                                with the <strong>read_api</strong> scope.
                              </p>
                            )}
                          </div>
                          {newConnPlatform === "gitlab" && (
                            <div className="space-y-2">
                              <Label>Base URL (optional)</Label>
                              <Input
                                placeholder="https://gitlab.com"
                                value={newConnBaseUrl}
                                onChange={(e) => setNewConnBaseUrl(e.target.value)}
                              />
                            </div>
                          )}
                        </div>
                        <DialogFooter>
                          <Button
                            variant="outline"
                            onClick={handleAddConnector}
                            disabled={
                              !newConnName.trim() ||
                              !newConnToken.trim() ||
                              addingConn
                            }
                          >
                            {addingConn ? (
                              <Loader2 className="h-4 w-4 animate-spin mr-2" />
                            ) : null}
                            Add Connection
                          </Button>
                        </DialogFooter>
                      </DialogContent>
                    </Dialog>
                  }
                />

                <div className="space-y-2">
                  {connectors.map((conn) => (
                    <div
                      key={conn.id}
                      className="flex items-center justify-between rounded-lg border p-4"
                    >
                      <div className="flex items-center gap-3">
                        {conn.platform === "github" ? (
                          <Github className="h-5 w-5" />
                        ) : (
                          <GitlabIcon className="h-5 w-5" />
                        )}
                        <div>
                          <p className="text-sm font-medium">{conn.name}</p>
                          <p className="text-xs text-muted-foreground">
                            {conn.platform} &middot; {conn.token_preview}
                            {conn.base_url && (
                              <span> &middot; {conn.base_url}</span>
                            )}
                          </p>
                        </div>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => handleRemoveConnector(conn.id)}
                        aria-label="Remove connector"
                      >
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    </div>
                  ))}

                  {connectors.length === 0 && (
                    <p className="text-sm text-muted-foreground py-4 text-center">
                      No connections yet. Add one to get started.
                    </p>
                  )}
                </div>
              </div>
            </TabsContent>

            {/* ===== TRACKED PROJECTS TAB ===== */}
            <TabsContent value="projects" className="p-6">
              <div className="space-y-4">
                <SettingHeader
                  title="Tracked Projects"
                  subtitle="Add and manage your tracked repositories"
                  action={
                    <div className="flex items-center gap-2">
                      {projects.length > 0 && (
                        <Dialog>
                          <DialogTrigger asChild>
                            <Button variant="destructive" size="sm" className="gap-2">
                              <Trash2 className="h-4 w-4" /> Clear All Data
                            </Button>
                          </DialogTrigger>
                          <DialogContent>
                            <DialogHeader>
                              <DialogTitle>Clear All Data</DialogTitle>
                              <DialogDescription>
                                This will delete all projects, items, chat history, and settings.
                                You will be redirected to the onboarding screen to start fresh.
                              </DialogDescription>
                            </DialogHeader>
                            <DialogFooter>
                              <DialogTrigger asChild>
                                <Button variant="outline">Cancel</Button>
                              </DialogTrigger>
                              <Button
                                variant="destructive"
                                onClick={handleResetDatabase}
                                disabled={saving === "reset"}
                              >
                                {saving === "reset" ? (
                                  <Loader2 className="h-4 w-4 animate-spin" />
                                ) : (
                                  <AlertTriangle className="h-4 w-4" />
                                )}
                                Clear Everything
                              </Button>
                            </DialogFooter>
                          </DialogContent>
                        </Dialog>
                      )}
                      <Button
                        variant="outline"
                        size="sm"
                        className="gap-2"
                        onClick={() => setAddProjectsOpen(true)}
                      >
                        <Plus className="h-4 w-4" /> Add Projects
                      </Button>
                    </div>
                  }
                />
                <AddProjectsDialog
                  open={addProjectsOpen}
                  onOpenChange={setAddProjectsOpen}
                  connectors={connectors}
                  trackedProjects={projects}
                  onProjectsAdded={async (newProjectIds) => {
                    await fetchProjects();
                    for (const id of newProjectIds) {
                      api.syncProjectItems(id);
                    }
                  }}
                />
                <div className="space-y-2">
                  {projects.map((project) => {
                    const conn = connectorForProject(project.connector_id);
                    return (
                      <div
                        key={project.id}
                        className="rounded-lg border p-3"
                      >
                        <div className="flex items-center justify-between">
                          <div>
                            <p className={`text-sm font-medium${!project.sync_enabled ? " text-muted-foreground" : ""}`}>
                              {project.owner}/{project.name}
                              {!project.sync_enabled && (
                                <span className="ml-2 text-xs text-muted-foreground">(sync paused)</span>
                              )}
                            </p>
                            <p className="text-xs text-muted-foreground flex items-center gap-1">
                              {project.platform}
                              {conn && (
                                <>
                                  <Link className="h-3 w-3" />
                                  {conn.name}
                                </>
                              )}
                            </p>
                            <p className="text-xs text-muted-foreground flex items-center gap-1">
                              {project.last_sync_at
                                ? `Last synced ${formatTimeAgo(project.last_sync_at)}`
                                : "Never synced"}
                              {project.last_sync_error && (
                                <span
                                  className="text-yellow-500 flex items-center gap-1"
                                  title={project.last_sync_error}
                                >
                                  <AlertTriangle className="h-3 w-3" />
                                  {/401|Unauthorized/i.test(project.last_sync_error)
                                    ? "Auth failed"
                                    : "Sync error"}
                                </span>
                              )}
                            </p>
                          </div>
                          <div className="flex items-center gap-1">
                            <Tooltip>
                              <TooltipTrigger asChild>
                                <Button
                                  variant="ghost"
                                  size="icon"
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
                                    <Pause className="h-4 w-4 text-muted-foreground" />
                                  ) : (
                                    <Play className="h-4 w-4 text-muted-foreground" />
                                  )}
                                </Button>
                              </TooltipTrigger>
                              <TooltipContent>{project.sync_enabled ? "Disable sync" : "Enable sync"}</TooltipContent>
                            </Tooltip>
                            <Dialog>
                              <Tooltip>
                                <TooltipTrigger asChild>
                                  <DialogTrigger asChild>
                                    <Button variant="ghost" size="icon">
                                      <FolderX className="h-4 w-4 text-muted-foreground" />
                                    </Button>
                                  </DialogTrigger>
                                </TooltipTrigger>
                                <TooltipContent>Clear project data</TooltipContent>
                              </Tooltip>
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
                                          setItems(items.filter((i) => i.project_id !== project.id));
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
                            <Tooltip>
                              <TooltipTrigger asChild>
                                <Button
                                  variant="ghost"
                                  size="icon"
                                  onClick={() => removeProject(project.id)}
                                >
                                  <Trash2 className="h-4 w-4 text-destructive" />
                                </Button>
                              </TooltipTrigger>
                              <TooltipContent>Remove project</TooltipContent>
                            </Tooltip>
                          </div>
                        </div>
                        {/* Maintainer Notes */}
                        <div className="mt-2 border-t pt-2">
                          <button
                            className="flex w-full items-center justify-between text-xs font-medium text-muted-foreground hover:text-foreground"
                            onClick={() => toggleNotes(project.id)}
                          >
                            <span>Maintainer Notes</span>
                            <ChevronRight className={`h-3 w-3 transition-transform ${expandedNotes.has(project.id) ? "rotate-90" : ""}`} />
                          </button>
                          {expandedNotes.has(project.id) && (
                            <div className="mt-2 space-y-2">
                              {(projectNotes[project.id] || []).map(note => (
                                <div key={note.id} className="flex items-start gap-2 rounded bg-muted/50 px-2 py-1.5 text-xs">
                                  <span className="flex-1">{note.content}</span>
                                  <button
                                    className="shrink-0 text-muted-foreground hover:text-destructive"
                                    onClick={() => handleRemoveNote(note.id, project.id)}
                                  >
                                    <Trash2 className="h-3 w-3" />
                                  </button>
                                </div>
                              ))}
                              {(projectNotes[project.id] || []).length === 0 && (
                                <p className="text-xs text-muted-foreground">No notes yet</p>
                              )}
                              <div className="flex gap-1">
                                <Input
                                  placeholder="Add a note for the AI..."
                                  className="h-7 text-xs"
                                  value={newNoteText[project.id] || ""}
                                  onChange={(e) => setNewNoteText(prev => ({ ...prev, [project.id]: e.target.value }))}
                                  onKeyDown={(e) => {
                                    if (e.key === "Enter") {
                                      e.preventDefault();
                                      handleAddNote(project.id);
                                    }
                                  }}
                                />
                                <Button size="sm" variant="outline" className="h-7 px-2" onClick={() => handleAddNote(project.id)}>
                                  <Plus className="h-3 w-3" />
                                </Button>
                              </div>
                            </div>
                          )}
                        </div>
                        {/* Per-Project AI Preferences */}
                        <div className="mt-2 border-t pt-2">
                          <button
                            className="flex w-full items-center justify-between text-xs font-medium text-muted-foreground hover:text-foreground"
                            onClick={() => toggleProjectAi(project.id)}
                          >
                            <span>AI Preferences Override</span>
                            <ChevronRight className={`h-3 w-3 transition-transform ${expandedProjectAi.has(project.id) ? "rotate-90" : ""}`} />
                          </button>
                          {expandedProjectAi.has(project.id) && aiSettings && (
                            <div className="mt-3">
                              <AIPreferencesForm
                                projectMode
                                focusAreas={(() => {
                                  const raw = getProjectSetting(project.id, "ai_focus_areas");
                                  if (raw) {
                                    try { return JSON.parse(raw); } catch { /* use default */ }
                                  }
                                  return aiSettings.ai_focus_areas;
                                })()}
                                reviewStrictness={
                                  getProjectSetting(project.id, "ai_review_strictness") || aiSettings.ai_review_strictness
                                }
                                responseTone={
                                  getProjectSetting(project.id, "ai_response_tone") || aiSettings.ai_response_tone
                                }
                                overrides={{
                                  focusAreas: !!getProjectSetting(project.id, "ai_focus_areas"),
                                  reviewStrictness: !!getProjectSetting(project.id, "ai_review_strictness"),
                                  responseTone: !!getProjectSetting(project.id, "ai_response_tone"),
                                }}
                                globalDefaults={{
                                  focusAreas: aiSettings.ai_focus_areas,
                                  reviewStrictness: aiSettings.ai_review_strictness,
                                  responseTone: aiSettings.ai_response_tone,
                                }}
                                onOverrideChange={(key, enabled) => {
                                  if (enabled) {
                                    // Set initial project override value from global
                                    if (key === "ai_focus_areas") {
                                      handleProjectAiSave(project.id, key, JSON.stringify(aiSettings.ai_focus_areas));
                                    } else if (key === "ai_review_strictness") {
                                      handleProjectAiSave(project.id, key, aiSettings.ai_review_strictness);
                                    } else if (key === "ai_response_tone") {
                                      handleProjectAiSave(project.id, key, aiSettings.ai_response_tone);
                                    }
                                  } else {
                                    handleProjectAiDelete(project.id, key);
                                  }
                                }}
                                onFocusAreasChange={(areas) => {
                                  handleProjectAiSave(project.id, "ai_focus_areas", JSON.stringify(areas));
                                }}
                                onReviewStrictnessChange={(v) => {
                                  handleProjectAiSave(project.id, "ai_review_strictness", v);
                                }}
                                onResponseToneChange={(v) => {
                                  handleProjectAiSave(project.id, "ai_response_tone", v);
                                }}
                              />
                            </div>
                          )}
                        </div>
                      </div>
                    );
                  })}
                  {projects.length === 0 && (
                    <p className="text-sm text-muted-foreground py-4 text-center">
                      No projects tracked yet
                    </p>
                  )}
                </div>
              </div>
            </TabsContent>


            {/* ===== SYNC TAB ===== */}
            <TabsContent value="sync" className="p-6">
              <div className="space-y-4">
                <SettingHeader
                  title="Sync"
                  subtitle="Configure synchronization and caching settings"
                />
                <div className="space-y-3">
                  <div className="space-y-2">
                    <Label>Refresh Interval</Label>
                    <Select
                      value={String(settings?.refresh_interval || 1800)}
                      onValueChange={(v) => {
                        handleSave("refresh_interval", v);
                        useAppStore.getState().setRefreshInterval(Number(v));
                      }}
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="60">1 minute</SelectItem>
                        <SelectItem value="300">5 minutes</SelectItem>
                        <SelectItem value="600">10 minutes</SelectItem>
                        <SelectItem value="1800">30 minutes</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <Button
                    variant="outline"
                    className="gap-2"
                    onClick={async () => {
                      await api.clearRepoCache();
                      try {
                        const paths = await api.getAppPaths();
                        setAppPaths(paths);
                      } catch {}
                    }}
                  >
                    <FolderX className="h-4 w-4" />
                    Clear Repo Cache
                  </Button>
                  {appPaths && (
                    <p className="text-xs text-muted-foreground">
                      {appPaths.repo_cache_dir} ({formatBytes(appPaths.repo_cache_size_bytes)})
                    </p>
                  )}
                </div>
              </div>
            </TabsContent>

            {/* ===== STORAGE TAB ===== */}
            <TabsContent value="database" className="p-6">
              <div className="space-y-4">
                <SettingHeader
                  title="Storage"
                  subtitle="Manage backups, cache, and storage operations"
                />

                {/* Database Backups */}
                <div className="rounded-lg border p-4 space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <div className="flex items-center gap-2">
                        <Database className="h-4 w-4" />
                        <h3 className="text-sm font-semibold">Database Backups</h3>
                      </div>
                      <p className="text-xs text-muted-foreground mt-1">
                        Create and restore snapshots of your database
                        {appPaths?.database_file && (
                          <span className="ml-1">— {appPaths.database_file}</span>
                        )}
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      onClick={handleCreateBackup}
                      disabled={saving === "backup"}
                      size="sm"
                      className="gap-2"
                    >
                      {saving === "backup" ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <Download className="h-3.5 w-3.5" />
                      )}
                      Create Backup
                    </Button>
                  </div>

                  <div className="space-y-2">
                    {backups.map((backup) => (
                      <div
                        key={backup.filename}
                        className="flex items-center justify-between rounded border p-3"
                      >
                        <div>
                          <p className="text-sm font-medium">
                            {backup.filename}
                          </p>
                          <p className="text-xs text-muted-foreground">
                            {new Date(backup.created_at).toLocaleString()}{" "}
                            &middot;{" "}
                            {(backup.size_bytes / 1024).toFixed(1)} KB
                          </p>
                        </div>
                        <div className="flex gap-1">
                          <Dialog
                            open={confirmRestore === backup.filename}
                            onOpenChange={(open) =>
                              setConfirmRestore(open ? backup.filename : null)
                            }
                          >
                            <DialogTrigger asChild>
                              <Button variant="outline" size="sm" className="gap-1">
                                <RotateCcw className="h-3 w-3" /> Restore
                              </Button>
                            </DialogTrigger>
                            <DialogContent>
                              <DialogHeader>
                                <DialogTitle>Restore Backup</DialogTitle>
                                <DialogDescription>
                                  This will replace the current database with
                                  the backup. This action cannot be undone.
                                </DialogDescription>
                              </DialogHeader>
                              <DialogFooter>
                                <Button
                                  variant="outline"
                                  onClick={() => setConfirmRestore(null)}
                                >
                                  Cancel
                                </Button>
                                <Button
                                  variant="destructive"
                                  onClick={() =>
                                    handleRestoreBackup(backup.filename)
                                  }
                                  disabled={saving === "restore"}
                                >
                                  {saving === "restore" ? (
                                    <Loader2 className="h-4 w-4 animate-spin mr-2" />
                                  ) : null}
                                  Restore
                                </Button>
                              </DialogFooter>
                            </DialogContent>
                          </Dialog>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() =>
                              handleDeleteBackup(backup.filename)
                            }
                            aria-label="Delete backup"
                          >
                            <Trash2 className="h-3 w-3 text-destructive" />
                          </Button>
                        </div>
                      </div>
                    ))}
                    {backups.length === 0 && (
                      <p className="text-sm text-muted-foreground text-center py-2">
                        No backups yet
                      </p>
                    )}
                  </div>
                </div>

                {/* Repository Cache */}
                {appPaths && (
                  <div className="rounded-lg border p-4 space-y-4">
                    <div>
                      <div className="flex items-center gap-2">
                        <FolderGit2 className="h-4 w-4" />
                        <h3 className="text-sm font-semibold">Repository Cache</h3>
                      </div>
                      <p className="text-xs text-muted-foreground mt-1">
                        Cloned repositories for AI analysis are stored locally
                      </p>
                    </div>
                    <div className="flex items-center justify-between">
                      <div className="text-sm">
                        <span className="font-medium">{formatBytes(appPaths.repo_cache_size_bytes)}</span>
                        <span className="text-muted-foreground ml-2">in {appPaths.repo_cache_dir}</span>
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        className="gap-2"
                        onClick={async () => {
                          await api.clearRepoCache();
                          try {
                            const paths = await api.getAppPaths();
                            setAppPaths(paths);
                          } catch {}
                        }}
                      >
                        <FolderX className="h-3.5 w-3.5" />
                        Clear Cache
                      </Button>
                    </div>
                  </div>
                )}

                {/* Danger Zone */}
                <div className="rounded-lg border border-destructive/30 p-4 space-y-4">
                  <div>
                    <div className="flex items-center gap-2 text-destructive">
                      <AlertTriangle className="h-4 w-4" />
                      <h3 className="text-sm font-semibold">Danger Zone</h3>
                    </div>
                    <p className="text-xs text-muted-foreground mt-1">
                      Permanently delete all data and start fresh. You will be redirected to onboarding.
                    </p>
                  </div>
                  <div className="flex justify-end">
                    <Dialog
                      open={confirmReset}
                      onOpenChange={setConfirmReset}
                    >
                      <DialogTrigger asChild>
                        <Button variant="destructive" size="sm" className="gap-2">
                          <AlertTriangle className="h-3.5 w-3.5" /> Reset Everything
                        </Button>
                      </DialogTrigger>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>Reset Everything</DialogTitle>
                        <DialogDescription>
                          This will permanently delete all data including
                          projects, items, chat history, and settings. This
                          action cannot be undone.
                        </DialogDescription>
                      </DialogHeader>
                      <DialogFooter>
                        <Button
                          variant="outline"
                          onClick={() => setConfirmReset(false)}
                        >
                          Cancel
                        </Button>
                        <Button
                          variant="destructive"
                          onClick={handleResetDatabase}
                          disabled={saving === "reset"}
                        >
                          {saving === "reset" ? (
                            <Loader2 className="h-4 w-4 animate-spin mr-2" />
                          ) : null}
                          Reset Everything
                        </Button>
                      </DialogFooter>
                    </DialogContent>
                    </Dialog>
                  </div>
                </div>
              </div>
            </TabsContent>

            {/* ===== LOGS TAB ===== */}
            <TabsContent value="logs" className="p-6">
              <div className="space-y-6">
                <SettingHeader
                  title="Logs"
                  subtitle={appPaths?.log_dir
                    ? `Configure logging and view log entries — ${appPaths.log_dir}`
                    : "Configure logging and view log entries"}
                />
                {/* ---- Log Configuration ---- */}
                <div className="rounded-lg border p-4 space-y-3">
                  <h3 className="text-sm font-semibold">Log Configuration</h3>
                  <p className="text-xs text-muted-foreground">
                    Set the minimum severity that gets written to log files.
                    Changing this takes effect immediately and persists across
                    restarts.
                  </p>
                  <div className="flex items-center gap-3">
                    <Label className="text-xs shrink-0">Minimum Level</Label>
                    <Select
                      value={logLevel}
                      onValueChange={handleSetLogLevel}
                    >
                      <SelectTrigger className="w-36">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="ERROR">Error</SelectItem>
                        <SelectItem value="WARN">Warn</SelectItem>
                        <SelectItem value="INFO">Info</SelectItem>
                        <SelectItem value="DEBUG">Debug</SelectItem>
                        <SelectItem value="TRACE">Trace</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                {/* ---- Log Viewer ---- */}
                <div className="rounded-lg border p-4 space-y-3">
                  <div className="flex items-center justify-between">
                    <h3 className="text-sm font-semibold">Log Viewer</h3>
                    <div className="flex items-center gap-2">
                      <Button
                        variant="outline"
                        size="icon"
                        onClick={() => loadLogs(0)}
                        aria-label="Refresh logs"
                      >
                        <RefreshCw className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleClearLogs}
                        className="gap-1"
                      >
                        <Trash2 className="h-3 w-3" /> Clear
                      </Button>
                    </div>
                  </div>

                  <div className="flex items-center gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs">Show Level</Label>
                      <Select
                        value={displayLevel}
                        onValueChange={(v) => setDisplayLevel(v)}
                      >
                        <SelectTrigger className="w-36">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="ERROR">Error+</SelectItem>
                          <SelectItem value="WARN">Warn+</SelectItem>
                          <SelectItem value="INFO">Info+</SelectItem>
                          <SelectItem value="DEBUG">Debug+</SelectItem>
                          <SelectItem value="TRACE">All</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="flex-1 space-y-1">
                      <Label className="text-xs">Search</Label>
                      <div className="relative">
                        <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                        <Input
                          placeholder="Filter by text..."
                          value={logSearch}
                          onChange={(e) => setLogSearch(e.target.value)}
                          className="pl-9"
                          onKeyDown={(e) => {
                            if (e.key === "Enter") loadLogs(0);
                          }}
                        />
                      </div>
                    </div>
                  </div>

                  <div className="rounded border bg-muted/30 text-xs">
                    {logEntries?.entries.length ? (
                      <table className="w-full font-mono">
                        <thead className="sticky top-0 bg-muted/80 backdrop-blur-sm">
                          <tr className="border-b text-left text-muted-foreground">
                            <th className="w-6 p-2" />
                            <th className="p-2 whitespace-nowrap">Time</th>
                            <th className="p-2 whitespace-nowrap">Level</th>
                            <th className="p-2">Message</th>
                          </tr>
                        </thead>
                        <tbody>
                          {logEntries.entries.map((entry, i) => {
                            const isExpanded = expandedLogIndex === i;
                            return (
                              <Fragment key={`${entry.timestamp}-${i}`}>
                                <tr
                                  className="border-b border-border/50 cursor-pointer hover:bg-muted/50 transition-colors"
                                  onClick={() => setExpandedLogIndex(isExpanded ? null : i)}
                                >
                                  <td className="p-2 text-muted-foreground">
                                    <ChevronRight className={`h-3 w-3 transition-transform ${isExpanded ? "rotate-90" : ""}`} />
                                  </td>
                                  <td className="p-2 text-muted-foreground whitespace-nowrap">
                                    {formatLogTimestamp(entry.timestamp)}
                                  </td>
                                  <td className="p-2">
                                    <span className={`inline-block rounded px-1.5 py-0.5 text-[10px] font-semibold ${levelBadge(entry.level)}`}>
                                      {entry.level}
                                    </span>
                                  </td>
                                  <td className="p-2 truncate max-w-0 w-full">
                                    {entry.message}
                                  </td>
                                </tr>
                                {isExpanded && (
                                  <tr className="border-b border-border/50 bg-muted/20">
                                    <td />
                                    <td colSpan={3} className="p-3 space-y-2">
                                      <div>
                                        <span className="text-muted-foreground">Target: </span>
                                        <span className="text-foreground">{entry.target}</span>
                                      </div>
                                      <div>
                                        <span className="text-muted-foreground">Message: </span>
                                        <pre className="mt-1 whitespace-pre-wrap break-all text-foreground bg-muted/40 rounded p-2">{entry.message}</pre>
                                      </div>
                                      {Object.keys(entry.fields).length > 0 && (
                                        Object.entries(entry.fields).map(([key, value]) => (
                                          <div key={key}>
                                            <span className="text-muted-foreground">{key}: </span>
                                            <span className="text-foreground">{value}</span>
                                          </div>
                                        ))
                                      )}
                                    </td>
                                  </tr>
                                )}
                              </Fragment>
                            );
                          })}
                        </tbody>
                      </table>
                    ) : (
                      <p className="text-center text-muted-foreground py-8">
                        No log entries
                      </p>
                    )}
                  </div>

                  {logEntries?.has_more && (
                    <Button
                      variant="outline"
                      className="w-full"
                      onClick={() => loadLogs(logOffset + 200)}
                    >
                      Load more
                    </Button>
                  )}

                  {logEntries && (
                    <p className="text-xs text-muted-foreground text-center">
                      Showing{" "}
                      {Math.min(logOffset + 200, logEntries.total)} of{" "}
                      {logEntries.total} entries
                    </p>
                  )}
                </div>
              </div>
            </TabsContent>

            {/* ===== AI PROVIDER TAB ===== */}
            <TabsContent value="ai-provider" className="p-6">
              {!aiSettings ? (
                <div className="flex items-center justify-center py-12">
                  <div className="animate-pulse text-muted-foreground">Loading AI settings...</div>
                </div>
              ) : (
                <div className="max-w-3xl space-y-6">
                  <SettingHeader title="AI Provider" subtitle="Choose your AI provider, configure API access, and set model preferences" />
                  <AIProviderSelector
                    mode={aiSettings.ai_mode as "api" | "claude_cli" | "cursor_cli"}
                    hasApiKey={aiSettings.has_ai_api_key}
                    model={aiSettings.ai_model}
                    customInstructions={aiSettings.ai_custom_instructions}
                    onModeChange={(mode) => handleAiSave("ai_mode", mode)}
                    onApiKeyChange={(key) => handleAiSave("ai_api_key", key)}
                    onModelChange={(model) => handleAiSave("ai_model", model)}
                    onCustomInstructionsChange={(instructions) => handleAiSave("ai_custom_instructions", instructions)}
                  />
                </div>
              )}
            </TabsContent>

            {/* ===== AI PREFERENCES TAB ===== */}
            <TabsContent value="ai-preferences" className="p-6">
              {!aiSettings ? (
                <div className="flex items-center justify-center py-12">
                  <div className="animate-pulse text-muted-foreground">Loading AI settings...</div>
                </div>
              ) : (
                <div className="max-w-2xl space-y-6">
                  <SettingHeader title="AI Preferences" subtitle="Configure analysis focus areas, review strictness, and response tone" />
                  <AIPreferencesForm
                    focusAreas={aiSettings.ai_focus_areas}
                    reviewStrictness={aiSettings.ai_review_strictness}
                    responseTone={aiSettings.ai_response_tone}
                    onFocusAreasChange={(areas) => handleAiSave("ai_focus_areas", JSON.stringify(areas))}
                    onReviewStrictnessChange={(v) => handleAiSave("ai_review_strictness", v)}
                    onResponseToneChange={(v) => handleAiSave("ai_response_tone", v)}
                  />
                </div>
              )}
            </TabsContent>
          </ScrollArea>
        </div>
      </Tabs>
    </div>
  );
}

