import { useState, useEffect, useCallback } from "react";
import { errorMessage } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { toast } from "sonner";
import * as api from "@/lib/tauri";
import type { Connector, Project } from "@/types";
import { Loader2, Plus } from "lucide-react";
import { RepoBrowser, type RepoWithConnector } from "@/components/shared/RepoBrowser";

interface AddProjectsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  connectors: Connector[];
  trackedProjects: Project[];
  onProjectsAdded: (newProjectIds: string[]) => void;
}

export function AddProjectsDialog({
  open,
  onOpenChange,
  connectors,
  trackedProjects,
  onProjectsAdded,
}: AddProjectsDialogProps) {
  const [selectedRepos, setSelectedRepos] = useState<Set<string>>(new Set());
  const [loadedRepos, setLoadedRepos] = useState<RepoWithConnector[]>([]);
  const [urlInput, setUrlInput] = useState("");
  const [addingUrl, setAddingUrl] = useState(false);
  const [addingSelected, setAddingSelected] = useState(false);

  // Reset all state when dialog closes
  useEffect(() => {
    if (!open) {
      setSelectedRepos(new Set());
      setLoadedRepos([]);
      setUrlInput("");
    }
  }, [open]);

  const handleReposLoaded = useCallback((repos: RepoWithConnector[]) => {
    setLoadedRepos(repos);
  }, []);

  const handleAddSelected = async () => {
    if (selectedRepos.size === 0) return;
    setAddingSelected(true);
    try {
      const newIds: string[] = [];
      for (const repoUrl of selectedRepos) {
        const repo = loadedRepos.find((r) => r.url === repoUrl);
        if (!repo) continue;
        const project = await api.addProject({
          name: repo.name,
          owner: repo.owner,
          platform: repo.platform,
          url: repo.url,
          connector_id: repo.connectorId,
        });
        newIds.push(project.id);
      }
      onProjectsAdded(newIds);
      onOpenChange(false);
    } catch (err) {
      toast.error("Failed to add projects", { description: errorMessage(err) });
    } finally {
      setAddingSelected(false);
    }
  };

  const handleAddByUrl = async () => {
    if (!urlInput.trim()) return;
    setAddingUrl(true);
    try {
      let connectorId: string | undefined;
      if (urlInput.includes("github")) {
        connectorId = connectors.find((c) => c.platform === "github")?.id;
      } else if (urlInput.includes("gitlab")) {
        connectorId = connectors.find((c) => c.platform === "gitlab")?.id;
      }
      const project = await api.addProjectByUrl(urlInput, connectorId);
      onProjectsAdded([project.id]);
      setUrlInput("");
    } catch (err) {
      toast.error("Failed to add project", { description: errorMessage(err) });
    } finally {
      setAddingUrl(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:!max-w-2xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>Add Projects</DialogTitle>
          <DialogDescription>
            Browse repos from your accounts or paste a URL
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0 overflow-y-auto space-y-4">
          {connectors.length === 0 ? (
            <p className="text-sm text-muted-foreground py-4 text-center">
              No connected accounts. Add one in the Accounts tab to browse
              repositories.
            </p>
          ) : (
            <RepoBrowser
              connectors={connectors}
              trackedProjects={trackedProjects}
              selectedRepos={selectedRepos}
              onSelectionChange={setSelectedRepos}
              onReposLoaded={handleReposLoaded}
              showAccountSelector
            />
          )}

          <Separator />
          <div className="space-y-2">
            <p className="text-xs text-muted-foreground">Or add by URL</p>
            <div className="flex gap-2">
              <Input
                className="min-w-0"
                placeholder="https://github.com/owner/repo"
                value={urlInput}
                onChange={(e) => setUrlInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleAddByUrl();
                }}
              />
              <Button
                variant="outline"
                size="icon"
                className="shrink-0"
                onClick={handleAddByUrl}
                disabled={!urlInput.trim() || addingUrl}
                aria-label="Add project"
              >
                {addingUrl ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Plus className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>
        </div>

        <DialogFooter>
          <div className="flex w-full items-center justify-between">
            <Badge variant="secondary">
              {selectedRepos.size} selected
            </Badge>
            <Button
              onClick={handleAddSelected}
              disabled={selectedRepos.size === 0 || addingSelected}
            >
              {addingSelected && (
                <Loader2 className="h-4 w-4 animate-spin mr-2" />
              )}
              Add Selected
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
