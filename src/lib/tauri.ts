import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Project,
  Item,
  ItemPageResponse,
  ChatMessage,
  AnalyzeActionRequest,
  AiSettings,
  AppSettings,
  AppPaths,
  AuthStatus,
  GitHubRepo,
  GitLabProject,
  Connector,
  ConnectorRepo,
  BackupInfo,
  LogEntriesResponse,
  ProjectNote,
  ProjectSettingEntry,
  DraftIssue,
  CreateIssueResponse,
  UpdateInfo,
} from "@/types";

export { listen, type UnlistenFn };

// Auth commands
export const getAuthStatus = () => invoke<AuthStatus>("get_auth_status");
export const saveGithubToken = (token: string) =>
  invoke<void>("save_github_token", { token });
export const saveGitlabToken = (token: string, baseUrl?: string) =>
  invoke<void>("save_gitlab_token", { token, baseUrl });
export const disconnectGithub = () => invoke<void>("disconnect_github");
export const disconnectGitlab = () => invoke<void>("disconnect_gitlab");
export const listGithubRepos = (connectorId?: string) =>
  invoke<GitHubRepo[]>("list_github_repos", { connectorId });
export const listGitlabProjects = (connectorId?: string) =>
  invoke<GitLabProject[]>("list_gitlab_projects", { connectorId });

// Connector commands
export const listConnectors = () =>
  invoke<Connector[]>("list_connectors");
export const addConnector = (input: {
  name: string;
  platform: "github" | "gitlab";
  token: string;
  base_url?: string;
}) => invoke<Connector>("add_connector", { input });
export const updateConnector = (
  id: string,
  input: { name?: string; token?: string; base_url?: string }
) => invoke<Connector>("update_connector", { id, input });
export const removeConnector = (id: string) =>
  invoke<void>("remove_connector", { id });
export const listConnectorRepos = (connectorId: string) =>
  invoke<ConnectorRepo[]>("list_connector_repos", { connectorId });

// Repo commands
export const listProjects = () => invoke<Project[]>("list_projects");
export const addProject = (input: {
  name: string;
  owner: string;
  platform: string;
  url: string;
  connector_id?: string;
}) => invoke<Project>("add_project", { input });
export const addProjectByUrl = (url: string, connectorId?: string) =>
  invoke<Project>("add_project_by_url", { url, connectorId });
export const removeProject = (id: string) =>
  invoke<void>("remove_project", { id });
export const prepareRepo = (
  projectId: string,
  branch?: string,
  prNumber?: number
) => invoke<string>("prepare_repo", { projectId, branch, prNumber });
export const clearRepoCache = () => invoke<void>("clear_repo_cache");
export const toggleProjectSync = (id: string, enabled: boolean) =>
  invoke<void>("toggle_project_sync", { id, enabled });

// Item commands
export const listItems = (filters: {
  projectId?: string;
  itemType?: string;
  starredOnly?: boolean;
  searchQuery?: string;
  cursor?: string;
  pageSize?: number;
}) => invoke<ItemPageResponse>("list_items", { ...filters });
export const getItem = (id: string) => invoke<Item>("get_item", { id });
export const markItemRead = (id: string, isRead: boolean) =>
  invoke<void>("mark_item_read", { id, isRead });
export const toggleItemStar = (id: string, isStarred: boolean) =>
  invoke<void>("toggle_item_star", { id, isStarred });
export const syncProjectItems = (projectId: string) =>
  invoke<void>("sync_project_items", { projectId });
export const deleteItem = (id: string) =>
  invoke<void>("delete_item", { id });
export const listDismissedItems = (filters: {
  projectId?: string;
  itemType?: string;
  searchQuery?: string;
  cursor?: string;
  pageSize?: number;
}) => invoke<ItemPageResponse>("list_dismissed_items", { ...filters });
export const restoreItem = (id: string) =>
  invoke<void>("restore_item", { id });
export const clearProjectData = (projectId: string) =>
  invoke<void>("clear_project_data", { projectId });
export const fullSyncProjectItems = (projectId: string) =>
  invoke<void>("full_sync_project_items", { projectId });

// AI commands
export const getChatMessages = (itemId: string) =>
  invoke<ChatMessage[]>("get_chat_messages", { itemId });
export const sendChatMessage = (itemId: string, message: string) =>
  invoke<ChatMessage>("send_chat_message", { itemId, message });
export const autoAnalyzeItem = (itemId: string) =>
  invoke<ChatMessage>("auto_analyze_item", { itemId });
export const clearChat = (itemId: string) =>
  invoke<void>("clear_chat", { itemId });
export const analyzeItemAction = (request: AnalyzeActionRequest) =>
  invoke<ChatMessage>("analyze_item_action", { request });
export const getAnalyzedItemIds = () =>
  invoke<string[]>("get_analyzed_item_ids");

// Project notes commands
export const listProjectNotes = (projectId: string) =>
  invoke<ProjectNote[]>("list_project_notes", { projectId });
export const addProjectNote = (projectId: string, content: string) =>
  invoke<ProjectNote>("add_project_note", { projectId, content });
export const removeProjectNote = (id: string) =>
  invoke<void>("remove_project_note", { id });

// Project settings commands
export const getProjectSettings = (projectId: string) =>
  invoke<ProjectSettingEntry[]>("get_project_settings", { projectId });
export const updateProjectSetting = (
  projectId: string,
  key: string,
  value: string
) => invoke<void>("update_project_setting", { projectId, key, value });
export const deleteProjectSetting = (projectId: string, key: string) =>
  invoke<void>("delete_project_setting", { projectId, key });

// Settings commands
export const getSettings = () => invoke<AppSettings>("get_settings");
export const getAiSettings = () => invoke<AiSettings>("get_ai_settings");
export const updateSetting = (key: string, value: string) =>
  invoke<void>("update_setting", { key, value });
export const deleteSetting = (key: string) =>
  invoke<void>("delete_setting", { key });
export const isOnboardingComplete = () =>
  invoke<boolean>("is_onboarding_complete");
export const getAppPaths = () => invoke<AppPaths>("get_app_paths");

// Database commands
export const createBackup = () => invoke<BackupInfo>("create_backup");
export const listBackups = () => invoke<BackupInfo[]>("list_backups");
export const restoreBackup = (filename: string) =>
  invoke<void>("restore_backup", { filename });
export const resetDatabase = () => invoke<void>("reset_database");
export const deleteBackup = (filename: string) =>
  invoke<void>("delete_backup", { filename });

// Draft issue commands
export const listDraftIssues = (projectId?: string) =>
  invoke<DraftIssue[]>("list_draft_issues", { projectId });
export const createDraftIssue = (projectId: string, rawContent: string) =>
  invoke<DraftIssue>("create_draft_issue", { projectId, rawContent });
export const updateDraftIssue = (
  id: string,
  updates: {
    projectId?: string;
    title?: string;
    body?: string;
    labels?: string[];
    priority?: string;
    area?: string;
    rawContent?: string;
  }
) =>
  invoke<DraftIssue>("update_draft_issue", { id, ...updates });
export const deleteDraftIssue = (id: string) =>
  invoke<void>("delete_draft_issue", { id });
export const generateIssueFromDraft = (id: string) =>
  invoke<DraftIssue>("generate_issue_from_draft", { id });
export const submitDraftToProvider = (id: string) =>
  invoke<CreateIssueResponse>("submit_draft_to_provider", { id });
export const getDraftIssueCount = () =>
  invoke<number>("get_draft_issue_count");
export const toggleDraftIssueStar = (id: string, isStarred: boolean) =>
  invoke<void>("toggle_draft_issue_star", { id, isStarred });

// Logging commands
export const getLogLevel = () => invoke<string>("get_log_level");
export const setLogLevel = (level: string) =>
  invoke<void>("set_log_level", { level });
export const getLogEntries = (
  levelFilter?: string,
  textFilter?: string,
  limit?: number,
  offset?: number
) =>
  invoke<LogEntriesResponse>("get_log_entries", {
    levelFilter,
    textFilter,
    limit,
    offset,
  });
export const clearLogs = () => invoke<void>("clear_logs");

// Updater commands
export const checkForUpdate = () =>
  invoke<UpdateInfo | null>("check_for_update");
