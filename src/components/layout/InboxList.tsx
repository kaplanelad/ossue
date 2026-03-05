import { useMemo, useRef, useEffect, useCallback, useState } from "react";
import { errorMessage } from "@/lib/utils";
import { useItems } from "@/hooks/useItems";
import { useAppStore } from "@/stores/appStore";
import { useDraftIssueStore } from "@/stores/draftIssueStore";
import { InboxItem } from "@/components/inbox/InboxItem";
import { NoteItem } from "@/components/notes/NoteItem";
import { BulkActionBar } from "@/components/inbox/BulkActionBar";
import { NoteBulkActionBar } from "@/components/notes/NoteBulkActionBar";
import { CreateNoteDialog } from "@/components/notes/CreateNoteDialog";
import { SyncProgressBar } from "@/components/inbox/SyncProgressBar";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import {
  MoreVertical, RefreshCw, RotateCw, RotateCcw, Loader2, Plus, StickyNote, Search, X,
  Star, EyeOff, Sparkles, PauseCircle, Inbox, SearchX, CircleDot, GitPullRequest, MessageCircle,
} from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu";
import { toast } from "sonner";
import * as api from "@/lib/tauri";
import type { AnalysisAction } from "@/types";
import { EmptyState } from "@/components/shared/EmptyState";

export function InboxList() {
  const { items, syncItems, isSyncing, syncDisabled, setSelectedItemId, selectedItemId, markRead, markUnread, deleteItem, restoreItem, fullSync } = useItems();
  const { selectedProjectIds, projects, syncingProjects, activeAnalyses, selectedItemIds, toggleItemSelection, clearSelection, startAnalysis, clearAnalysis, analyzedItemIds, showAnalyzedOnly, showStarredOnly, showDismissedOnly, updateItem, itemTypeFilter, refreshInbox, hasMore, isLoadingMore, fetchMore, searchQuery, setSearchQuery } = useAppStore();
  const {
    selectedNoteId,
    setSelectedNoteId,
    selectedNoteIds,
    toggleNoteSelection,
    clearNoteSelection,
    isGenerating,
    setIsGenerating,
    openCreateNote,
    openEditNote,
  } = useDraftIssueStore();

  const [localSearch, setLocalSearch] = useState(searchQuery);
  const [isSearchOpen, setIsSearchOpen] = useState(searchQuery.length > 0);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const handleSearchChange = useCallback(
    (value: string) => {
      setLocalSearch(value);
      clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        setSearchQuery(value);
      }, 300);
    },
    [setSearchQuery]
  );

  const handleClearSearch = useCallback(() => {
    setLocalSearch("");
    setSearchQuery("");
    setIsSearchOpen(false);
  }, [setSearchQuery]);

  const handleOpenSearch = useCallback(() => {
    setIsSearchOpen(true);
    setTimeout(() => searchInputRef.current?.focus(), 0);
  }, []);

  useEffect(() => {
    return () => clearTimeout(debounceRef.current);
  }, []);

  const syncEntries = Object.entries(syncingProjects);
  const analysisEntries: [string, string | null][] = Object.entries(activeAnalyses)
    .filter(([, state]) => state.isStreaming)
    .map(([id, state]) => [id, state.status]);

  const projectMap = useMemo(
    () => new Map(projects.map((p) => [p.id, p])),
    [projects]
  );

  const filteredItems = useMemo(() => {
    let result = items;
    if (selectedProjectIds.length > 0) {
      const projectIdSet = new Set(selectedProjectIds);
      result = result.filter((item) => projectIdSet.has(item.project_id));
    }
    if (itemTypeFilter !== "all") {
      result = result.filter((item) => item.item_type === itemTypeFilter);
    }
    if (showAnalyzedOnly) {
      result = result.filter((item) => analyzedItemIds.has(item.id) || (item.item_type === "note" && item.type_data.kind === "note" && item.type_data.draft_status === "ready"));
    }
    if (showStarredOnly) {
      result = result.filter((item) => item.is_starred);
    }
    return result;
  }, [items, selectedProjectIds, itemTypeFilter, showAnalyzedOnly, analyzedItemIds, showStarredOnly]);

  const selectedIdSet = useMemo(
    () => new Set(selectedItemIds),
    [selectedItemIds]
  );

  const selectedItems = useMemo(
    () => filteredItems.filter((item) => item.item_type !== "note" && selectedIdSet.has(item.id)),
    [filteredItems, selectedIdSet]
  );

  const selectedNoteIdSet = useMemo(
    () => new Set(selectedNoteIds),
    [selectedNoteIds]
  );

  const selectedNotes = useMemo(
    () => filteredItems.filter((i) => i.item_type === "note" && selectedNoteIdSet.has(i.id)),
    [filteredItems, selectedNoteIdSet]
  );

  const handleItemClick = async (id: string) => {
    setSelectedNoteId(null);
    clearNoteSelection();
    setSelectedItemId(id);
    await markRead(id);
  };

  const handleToggleStar = async (item: { id: string; is_starred: boolean }) => {
    const newStarred = !item.is_starred;
    updateItem(item.id, { is_starred: newStarred });
    try {
      await api.toggleItemStar(item.id, newStarred);
    } catch (err) {
      updateItem(item.id, { is_starred: item.is_starred });
      toast.error("Failed to update star", { description: errorMessage(err) });
    }
  };

  const handleBulkMarkRead = async () => {
    await Promise.all(selectedItemIds.map((id) => markRead(id)));
    clearSelection();
  };

  const handleBulkMarkUnread = async () => {
    await Promise.all(selectedItemIds.map((id) => markUnread(id)));
    clearSelection();
  };

  const handleBulkDelete = async () => {
    await Promise.all(selectedItemIds.map((id) => deleteItem(id)));
    clearSelection();
  };

  const handleBulkRestore = async () => {
    await Promise.all(selectedItemIds.map((id) => restoreItem(id)));
    clearSelection();
  };

  const handleBulkAiAction = async (action: AnalysisAction) => {
    const itemsToProcess = selectedItems.filter(
      (item) => !activeAnalyses[item.id]?.isStreaming
    );
    clearSelection();

    for (const item of itemsToProcess) {
      startAnalysis(item.id);
    }

    const CONCURRENCY = 5;
    const queue = [...itemsToProcess];
    const workers = Array.from({ length: Math.min(CONCURRENCY, queue.length) }, async () => {
      while (queue.length > 0) {
        const item = queue.shift();
        if (!item) break;
        try {
          await api.analyzeItemAction({ item_id: item.id, action });
        } catch (err) {
          clearAnalysis(item.id);
          if (errorMessage(err) !== "Already analyzed") {
            toast.error(`AI failed on "${item.title}"`, { description: errorMessage(err) });
          }
        }
      }
    });
    await Promise.all(workers);
  };

  const handleNoteToggleStar = async (note: { id: string; is_starred: boolean }) => {
    const newStarred = !note.is_starred;
    updateItem(note.id, { is_starred: newStarred });
    try {
      await api.toggleDraftIssueStar(note.id, newStarred);
    } catch (err) {
      updateItem(note.id, { is_starred: note.is_starred });
      toast.error("Failed to update star", { description: errorMessage(err) });
    }
  };

  const handleNoteClick = (id: string) => {
    setSelectedItemId(null);
    clearSelection();
    setSelectedNoteId(id);
  };

  const handleNoteGenerate = async (noteId: string) => {
    setIsGenerating(noteId, true);
    try {
      await api.generateIssueFromDraft(noteId);
      await refreshInbox();
    } catch (err) {
      toast.error("Generation failed", { description: errorMessage(err) });
    } finally {
      setIsGenerating(noteId, false);
    }
  };

  const handleNoteDelete = async (noteId: string) => {
    if (selectedNoteId === noteId) setSelectedNoteId(null);
    try {
      await api.deleteDraftIssue(noteId);
      await refreshInbox();
    } catch (err) {
      toast.error("Failed to delete", { description: errorMessage(err) });
    }
  };

  const handleBulkNoteGenerate = async () => {
    const drafts = selectedNotes.filter((n) => n.type_data.kind === "note" && n.type_data.draft_status === "draft");
    clearNoteSelection();

    for (const note of drafts) {
      setIsGenerating(note.id, true);
    }

    for (const note of drafts) {
      try {
        await api.generateIssueFromDraft(note.id);
      } catch (err) {
        toast.error(`Generation failed for note`, { description: errorMessage(err) });
      } finally {
        setIsGenerating(note.id, false);
      }
    }
    await refreshInbox();
  };

  const handleBulkNoteDelete = async () => {
    const ids = [...selectedNoteIds];
    clearNoteSelection();
    if (selectedNoteId && ids.includes(selectedNoteId)) {
      setSelectedNoteId(null);
    }
    try {
      await Promise.all(ids.map((id) => api.deleteDraftIssue(id)));
      await refreshInbox();
    } catch (err) {
      toast.error("Failed to delete notes", { description: errorMessage(err) });
    }
  };

  const handleCreateNote = () => {
    if (selectedProjectIds.length === 1) {
      openCreateNote(selectedProjectIds[0]);
    } else {
      openCreateNote();
    }
  };

  const sentinelRef = useRef<HTMLDivElement>(null);

  const handleFetchMore = useCallback(() => {
    if (hasMore && !isLoadingMore) {
      fetchMore();
    }
  }, [hasMore, isLoadingMore, fetchMore]);

  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          handleFetchMore();
        }
      },
      { threshold: 0 }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [handleFetchMore]);

  // Determine header title
  const headerTitle = showDismissedOnly
    ? "Dismissed"
    : itemTypeFilter === "note"
      ? "Notes"
      : selectedProjectIds.length === 0
        ? "All Items"
        : selectedProjectIds.length === 1
          ? projects.find((p) => p.id === selectedProjectIds[0])?.name ?? "Inbox"
          : `${selectedProjectIds.length} Projects`;

  const isNotesOnly = itemTypeFilter === "note";

  function renderEmptyState() {
    if (searchQuery) {
      return (
        <EmptyState
          key="search"
          icon={SearchX}
          title="No results found"
          description={`No items match "${searchQuery}"`}
        />
      );
    }
    if (isNotesOnly) {
      return (
        <EmptyState
          key="notes"
          icon={StickyNote}
          iconClassName="text-amber-500/60 dark:text-amber-400/60"
          iconContainerClassName="bg-amber-500/10 dark:bg-amber-400/10"
          title="No notes yet"
          description="Create a note to capture ideas. You can generate structured issues from them later."
          action={{ label: "Create Note", onClick: handleCreateNote, icon: Plus }}
        />
      );
    }
    if (showDismissedOnly) {
      return (
        <EmptyState
          key="dismissed"
          icon={EyeOff}
          title="No dismissed items"
          description="Items you dismiss will appear here."
        />
      );
    }
    if (showStarredOnly) {
      return (
        <EmptyState
          key="starred"
          icon={Star}
          iconClassName="text-yellow-500/60 dark:text-yellow-400/60"
          iconContainerClassName="bg-yellow-500/10 dark:bg-yellow-400/10"
          title="No starred items"
          description="Click the star icon on items to mark them as favorites."
        />
      );
    }
    if (showAnalyzedOnly) {
      return (
        <EmptyState
          key="analyzed"
          icon={Sparkles}
          iconClassName="text-purple-500/60 dark:text-purple-400/60"
          iconContainerClassName="bg-purple-500/10 dark:bg-purple-400/10"
          title="No analyzed items"
          description="None of the current items have AI chat history."
        />
      );
    }
    if (itemTypeFilter === "issue") {
      return (
        <EmptyState
          key="issue"
          icon={CircleDot}
          title="No issues"
          description="No issues match the current filters."
        />
      );
    }
    if (itemTypeFilter === "pr") {
      return (
        <EmptyState
          key="pr"
          icon={GitPullRequest}
          title="No pull requests"
          description="No pull requests match the current filters."
        />
      );
    }
    if (itemTypeFilter === "discussion") {
      return (
        <EmptyState
          key="discussion"
          icon={MessageCircle}
          title="No discussions"
          description="No discussions match the current filters."
        />
      );
    }
    if (selectedProjectIds.length > 0) {
      if (syncDisabled) {
        return (
          <EmptyState
            key="sync-paused"
            icon={PauseCircle}
            title="Sync is paused for this project"
          />
        );
      }
      return (
        <EmptyState
          key="sync-enabled"
          icon={Inbox}
          title="No items yet"
          description="Items will appear here after syncing."
          action={{ label: "Sync now", onClick: syncItems }}
        />
      );
    }
    return (
      <EmptyState
        key="fallback"
        icon={Inbox}
        title="No items yet"
        description="Select a project and sync to see items here."
      />
    );
  }

  return (
    <div className="flex h-full min-w-0 flex-1 flex-col overflow-hidden border-r">
      <div className="flex h-14 shrink-0 items-center justify-between border-b px-4 gap-2">
        {isSearchOpen ? (
          <div className="relative flex min-w-0 flex-1 items-center">
            <Search className="absolute left-2.5 h-3.5 w-3.5 text-muted-foreground" />
            <Input
              ref={searchInputRef}
              value={localSearch}
              onChange={(e) => handleSearchChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") handleClearSearch();
              }}
              placeholder="Search items..."
              className="h-8 pl-8 pr-8 text-sm"
            />
            <button
              onClick={handleClearSearch}
              className="absolute right-2 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        ) : (
          <>
            <h2 className="min-w-0 truncate text-base font-bold">{headerTitle}</h2>
            <div className="flex-1" />
          </>
        )}
        <div className="flex shrink-0 items-center gap-1">
          {!isSearchOpen && (
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={handleOpenSearch}
              aria-label="Search"
            >
              <Search className="h-4 w-4" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={handleCreateNote}
            aria-label="Create note"
          >
            <Plus className="h-4 w-4" />
          </Button>
          {isNotesOnly ? (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="More options">
                  <MoreVertical className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={() => window.location.reload()}>
                  <RotateCw className="h-4 w-4" />
                  Reload page
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => refreshInbox()}>
                  <RefreshCw className="h-4 w-4" />
                  Refresh
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  disabled={isSyncing || selectedProjectIds.length > 1}
                  aria-label="Sync options"
                >
                  {isSyncing ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <MoreVertical className="h-4 w-4" />
                  )}
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={() => window.location.reload()}>
                  <RotateCw className="h-4 w-4" />
                  Reload page
                </DropdownMenuItem>
                <DropdownMenuItem onClick={syncItems} disabled={isSyncing}>
                  <RefreshCw className="h-4 w-4" />
                  Refresh
                </DropdownMenuItem>
                <DropdownMenuItem onClick={fullSync} disabled={isSyncing}>
                  <RotateCcw className="h-4 w-4" />
                  Full sync (restore deleted items)
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>
      </div>

      {!isNotesOnly && (syncEntries.length > 0 || analysisEntries.length > 0) && (
        <SyncProgressBar
          entries={syncEntries}
          projects={projects}
          analysisEntries={analysisEntries}
          items={items}
        />
      )}

      {filteredItems.length === 0 ? (
        renderEmptyState()
      ) : (
      <ScrollArea className="min-h-0 flex-1">
          <div className="flex flex-col">
            {filteredItems.map((item) => {
              if (item.item_type === "note") {
                const project = projectMap.get(item.project_id);
                return (
                  <NoteItem
                    key={`note-${item.id}`}
                    note={item}
                    projectLabel={project ? `${project.owner}/${project.name}` : undefined}
                    isSelected={item.id === selectedNoteId}
                    isChecked={selectedNoteIdSet.has(item.id)}
                    isGenerating={isGenerating[item.id] || false}
                    onToggleSelect={() => toggleNoteSelection(item.id)}
                    onClick={() => handleNoteClick(item.id)}
                    onDelete={() => handleNoteDelete(item.id)}
                    onGenerate={() => handleNoteGenerate(item.id)}
                    onEdit={() => openEditNote(item)}
                    onToggleStar={() => handleNoteToggleStar(item)}
                  />
                );
              }
              const project = projectMap.get(item.project_id);
              return (
                <InboxItem
                  key={`item-${item.id}`}
                  item={item}
                  repoName={project ? `${project.owner}/${project.name}` : undefined}
                  platform={project?.platform}
                  isSelected={item.id === selectedItemId}
                  isAnalyzing={!!activeAnalyses[item.id]?.isStreaming}
                  hasAnalysis={analyzedItemIds.has(item.id)}
                  isChecked={selectedIdSet.has(item.id)}
                  onToggleSelect={() => toggleItemSelection(item.id)}
                  onClick={() => handleItemClick(item.id)}
                  onToggleStar={() => handleToggleStar(item)}
                  onMarkUnread={() => markUnread(item.id)}
                  onDelete={() => deleteItem(item.id)}
                  onRestore={() => restoreItem(item.id)}
                  isDismissedView={showDismissedOnly}
                />
              );
            })}
            <div ref={sentinelRef} className="h-1" />
            {isLoadingMore && (
              <div className="flex items-center justify-center py-3">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
              </div>
            )}
          </div>
      </ScrollArea>
      )}
      {selectedItemIds.length > 0 && (
        <BulkActionBar
          selectedItems={selectedItems}
          onMarkRead={handleBulkMarkRead}
          onMarkUnread={handleBulkMarkUnread}
          onDelete={handleBulkDelete}
          onRestore={handleBulkRestore}
          isDismissedView={showDismissedOnly}
          onAiAction={handleBulkAiAction}
          onClearSelection={clearSelection}
        />
      )}
      {selectedNoteIds.length > 0 && (
        <NoteBulkActionBar
          selectedNotes={selectedNotes}
          onGenerateIssue={handleBulkNoteGenerate}
          onDelete={handleBulkNoteDelete}
          onClearSelection={clearNoteSelection}
        />
      )}
      <CreateNoteDialog />
    </div>
  );
}
