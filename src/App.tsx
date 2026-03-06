import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "@/stores/appStore";
import { useDraftIssueStore } from "@/stores/draftIssueStore";
import { useTheme } from "@/hooks/useTheme";
import { useAIStreaming } from "@/hooks/useAIStreaming";
import { useUpdateChecker } from "@/hooks/useUpdateChecker";
import { UpdateBanner } from "@/components/UpdateBanner";
import { Sidebar } from "@/components/layout/Sidebar";
import { InboxList } from "@/components/layout/InboxList";
import { ChatPanel } from "@/components/layout/ChatPanel";
import { NotePanel } from "@/components/notes/NotePanel";
import { SplashScreen } from "@/components/layout/SplashScreen";
import { Welcome } from "@/components/onboarding/Welcome";
import { SettingsPage } from "@/components/settings/SettingsPage";
import { TooltipProvider } from "@/components/ui/tooltip";
import { DatabaseError } from "@/components/DatabaseError";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { Toaster } from "@/components/ui/sonner";
import { cn } from "@/lib/utils";
import * as api from "@/lib/tauri";

const MIN_CHAT_WIDTH = 200;
const MAX_CHAT_WIDTH = 1200;
const DEFAULT_CHAT_WIDTH = 480;

function App() {
  const { currentPage, setCurrentPage, selectedItemId, setProjects, setItems, resolvedTheme } = useAppStore();
  const selectedNoteId = useDraftIssueStore((s) => s.selectedNoteId);
  useTheme();
  useAIStreaming();
  const { updateInfo, dismissUpdate } = useUpdateChecker();
  const [chatWidth, setChatWidth] = useState(DEFAULT_CHAT_WIDTH);
  const [isLoading, setIsLoading] = useState(true);
  const [dbError, setDbError] = useState<string | null>(null);
  const isResizing = useRef(false);

  const initApp = useCallback(async () => {
    try {
      const complete = await api.isOnboardingComplete();
      if (complete) {
        // Check if AI is actually configured (not just defaults)
        const aiSettings = await api.getAiSettings();
        const aiConfigured =
          aiSettings.has_ai_api_key ||
          aiSettings.ai_mode !== "api";

        if (!aiConfigured) {
          // Connectors exist but AI not configured — resume onboarding
          setCurrentPage("onboarding");
        } else {
          // Preload projects and items so the inbox doesn't flash a spinner
          const projects = await api.listProjects();
          setProjects(projects);
          const response = await api.listItems({});
          setItems(response.items);
          const settings = await api.getSettings();
          useAppStore.getState().setRefreshInterval(settings.refresh_interval);
          setCurrentPage("main");
        }
      } else {
        setCurrentPage("onboarding");
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("Database not initialized")) {
        setDbError(msg);
      } else {
        setCurrentPage("onboarding");
      }
    } finally {
      setIsLoading(false);
    }
  }, [setCurrentPage, setProjects, setItems]);

  useEffect(() => {
    initApp();
  }, [initApp]);

  // Cmd+N to open create note dialog, Cmd+R to refresh, Cmd+1-5 to switch tabs
  useEffect(() => {
    const tabFilters = ["all", "note", "issue", "pr", "discussion"] as const;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!e.metaKey) return;
      if (e.key === "n") {
        e.preventDefault();
        useDraftIssueStore.getState().openCreateNote();
        return;
      }
      if (e.key === "r") {
        e.preventDefault();
        useAppStore.getState().refreshInbox();
        return;
      }
      if (!e.shiftKey && !e.altKey) {
        const num = parseInt(e.key, 10);
        if (num >= 1 && num <= 5) {
          e.preventDefault();
          useAppStore.getState().setItemTypeFilter(tabFilters[num - 1]);
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Listen for DB init errors emitted from the backend
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<string>("db:init-error", (event) => {
      setDbError(event.payload);
      setIsLoading(false);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isResizing.current = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing.current) return;
      const newWidth = window.innerWidth - e.clientX;
      setChatWidth(Math.min(MAX_CHAT_WIDTH, Math.max(MIN_CHAT_WIDTH, newWidth)));
    };

    const handleMouseUp = () => {
      isResizing.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }, []);

  if (isLoading) {
    return <SplashScreen />;
  }

  if (dbError) {
    return (
      <DatabaseError
        error={dbError}
        onReset={() => {
          setDbError(null);
          setIsLoading(true);
          initApp();
        }}
      />
    );
  }

  // Determine which right panel to show
  const showChatPanel = selectedItemId != null;
  const showNotePanel = selectedNoteId != null && selectedItemId == null;
  const showRightPanel = showChatPanel || showNotePanel;

  return (
    <ErrorBoundary>
      <TooltipProvider>
        {currentPage === "onboarding" && (
          <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
            <Welcome />
          </div>
        )}
        {(currentPage === "main" || currentPage === "settings") && (
          <div className={cn("flex h-screen w-screen overflow-hidden bg-background text-foreground", resolvedTheme === "dark" && "dark")}>
            {currentPage === "settings" && <SettingsPage />}
            {currentPage === "main" && (
              <div className="flex flex-1 flex-col overflow-hidden">
                {updateInfo && (
                  <UpdateBanner updateInfo={updateInfo} onDismiss={dismissUpdate} />
                )}
              <div className="flex flex-1 overflow-hidden">
                <Sidebar />
                <InboxList />
                {showRightPanel && (
                  <>
                    {/* Resize handle */}
                    <div
                      className="flex w-1.5 shrink-0 cursor-col-resize items-center justify-center transition-colors hover:bg-primary/10 active:bg-primary/20"
                      onMouseDown={handleMouseDown}
                    >
                      <div className="h-8 w-0.5 rounded-full bg-border" />
                    </div>
                    {showChatPanel && <ChatPanel width={chatWidth} />}
                    {showNotePanel && <NotePanel width={chatWidth} />}
                  </>
                )}
              </div>
              </div>
            )}
          </div>
        )}
        <Toaster position="bottom-center" offset={48} />
      </TooltipProvider>
    </ErrorBoundary>
  );
}

export default App;
