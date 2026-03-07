import { useMemo, useRef, useEffect, useCallback, useState } from "react";
import { errorMessage } from "@/lib/utils";
import { useItems } from "@/hooks/useItems";
import { useAppStore } from "@/stores/appStore";
import { useItemStore } from "@/stores/itemStore";
import { useDraftIssueStore } from "@/stores/draftIssueStore";
import { useUiStore } from "@/stores/uiStore";
import { InboxItem } from "@/components/inbox/InboxItem";
import { extractLinkedIssueNumbers } from "@/lib/linkedItems";
import { NoteItem } from "@/components/notes/NoteItem";
import { BulkActionBar } from "@/components/inbox/BulkActionBar";
import { NoteBulkActionBar } from "@/components/notes/NoteBulkActionBar";
import { CreateNoteDialog } from "@/components/notes/CreateNoteDialog";
import { SyncProgressBar } from "@/components/inbox/SyncProgressBar";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  MoreVertical, RefreshCw, RotateCw, RotateCcw, Loader2, Plus, StickyNote, Search, X,
  Star, EyeOff, Sparkles, PauseCircle, Inbox, SearchX, CircleDot, GitPullRequest, MessageCircle, Keyboard, FolderGit2,
  Github, GitlabIcon, ChevronRight,
} from "lucide-react";
import { Input } from "@/components/ui/input";
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
} from "@/components/ui/dialog";
import { toast } from "sonner";
import { useChatStore } from "@/stores/chatStore";
import * as api from "@/lib/tauri";
import type { AnalysisAction, Item } from "@/types";
import { EmptyState } from "@/components/shared/EmptyState";
import { KeyboardShortcutsDialog } from "@/components/shared/KeyboardShortcutsDialog";

export function InboxList() {
  const { items, syncItems, isSyncing, syncDisabled, setSelectedItemId, selectedItemId, markRead, markUnread, deleteItem, restoreItem, fullSync } = useItems();
  const { selectedProjectIds, projects, syncingProjects, activeAnalyses, selectedItemIds, toggleItemSelection, selectAllItems, clearSelection, startAnalysis, clearAnalysis, analyzedItemIds, removeAnalyzedItemId, showAnalyzedOnly, showStarredOnly, showDismissedOnly, updateItem, itemTypeFilter, setItemTypeFilter, refreshInbox, hasMore, isLoadingMore, fetchMore, searchQuery, setSearchQuery } = useAppStore();
  const {
    selectedNoteId,
    setSelectedNoteId,
    selectedNoteIds,
    toggleNoteSelection,
    clearNoteSelection,
    lastClickedNoteIndex,
    setLastClickedNoteIndex,
    isGenerating,
    setIsGenerating,
    openCreateNote,
    openEditNote,
  } = useDraftIssueStore();
  const {
    lastClickedIndex,
    setLastClickedIndex,
    focusedIndex,
    setFocusedIndex,
  } = useItemStore();
  const { groupByRepository, setGroupByRepository } = useUiStore();

  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const animatedGroupsRef = useRef<Set<string>>(new Set());
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [publishConfirmOpen, setPublishConfirmOpen] = useState(false);
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

  // Compute linked items map: itemId → linked Item[]
  // Uses allItemsCache so links work even when filtered by type (e.g. Issues or PRs view)
  const allItemsCache = useItemStore((s) => s.allItemsCache);
  const linkedItemsMap = useMemo(() => {
    const allItems = Array.from(allItemsCache.values());
    const map = new Map<string, Item[]>();
    // Index: projectId → externalId → Item
    const byProjectAndNumber = new Map<string, Map<number, Item>>();
    // Index: projectId → [PR items with refs]
    const prsByProject = new Map<string, { pr: Item; refs: number[] }[]>();

    for (const item of allItems) {
      if (item.type_data.kind === "note") continue;
      const key = item.project_id;
      if (!byProjectAndNumber.has(key)) byProjectAndNumber.set(key, new Map());
      byProjectAndNumber.get(key)!.set(item.type_data.external_id, item);

      if (item.item_type === "pr" && item.body) {
        const refs = extractLinkedIssueNumbers(item.body);
        if (refs.length > 0) {
          if (!prsByProject.has(key)) prsByProject.set(key, []);
          prsByProject.get(key)!.push({ pr: item, refs });
        }
      }
    }

    // PR → linked issues
    for (const [projId, prs] of prsByProject) {
      const numIndex = byProjectAndNumber.get(projId);
      if (!numIndex) continue;
      for (const { pr, refs } of prs) {
        const linked = refs.map((n) => numIndex.get(n)).filter((i): i is Item => !!i && i.id !== pr.id);
        if (linked.length > 0) map.set(pr.id, linked);
        // Issue → linking PRs (reverse)
        for (const issue of linked) {
          const existing = map.get(issue.id) ?? [];
          if (!existing.some((i) => i.id === pr.id)) {
            map.set(issue.id, [...existing, pr]);
          }
        }
      }
    }
    return map;
  }, [allItemsCache]);

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

  const itemIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    filteredItems.forEach((item, i) => map.set(item.id, i));
    return map;
  }, [filteredItems]);

  const groupedItems = useMemo(() => {
    if (!groupByRepository) {
      animatedGroupsRef.current.clear();
      return null;
    }
    const groups = new Map<string, typeof filteredItems>();
    for (const item of filteredItems) {
      const key = item.project_id;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(item);
    }
    return groups;
  }, [filteredItems, groupByRepository]);

  // Range selection handler for shift+click
  const handleToggleSelectItem = useCallback(
    (id: string, index: number, event: React.MouseEvent) => {
      if (event.shiftKey && lastClickedIndex !== null) {
        const start = Math.min(lastClickedIndex, index);
        const end = Math.max(lastClickedIndex, index);
        const rangeIds = filteredItems
          .slice(start, end + 1)
          .filter((item) => item.item_type !== "note")
          .map((item) => item.id);
        const merged = new Set([...selectedItemIds, ...rangeIds]);
        selectAllItems(Array.from(merged));
      } else {
        toggleItemSelection(id);
      }
      setLastClickedIndex(index);
    },
    [lastClickedIndex, filteredItems, selectedItemIds, selectAllItems, toggleItemSelection, setLastClickedIndex]
  );

  const handleToggleSelectNote = useCallback(
    (id: string, index: number, event: React.MouseEvent) => {
      if (event.shiftKey && lastClickedNoteIndex !== null) {
        const start = Math.min(lastClickedNoteIndex, index);
        const end = Math.max(lastClickedNoteIndex, index);
        const rangeIds = filteredItems
          .slice(start, end + 1)
          .filter((item) => item.item_type === "note")
          .map((item) => item.id);
        const merged = new Set([...selectedNoteIds, ...rangeIds]);
        useDraftIssueStore.setState({ selectedNoteIds: Array.from(merged) });
      } else {
        toggleNoteSelection(id);
      }
      setLastClickedNoteIndex(index);
    },
    [lastClickedNoteIndex, filteredItems, selectedNoteIds, toggleNoteSelection, setLastClickedNoteIndex]
  );

  // Select All checkbox logic
  const nonNoteItems = useMemo(
    () => filteredItems.filter((item) => item.item_type !== "note"),
    [filteredItems]
  );
  const noteItems = useMemo(
    () => filteredItems.filter((item) => item.item_type === "note"),
    [filteredItems]
  );
  const allItemsSelected = nonNoteItems.length > 0 && nonNoteItems.every((item) => selectedIdSet.has(item.id));
  const allNotesSelected = noteItems.length > 0 && noteItems.every((item) => selectedNoteIdSet.has(item.id));
  const someSelected = selectedItemIds.length > 0 || selectedNoteIds.length > 0;
  const allSelected = (nonNoteItems.length === 0 || allItemsSelected) && (noteItems.length === 0 || allNotesSelected) && filteredItems.length > 0;

  const handleSelectAll = useCallback(() => {
    if (allSelected) {
      clearSelection();
      clearNoteSelection();
    } else {
      if (nonNoteItems.length > 0) selectAllItems(nonNoteItems.map((i) => i.id));
      if (noteItems.length > 0) useDraftIssueStore.setState({ selectedNoteIds: noteItems.map((i) => i.id) });
    }
  }, [allSelected, nonNoteItems, noteItems, selectAllItems, clearSelection, clearNoteSelection]);

  // Keyboard navigation
  const itemRefs = useRef<Map<number, HTMLElement>>(new Map());

  const handleItemClick = async (id: string) => {
    setSelectedNoteId(null);
    clearNoteSelection();
    setSelectedItemId(id);
    await markRead(id);
  };

  const handleNavigateToLinkedItem = async (id: string) => {
    // Switch to "all" filter so the linked item is visible regardless of current type filter
    if (itemTypeFilter !== "all") {
      setItemTypeFilter("all");
    }
    handleItemClick(id);
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

  const handleClearHistory = async (itemId: string) => {
    try {
      await api.clearChat(itemId);
      removeAnalyzedItemId(itemId);
      clearAnalysis(itemId);
      useChatStore.getState().clearChat(itemId);
    } catch (err) {
      toast.error("Failed to clear AI history", { description: errorMessage(err) });
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

  // Keyboard navigation effect
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (document.activeElement?.tagName || "").toLowerCase();
      if (tag === "input" || tag === "textarea" || (document.activeElement as HTMLElement)?.isContentEditable) {
        return;
      }

      const total = filteredItems.length;
      if (total === 0) return;

      const current = focusedIndex ?? -1;

      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        const next = Math.min(current + 1, total - 1);
        setFocusedIndex(next);
        itemRefs.current.get(next)?.scrollIntoView({ block: "nearest" });
        if (e.shiftKey) {
          const item = filteredItems[next];
          if (item.item_type === "note") {
            if (!selectedNoteIdSet.has(item.id)) toggleNoteSelection(item.id);
          } else {
            if (!selectedIdSet.has(item.id)) toggleItemSelection(item.id);
          }
        }
        return;
      }

      if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        const next = Math.max(current - 1, 0);
        setFocusedIndex(next);
        itemRefs.current.get(next)?.scrollIntoView({ block: "nearest" });
        if (e.shiftKey) {
          const item = filteredItems[next];
          if (item.item_type === "note") {
            if (!selectedNoteIdSet.has(item.id)) toggleNoteSelection(item.id);
          } else {
            if (!selectedIdSet.has(item.id)) toggleItemSelection(item.id);
          }
        }
        return;
      }

      if (e.key === "x" && current >= 0 && current < total) {
        e.preventDefault();
        const item = filteredItems[current];
        if (item.item_type === "note") {
          toggleNoteSelection(item.id);
        } else {
          toggleItemSelection(item.id);
        }
        return;
      }

      if ((e.key === "Enter" || e.key === "o") && current >= 0 && current < total) {
        e.preventDefault();
        const item = filteredItems[current];
        if (item.item_type === "note") {
          handleNoteClick(item.id);
        } else {
          handleItemClick(item.id);
        }
        return;
      }

      if (e.key === "e" && !showDismissedOnly) {
        e.preventDefault();
        if (selectedItemIds.length > 0) {
          handleBulkDelete();
        } else if (current >= 0 && current < total) {
          const item = filteredItems[current];
          if (item.item_type !== "note") {
            deleteItem(item.id);
          }
        }
        return;
      }

      if (e.key === "a" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSelectAll();
        return;
      }

      if (e.key === "?") {
        e.preventDefault();
        setShortcutsOpen(true);
        return;
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [focusedIndex, filteredItems, selectedIdSet, selectedNoteIdSet, selectedItemIds, setFocusedIndex, toggleItemSelection, toggleNoteSelection, showDismissedOnly, handleSelectAll, deleteItem, handleBulkDelete, handleItemClick, handleNoteClick]);

  // Reset focused index when filtered items change
  useEffect(() => {
    if (focusedIndex !== null && focusedIndex >= filteredItems.length) {
      setFocusedIndex(filteredItems.length > 0 ? filteredItems.length - 1 : null);
    }
  }, [filteredItems.length, focusedIndex, setFocusedIndex]);

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

  const handleBulkNotePublish = async () => {
    const readyNotes = selectedNotes.filter((n) => n.type_data.kind === "note" && n.type_data.draft_status === "ready");
    clearNoteSelection();

    let successCount = 0;
    for (const note of readyNotes) {
      try {
        await api.submitDraftToProvider(note.id);
        successCount++;
      } catch (err) {
        toast.error(`Failed to publish note`, { description: errorMessage(err) });
      }
    }
    if (successCount > 0) {
      toast.success(`Published ${successCount} issue${successCount > 1 ? "s" : ""}`);
    }
    await refreshInbox();
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
            {filteredItems.length > 0 && (
              <Checkbox
                checked={allSelected ? true : someSelected ? "indeterminate" : false}
                onClick={handleSelectAll}
                className="shrink-0"
                aria-label="Select all"
              />
            )}
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
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={isNotesOnly ? () => refreshInbox() : syncItems}
            disabled={!isNotesOnly && isSyncing}
            aria-label="Refresh"
          >
            {!isNotesOnly && isSyncing ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <RefreshCw className="h-4 w-4" />
            )}
          </Button>
          <Button
            variant={groupByRepository ? "secondary" : "ghost"}
            size="icon"
            className="h-8 w-8"
            onClick={() => setGroupByRepository(!groupByRepository)}
            title="Group by repository"
            aria-label="Group by repository"
          >
            <FolderGit2 className="h-4 w-4" />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8" aria-label="More options">
                <MoreVertical className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" onCloseAutoFocus={(e) => e.preventDefault()}>
              <DropdownMenuItem onClick={() => window.location.reload()}>
                <RotateCw className="h-4 w-4" />
                Reload page
              </DropdownMenuItem>
              {!isNotesOnly && (
                <DropdownMenuItem onClick={fullSync} disabled={isSyncing}>
                  <RotateCcw className="h-4 w-4" />
                  Full sync (restore deleted items)
                </DropdownMenuItem>
              )}
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => setShortcutsOpen(true)}>
                <Keyboard className="h-4 w-4" />
                Keyboard shortcuts
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
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
            {groupByRepository && groupedItems ? (
              Array.from(groupedItems.entries()).map(([projectId, groupItems], groupIdx) => {
                const project = projectMap.get(projectId);
                const isCollapsed = collapsedGroups.has(projectId);
                const PlatformIcon = project?.platform === "gitlab" ? GitlabIcon : Github;
                const isNew = !animatedGroupsRef.current.has(projectId);
                if (isNew) animatedGroupsRef.current.add(projectId);
                return (
                  <div key={projectId} className={isNew ? "repo-group-enter" : ""} style={isNew ? { animationDelay: `${groupIdx * 40}ms` } : undefined}>
                    <button
                      className="sticky top-0 z-10 flex w-full items-center gap-2.5 border-b border-border/60 bg-background/80 px-4 py-2 text-left backdrop-blur-md transition-colors hover:bg-muted/40"
                      onClick={() => setCollapsedGroups(prev => {
                        const next = new Set(prev);
                        if (next.has(projectId)) next.delete(projectId);
                        else next.add(projectId);
                        return next;
                      })}
                    >
                      <ChevronRight className={`h-3 w-3 shrink-0 text-muted-foreground/60 transition-transform duration-200 ${isCollapsed ? "" : "rotate-90"}`} />
                      <PlatformIcon className="h-3.5 w-3.5 shrink-0 text-muted-foreground/70" />
                      <span className="truncate text-xs" style={{ fontFamily: "'Syne', sans-serif" }}>
                        <span className="text-muted-foreground/60">{project?.owner ?? ""}</span>
                        <span className="text-muted-foreground/40 mx-0.5">/</span>
                        <span className="font-semibold text-foreground/80">{project?.name ?? projectId}</span>
                      </span>
                      <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-muted px-1.5 text-[10px] font-medium tabular-nums text-muted-foreground">
                        {groupItems.length}
                      </span>
                    </button>
                    {!isCollapsed && groupItems.map((item) => {
                      const globalIndex = itemIndexMap.get(item.id)!;
                      if (item.item_type === "note") {
                        const proj = projectMap.get(item.project_id);
                        return (
                          <NoteItem
                            key={`note-${item.id}`}
                            ref={(el: HTMLElement | null) => {
                              if (el) itemRefs.current.set(globalIndex, el);
                              else itemRefs.current.delete(globalIndex);
                            }}
                            note={item}
                            projectLabel={proj ? `${proj.owner}/${proj.name}` : undefined}
                            isSelected={item.id === selectedNoteId}
                            isChecked={selectedNoteIdSet.has(item.id)}
                            isFocused={globalIndex === focusedIndex}
                            isGenerating={isGenerating[item.id] || false}
                            onToggleSelect={(e: React.MouseEvent) => handleToggleSelectNote(item.id, globalIndex, e)}
                            onClick={() => handleNoteClick(item.id)}
                            onDelete={() => handleNoteDelete(item.id)}
                            onGenerate={() => handleNoteGenerate(item.id)}
                            onEdit={() => openEditNote(item)}
                            onToggleStar={() => handleNoteToggleStar(item)}
                          />
                        );
                      }
                      const proj = projectMap.get(item.project_id);
                      return (
                        <InboxItem
                          key={`item-${item.id}`}
                          ref={(el: HTMLElement | null) => {
                            if (el) itemRefs.current.set(globalIndex, el);
                            else itemRefs.current.delete(globalIndex);
                          }}
                          item={item}
                          repoName={proj ? `${proj.owner}/${proj.name}` : undefined}
                          platform={proj?.platform}
                          isSelected={item.id === selectedItemId}
                          isAnalyzing={!!activeAnalyses[item.id]?.isStreaming}
                          hasAnalysis={analyzedItemIds.has(item.id)}
                          isChecked={selectedIdSet.has(item.id)}
                          isFocused={globalIndex === focusedIndex}
                          onToggleSelect={(e: React.MouseEvent) => handleToggleSelectItem(item.id, globalIndex, e)}
                          onClick={() => handleItemClick(item.id)}
                          onToggleStar={() => handleToggleStar(item)}
                          onMarkUnread={() => markUnread(item.id)}
                          onDelete={() => deleteItem(item.id)}
                          onRestore={() => restoreItem(item.id)}
                          onClearHistory={() => handleClearHistory(item.id)}
                          isDismissedView={showDismissedOnly}

                    linkedItems={linkedItemsMap.get(item.id)}
                    onNavigateToItem={handleNavigateToLinkedItem}
                        />
                      );
                    })}
                  </div>
                );
              })
            ) : (
              filteredItems.map((item, index) => {
                if (item.item_type === "note") {
                  const project = projectMap.get(item.project_id);
                  return (
                    <NoteItem
                      key={`note-${item.id}`}
                      ref={(el: HTMLElement | null) => {
                        if (el) itemRefs.current.set(index, el);
                        else itemRefs.current.delete(index);
                      }}
                      note={item}
                      projectLabel={project ? `${project.owner}/${project.name}` : undefined}
                      isSelected={item.id === selectedNoteId}
                      isChecked={selectedNoteIdSet.has(item.id)}
                      isFocused={index === focusedIndex}
                      isGenerating={isGenerating[item.id] || false}
                      onToggleSelect={(e: React.MouseEvent) => handleToggleSelectNote(item.id, index, e)}
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
                    ref={(el: HTMLElement | null) => {
                      if (el) itemRefs.current.set(index, el);
                      else itemRefs.current.delete(index);
                    }}
                    item={item}
                    repoName={project ? `${project.owner}/${project.name}` : undefined}
                    platform={project?.platform}
                    isSelected={item.id === selectedItemId}
                    isAnalyzing={!!activeAnalyses[item.id]?.isStreaming}
                    hasAnalysis={analyzedItemIds.has(item.id)}
                    isChecked={selectedIdSet.has(item.id)}
                    isFocused={index === focusedIndex}
                    onToggleSelect={(e: React.MouseEvent) => handleToggleSelectItem(item.id, index, e)}
                    onClick={() => handleItemClick(item.id)}
                    onToggleStar={() => handleToggleStar(item)}
                    onMarkUnread={() => markUnread(item.id)}
                    onDelete={() => deleteItem(item.id)}
                    onRestore={() => restoreItem(item.id)}
                    onClearHistory={() => handleClearHistory(item.id)}
                    isDismissedView={showDismissedOnly}
                    linkedItems={linkedItemsMap.get(item.id)}
                    onNavigateToItem={handleNavigateToLinkedItem}
                  />
                );
              })
            )}
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
          onPublish={() => setPublishConfirmOpen(true)}
          onDelete={handleBulkNoteDelete}
          onClearSelection={clearNoteSelection}
        />
      )}
      <CreateNoteDialog />
      <KeyboardShortcutsDialog open={shortcutsOpen} onOpenChange={setShortcutsOpen} />
      <Dialog open={publishConfirmOpen} onOpenChange={setPublishConfirmOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Publish Notes as Issues</DialogTitle>
            <DialogDescription>
              This will create {selectedNotes.filter((n) => n.type_data.kind === "note" && n.type_data.draft_status === "ready").length} issue{selectedNotes.filter((n) => n.type_data.kind === "note" && n.type_data.draft_status === "ready").length !== 1 ? "s" : ""} on the respective repositories. This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Cancel</Button>
            </DialogClose>
            <Button onClick={() => { setPublishConfirmOpen(false); handleBulkNotePublish(); }}>
              Publish
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
