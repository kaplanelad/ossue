import { useState, useEffect, useCallback, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import * as api from "@/lib/tauri";
import type { Connector, ConnectorRepo, Platform, Project } from "@/types";
import { Loader2, Check, Search, Github, GitlabIcon, Star } from "lucide-react";

const PAGE_SIZE = 50;

export interface RepoWithConnector extends ConnectorRepo {
  connectorId: string;
  connectorName: string;
  platform: Platform;
}

interface RepoBrowserProps {
  connectors: Connector[];
  trackedProjects?: Project[];
  selectedRepos: Set<string>;
  onSelectionChange: (selected: Set<string>) => void;
  /** Fires whenever the loaded repos list changes, so the parent can look up full repo data. */
  onReposLoaded?: (repos: RepoWithConnector[]) => void;
  /** When true, shows a dropdown to pick a single account before loading repos. */
  showAccountSelector?: boolean;
}

export function RepoBrowser({
  connectors,
  trackedProjects,
  selectedRepos,
  onSelectionChange,
  onReposLoaded,
  showAccountSelector = false,
}: RepoBrowserProps) {
  const [repos, setRepos] = useState<RepoWithConnector[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [search, setSearch] = useState("");
  const [filterConnectorId, setFilterConnectorId] = useState("");
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);

  const normalizeUrl = (url: string) => url.toLowerCase().replace(/\/+$/, "");

  const isTracked = useCallback(
    (repoUrl: string) => {
      if (!trackedProjects) return false;
      return trackedProjects.some(
        (p) => normalizeUrl(p.url) === normalizeUrl(repoUrl)
      );
    },
    [trackedProjects]
  );

  const fetchRepos = useCallback(
    async (connectorIds: string[]) => {
      setIsLoading(true);
      try {
        const allRepos: RepoWithConnector[] = [];
        for (const connId of connectorIds) {
          const connector = connectors.find((c) => c.id === connId);
          if (!connector) continue;
          try {
            const connRepos = await api.listConnectorRepos(connector.id);
            allRepos.push(
              ...connRepos.map((r) => ({
                ...r,
                connectorId: connector.id,
                connectorName: connector.name,
                platform: connector.platform,
              }))
            );
          } catch (err) {
            console.error(
              `Failed to fetch repos for ${connector.name}:`,
              err
            );
          }
        }
        setRepos(allRepos);
        onReposLoaded?.(allRepos);
      } finally {
        setIsLoading(false);
      }
    },
    [connectors, onReposLoaded]
  );

  // Auto-select if there's exactly one connector
  useEffect(() => {
    if (showAccountSelector && connectors.length === 1 && !filterConnectorId) {
      setFilterConnectorId(connectors[0].id);
    }
  }, [showAccountSelector, connectors, filterConnectorId]);

  // Load repos based on mode
  useEffect(() => {
    if (showAccountSelector) {
      if (!filterConnectorId) {
        setRepos([]);
        onReposLoaded?.([]);
        return;
      }
      fetchRepos([filterConnectorId]);
    } else {
      if (connectors.length === 0) {
        setRepos([]);
        onReposLoaded?.([]);
        return;
      }
      fetchRepos(connectors.map((c) => c.id));
    }
  }, [showAccountSelector, filterConnectorId, connectors, fetchRepos, onReposLoaded]);

  // Reset pagination when search or filter changes
  useEffect(() => {
    setVisibleCount(PAGE_SIZE);
  }, [search, filterConnectorId]);

  const toggleRepo = (repoUrl: string) => {
    if (isTracked(repoUrl)) return;
    const next = new Set(selectedRepos);
    if (next.has(repoUrl)) {
      next.delete(repoUrl);
    } else {
      next.add(repoUrl);
    }
    onSelectionChange(next);
  };

  const filtered = useMemo(
    () =>
      repos.filter((r) =>
        r.full_name.toLowerCase().includes(search.toLowerCase())
      ),
    [repos, search]
  );

  const visible = filtered.slice(0, visibleCount);
  const hasMore = visibleCount < filtered.length;
  const remaining = filtered.length - visibleCount;

  const showRepoList = showAccountSelector ? !!filterConnectorId : true;

  return (
    <div className="space-y-3">
      {/* Account selector (settings mode only) */}
      {showAccountSelector && (
        <Select value={filterConnectorId} onValueChange={setFilterConnectorId}>
          <SelectTrigger>
            <SelectValue placeholder="Select an account..." />
          </SelectTrigger>
          <SelectContent>
            {connectors.map((c) => (
              <SelectItem key={c.id} value={c.id}>
                <span className="flex items-center gap-2">
                  {c.platform === "github" ? (
                    <Github className="h-3 w-3" />
                  ) : (
                    <GitlabIcon className="h-3 w-3" />
                  )}
                  {c.name}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}

      {showRepoList ? (
        <>
          {/* Search */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search repos..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-9"
              autoComplete="off"
            />
          </div>

          {/* Repo list */}
          {isLoading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin" />
            </div>
          ) : (
            <ScrollArea className="h-80 rounded-md border">
              <div className="flex flex-col">
                {visible.map((repo) => {
                  const tracked = isTracked(repo.url);
                  const selected = selectedRepos.has(repo.url);
                  return (
                    <button
                      key={`${repo.connectorId}-${repo.full_name}`}
                      className={`flex items-start gap-3 px-4 py-2 text-left hover:bg-muted/50 ${
                        selected ? "bg-muted" : ""
                      } ${tracked ? "opacity-50 cursor-default" : ""}`}
                      onClick={() => toggleRepo(repo.url)}
                      disabled={tracked}
                    >
                      <div
                        className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded border ${
                          tracked
                            ? "border-muted bg-muted"
                            : selected
                              ? "border-primary bg-primary text-primary-foreground"
                              : "border-input"
                        }`}
                      >
                        {(selected || tracked) && (
                          <Check className="h-3 w-3" />
                        )}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <p className="text-sm font-medium break-all flex-1">
                            {repo.full_name}
                          </p>
                          {tracked && (
                            <Badge
                              variant="secondary"
                              className="shrink-0 text-[10px] px-1.5 py-0"
                            >
                              tracked
                            </Badge>
                          )}
                          {repo.stars != null && (
                            <span className="shrink-0 flex items-center gap-1 text-xs text-muted-foreground">
                              <Star className="h-3 w-3" />
                              {repo.stars.toLocaleString()}
                            </span>
                          )}
                        </div>
                        {repo.description && (
                          <p className="text-xs text-muted-foreground break-words">
                            {repo.description}
                          </p>
                        )}
                      </div>
                    </button>
                  );
                })}
                {hasMore && (
                  <Button
                    variant="ghost"
                    className="w-full py-2 text-sm"
                    onClick={() => setVisibleCount((c) => c + PAGE_SIZE)}
                  >
                    Show {Math.min(remaining, PAGE_SIZE)} more
                    {remaining > PAGE_SIZE ? ` of ${remaining} remaining` : ""}
                  </Button>
                )}
                {filtered.length === 0 && (
                  <p className="text-sm text-muted-foreground py-8 text-center">
                    {search
                      ? "No repos matching your search"
                      : "No repos found"}
                  </p>
                )}
              </div>
            </ScrollArea>
          )}

          {/* Summary line */}
          {repos.length > 0 && (
            <p className="text-xs text-muted-foreground text-right">
              {filtered.length} of {repos.length} repos
              {search ? " matching search" : ""}
            </p>
          )}
        </>
      ) : (
        <p className="text-sm text-muted-foreground py-8 text-center">
          Select an account to browse repositories
        </p>
      )}
    </div>
  );
}
