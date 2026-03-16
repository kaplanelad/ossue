import { useState, useCallback, useEffect } from "react";
import { errorMessage } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import * as api from "@/lib/tauri";
import type { Connector, Project } from "@/types";
import { Loader2, Plus, ArrowRight, ArrowLeft, X } from "lucide-react";
import { RepoBrowser, type RepoWithConnector } from "@/components/shared/RepoBrowser";
import { SyncFilters } from "@/components/shared/SyncFilters";

interface AddedRepo {
  id: string;
  name: string;
  owner: string;
  url: string;
  platform: "github" | "gitlab";
}

type View = "select" | "sync";

interface RepoPickerProps {
  connectors: Connector[];
  onDone: () => void;
}

export function RepoPicker({ connectors, onDone }: RepoPickerProps) {
  const [view, setView] = useState<View>("select");
  const [selectedRepos, setSelectedRepos] = useState<Set<string>>(new Set());
  const [loadedRepos, setLoadedRepos] = useState<RepoWithConnector[]>([]);
  const [manualUrl, setManualUrl] = useState("");
  const [addingUrl, setAddingUrl] = useState(false);
  const [addedRepos, setAddedRepos] = useState<AddedRepo[]>([]);
  const [urlError, setUrlError] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [isCreating, setIsCreating] = useState(false);

  // On mount, load existing projects from DB to restore state when navigating back
  useEffect(() => {
    async function loadExistingProjects() {
      try {
        const existing = await api.listProjects();
        if (existing.length > 0) {
          setSelectedRepos(new Set(existing.map((p) => p.url)));
        }
      } catch {
        // ignore
      }
    }
    loadExistingProjects();
  }, []);

  const handleReposLoaded = useCallback((repos: RepoWithConnector[]) => {
    setLoadedRepos(repos);
  }, []);

  const handleAddUrl = async () => {
    if (!manualUrl.trim()) return;
    setAddingUrl(true);
    setUrlError(null);
    try {
      let connectorId: string | undefined;
      if (manualUrl.includes("github")) {
        connectorId = connectors.find((c) => c.platform === "github")?.id;
      } else if (manualUrl.includes("gitlab")) {
        connectorId = connectors.find((c) => c.platform === "gitlab")?.id;
      }
      const project = await api.addProjectByUrl(manualUrl, connectorId);
      setAddedRepos((prev) => [
        ...prev,
        {
          id: project.id,
          name: project.name,
          owner: project.owner,
          url: project.url,
          platform: project.platform,
        },
      ]);
      setManualUrl("");
    } catch (err) {
      setUrlError(errorMessage(err));
    } finally {
      setAddingUrl(false);
    }
  };

  const removeAddedRepo = (index: number) => {
    setAddedRepos((prev) => prev.filter((_, i) => i !== index));
  };

  const syncProjects = async () => {
    const existing = await api.listProjects();
    const existingUrls = new Set(existing.map((p) => p.url));
    const addedUrls = new Set(addedRepos.map((r) => r.url));
    const wantedUrls = new Set([...selectedRepos, ...addedUrls]);

    // Remove projects that were deselected
    for (const proj of existing) {
      if (!wantedUrls.has(proj.url)) {
        try {
          await api.removeProject(proj.id);
        } catch (err) {
          console.error("Failed to remove repo:", err);
        }
      }
    }

    // Add projects that are newly selected
    for (const repoUrl of selectedRepos) {
      if (existingUrls.has(repoUrl)) continue;
      const repo = loadedRepos.find((r) => r.url === repoUrl);
      if (!repo) continue;
      try {
        await api.addProject({
          name: repo.name,
          owner: repo.owner,
          platform: repo.platform,
          url: repo.url,
          connector_id: repo.connectorId,
        });
      } catch (err) {
        console.error("Failed to add repo:", err);
      }
    }
  };

  const handleShowSyncSettings = async () => {
    setIsCreating(true);
    try {
      await syncProjects();
      const allProjects = await api.listProjects();
      const addedUrls = new Set(addedRepos.map((r) => r.url));
      const relevantProjects = allProjects.filter(
        (p) => selectedRepos.has(p.url) || addedUrls.has(p.url)
      );
      setProjects(relevantProjects);
      setView("sync");
    } finally {
      setIsCreating(false);
    }
  };

  const handleContinue = async () => {
    await syncProjects();
    onDone();
  };

  const totalSelected = selectedRepos.size + addedRepos.length;

  if (view === "sync") {
    return (
      <div className="space-y-4">
        <div className="space-y-3">
          {projects.map((project) => (
            <div
              key={project.id}
              className="rounded-lg p-3 space-y-2"
              style={{
                background: "rgba(255,255,255,0.03)",
                border: "1px solid rgba(255,255,255,0.08)",
              }}
            >
              <span className="text-sm font-medium">
                {project.owner}/{project.name}
              </span>
              <SyncFilters
                projectId={project.id}
                platform={project.platform}
                compact
              />
            </div>
          ))}
        </div>

        <div className="flex justify-between">
          <Button
            variant="ghost"
            size="sm"
            className="gap-1.5"
            onClick={() => setView("select")}
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            Back
          </Button>
          <Button onClick={onDone} className="gap-2">
            Continue <ArrowRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <RepoBrowser
        connectors={connectors}
        selectedRepos={selectedRepos}
        onSelectionChange={setSelectedRepos}
        onReposLoaded={handleReposLoaded}
      />

      <div className="space-y-2">
        <div className="flex gap-2">
          <Input
            className="min-w-0"
            placeholder="Or paste a repo URL..."
            value={manualUrl}
            onChange={(e) => {
              setManualUrl(e.target.value);
              setUrlError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleAddUrl();
            }}
          />
          <Button
            variant="outline"
            size="icon"
            className="shrink-0"
            onClick={handleAddUrl}
            disabled={!manualUrl.trim() || addingUrl}
          >
            {addingUrl ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Plus className="h-4 w-4" />
            )}
          </Button>
        </div>
        {urlError && (
          <p className="text-sm text-destructive">{urlError}</p>
        )}
      </div>

      {addedRepos.length > 0 && (
        <div className="space-y-1">
          {addedRepos.map((repo, index) => (
            <div
              key={`${repo.owner}/${repo.name}`}
              className="rounded-md border px-3 py-2"
            >
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium break-all">
                  {repo.owner}/{repo.name}
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6 shrink-0"
                  onClick={() => removeAddedRepo(index)}
                >
                  <X className="h-3 w-3" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="flex justify-between items-center">
        <Badge variant="secondary">{totalSelected} selected</Badge>
        <div className="flex items-center gap-2">
          {totalSelected > 0 && (
            <button
              type="button"
              className="text-xs text-muted-foreground hover:text-foreground disabled:opacity-50"
              onClick={handleShowSyncSettings}
              disabled={isCreating}
            >
              Sync settings
            </button>
          )}
          <Button onClick={handleContinue} className="gap-2">
            Continue <ArrowRight className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
