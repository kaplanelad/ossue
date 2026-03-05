import { useState, useCallback } from "react";
import { errorMessage } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import * as api from "@/lib/tauri";
import type { Connector } from "@/types";
import { Loader2, Plus, ArrowRight, X } from "lucide-react";
import { RepoBrowser, type RepoWithConnector } from "@/components/shared/RepoBrowser";

interface AddedRepo {
  name: string;
  owner: string;
  url: string;
}

interface RepoPickerProps {
  connectors: Connector[];
  onDone: () => void;
}

export function RepoPicker({ connectors, onDone }: RepoPickerProps) {
  const [selectedRepos, setSelectedRepos] = useState<Set<string>>(new Set());
  const [loadedRepos, setLoadedRepos] = useState<RepoWithConnector[]>([]);
  const [manualUrl, setManualUrl] = useState("");
  const [addingUrl, setAddingUrl] = useState(false);
  const [addedRepos, setAddedRepos] = useState<AddedRepo[]>([]);
  const [urlError, setUrlError] = useState<string | null>(null);

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
        { name: project.name, owner: project.owner, url: project.url },
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

  const handleDone = async () => {
    for (const repoUrl of selectedRepos) {
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
    onDone();
  };

  const totalSelected = selectedRepos.size + addedRepos.length;

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
              className="flex items-center justify-between rounded-md border px-3 py-2"
            >
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
          ))}
        </div>
      )}

      <div className="flex justify-between">
        <Badge variant="secondary">{totalSelected} selected</Badge>
        <Button onClick={handleDone} className="gap-2">
          Continue <ArrowRight className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
