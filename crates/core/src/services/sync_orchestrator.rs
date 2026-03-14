use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use crate::enums::{ItemState, ItemType};
use crate::models::project;
use crate::services::github::GitHubClient;
use crate::services::gitlab::GitLabClient;
use crate::sync;

/// Per-project sync filter configuration loaded from project_settings.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub sync_from_date_issues: Option<String>,
    pub sync_from_date_prs: Option<String>,
    pub sync_from_date_discussions: Option<String>,
    pub sync_issues: bool,
    pub sync_prs: bool,
    pub sync_discussions: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_from_date_issues: None,
            sync_from_date_prs: None,
            sync_from_date_discussions: None,
            sync_issues: true,
            sync_prs: true,
            sync_discussions: true,
        }
    }
}

/// Compute the effective `since` value for sync.
/// `sync_from_date` acts as a floor — we never fetch items older than this.
fn compute_since(
    is_full_reconciliation: bool,
    last_sync_at: Option<chrono::NaiveDateTime>,
    sync_from_date: Option<&str>,
) -> Option<String> {
    if is_full_reconciliation {
        sync_from_date.map(|s| s.to_string())
    } else {
        let last_sync = last_sync_at.map(|ts| ts.format("%Y-%m-%dT%H:%M:%SZ").to_string());
        match (last_sync, sync_from_date) {
            (Some(ls), Some(sf)) => Some(if ls.as_str() > sf { ls } else { sf.to_string() }),
            (Some(ls), None) => Some(ls),
            (None, Some(sf)) => Some(sf.to_string()),
            (None, None) => None,
        }
    }
}

/// Trait for receiving sync progress updates.
/// Implemented by the Tauri layer to emit events to the frontend.
#[async_trait::async_trait]
pub trait ProgressSink: Send + Sync {
    fn emit_progress(&self, phase: &str, page: u32, message: &str);
    fn emit_items(&self, items: Vec<crate::models::item::Model>);
    fn emit_complete(&self, total: usize);
    fn emit_error(&self, error: &str, retry_in: Option<u64>);
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Token resolution failed: {0}")]
    TokenResolution(String),
    #[error("Platform API error: {0}")]
    PlatformApi(String),
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("Sync error: {0}")]
    Sync(String),
}

fn parse_datetime(raw: &str, fallback: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str(
        &raw.replace('T', " ").replace('Z', ""),
        "%Y-%m-%d %H:%M:%S",
    )
    .unwrap_or_else(|_| {
        tracing::warn!(raw = %raw, "Failed to parse datetime, using fallback");
        fallback
    })
}

// ---------------------------------------------------------------------------
// PlatformSync trait
// ---------------------------------------------------------------------------

/// Abstracts platform-specific sync logic (GitHub, GitLab, etc.) behind a
/// common interface so the orchestration loop can be shared.
#[async_trait::async_trait]
pub trait PlatformSync: Send + Sync {
    /// Fetch a single page of issues using cursor-based pagination.
    /// Returns `(items, has_next_page, next_cursor)`.
    async fn fetch_issues_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error>;

    /// Fetch a single page of pull requests / merge requests using cursor-based pagination.
    /// Returns `(items, has_next_page, next_cursor)`.
    async fn fetch_prs_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error>;

    /// Fetch a single page of discussions using cursor-based pagination.
    /// Returns `(items, has_next_page, next_cursor)`.
    /// Platforms that do not support discussions return an empty vec with
    /// `has_next_page = false`.
    async fn fetch_discussions_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error>;

    /// Called once before sync starts for any platform-specific
    /// initialisation (e.g. GitLab project-ID resolution).
    async fn init(&mut self, db: &DatabaseConnection, proj: &project::Model) -> Result<(), Error>;
}

// ---------------------------------------------------------------------------
// GitHubPlatformSync
// ---------------------------------------------------------------------------

pub struct GitHubPlatformSync {
    client: GitHubClient,
    owner: String,
    name: String,
}

impl GitHubPlatformSync {
    pub fn new(client: GitHubClient, owner: &str, name: &str) -> Self {
        Self {
            client,
            owner: owner.to_string(),
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl PlatformSync for GitHubPlatformSync {
    async fn init(
        &mut self,
        _db: &DatabaseConnection,
        _proj: &project::Model,
    ) -> Result<(), Error> {
        // GitHub needs no pre-sync init beyond default-branch detection,
        // which is handled in sync_github_items before calling the unified loop.
        Ok(())
    }

    async fn fetch_issues_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
        let now = Utc::now().naive_utc();

        let (issues, has_next_page, end_cursor) = self
            .client
            .fetch_issues_page(&self.owner, &self.name, cursor, since)
            .await
            .map_err(|e| Error::PlatformApi(e.to_string()))?;

        let new_items: Vec<sync::NewItem> = issues
            .iter()
            .map(|issue| {
                let author = issue
                    .author
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_else(|| "ghost".to_string());
                sync::NewItem {
                    external_id: issue.number,
                    item_type: ItemType::Issue,
                    title: issue.title.clone(),
                    body: issue.body.clone().unwrap_or_default(),
                    state: ItemState::from_github_state(&issue.state, None),
                    author,
                    url: issue.url.clone(),
                    comments_count: issue.comments.total_count,
                    pr_branch: None,
                    labels: Vec::new(),
                    created_at: parse_datetime(&issue.created_at, now),
                    updated_at: parse_datetime(&issue.updated_at, now),
                }
            })
            .collect();

        Ok((new_items, has_next_page, end_cursor))
    }

    async fn fetch_prs_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
        let now = Utc::now().naive_utc();

        let (prs, has_next_page, end_cursor) = self
            .client
            .fetch_pull_requests_page(&self.owner, &self.name, cursor, since)
            .await
            .map_err(|e| Error::PlatformApi(e.to_string()))?;

        let mut hit_cutoff = false;
        let new_items: Vec<sync::NewItem> = prs
            .iter()
            .filter(|pr| {
                if let Some(since_val) = since {
                    if pr.updated_at.as_str() < since_val {
                        hit_cutoff = true;
                        return false;
                    }
                }
                true
            })
            .map(|pr| {
                let author = pr
                    .author
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_else(|| "ghost".to_string());
                sync::NewItem {
                    external_id: pr.number,
                    item_type: ItemType::PullRequest,
                    title: pr.title.clone(),
                    body: pr.body.clone().unwrap_or_default(),
                    state: ItemState::from_github_state(&pr.state, None),
                    author,
                    url: pr.url.clone(),
                    comments_count: pr.comments.total_count,
                    pr_branch: Some(pr.head_ref_name.clone()),
                    labels: Vec::new(),
                    created_at: parse_datetime(&pr.created_at, now),
                    updated_at: parse_datetime(&pr.updated_at, now),
                }
            })
            .collect();

        let effective_has_next = has_next_page && !hit_cutoff;
        Ok((new_items, effective_has_next, end_cursor))
    }

    async fn fetch_discussions_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
        let now = Utc::now().naive_utc();

        let (discussions, has_next_page, end_cursor) = self
            .client
            .fetch_discussions_page(&self.owner, &self.name, cursor)
            .await
            .map_err(|e| Error::PlatformApi(e.to_string()))?;

        let mut hit_cutoff = false;
        let new_items: Vec<sync::NewItem> = discussions
            .iter()
            .filter(|d| {
                if let Some(since_val) = since {
                    if d.updated_at.as_str() < since_val {
                        hit_cutoff = true;
                        return false;
                    }
                }
                true
            })
            .map(|d| {
                let author = d
                    .author
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_else(|| "ghost".to_string());
                let state = if d.closed {
                    ItemState::Closed
                } else {
                    ItemState::Open
                };
                sync::NewItem {
                    external_id: d.number,
                    item_type: ItemType::Discussion,
                    title: d.title.clone(),
                    body: d.body.clone().unwrap_or_default(),
                    state,
                    author,
                    url: d.url.clone(),
                    comments_count: d.comments.total_count,
                    pr_branch: None,
                    labels: Vec::new(),
                    created_at: parse_datetime(&d.created_at, now),
                    updated_at: parse_datetime(&d.updated_at, now),
                }
            })
            .collect();

        let effective_has_next = has_next_page && !hit_cutoff;
        Ok((new_items, effective_has_next, end_cursor))
    }
}

// ---------------------------------------------------------------------------
// GitLabPlatformSync
// ---------------------------------------------------------------------------

pub struct GitLabPlatformSync {
    client: GitLabClient,
    owner: String,
    name: String,
    gitlab_project_id: Option<i64>,
}

impl GitLabPlatformSync {
    pub fn new(client: GitLabClient, owner: &str, name: &str) -> Self {
        Self {
            client,
            owner: owner.to_string(),
            name: name.to_string(),
            gitlab_project_id: None,
        }
    }

    fn project_id(&self) -> i64 {
        self.gitlab_project_id
            .expect("GitLabPlatformSync::init must be called before fetching")
    }
}

#[async_trait::async_trait]
impl PlatformSync for GitLabPlatformSync {
    async fn init(&mut self, db: &DatabaseConnection, proj: &project::Model) -> Result<(), Error> {
        let gitlab_project_id = if let Some(ext_id) = proj.external_project_id {
            tracing::debug!(
                project_id = %proj.id,
                external_project_id = ext_id,
                "Using cached GitLab project ID"
            );
            ext_id
        } else {
            let path = format!("{}/{}", self.owner, self.name);
            let ext_id = self.client.get_project_id(&path).await.map_err(|e| {
                tracing::error!(
                    project_id = %proj.id,
                    path = %path,
                    error = %e,
                    "Failed to resolve GitLab project ID"
                );
                Error::PlatformApi(e.to_string())
            })?;

            let mut active: project::ActiveModel = proj.clone().into();
            active.external_project_id = Set(Some(ext_id));
            active.update(db).await?;

            tracing::info!(
                project_id = %proj.id,
                external_project_id = ext_id,
                "Cached GitLab project ID"
            );
            ext_id
        };
        self.gitlab_project_id = Some(gitlab_project_id);
        Ok(())
    }

    async fn fetch_issues_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
        let now = Utc::now().naive_utc();
        let pid = self.project_id();
        let page: u32 = cursor.and_then(|c| c.parse().ok()).unwrap_or(1);

        let (issues, has_more) = self
            .client
            .fetch_issues_page(pid, page, since)
            .await
            .map_err(|e| Error::PlatformApi(e.to_string()))?;

        let new_items: Vec<sync::NewItem> = issues
            .iter()
            .map(|issue| sync::NewItem {
                external_id: issue.iid,
                item_type: ItemType::Issue,
                title: issue.title.clone(),
                body: issue.description.clone().unwrap_or_default(),
                state: ItemState::from_gitlab_state(&issue.state),
                author: issue.author.username.clone(),
                url: issue.web_url.clone(),
                comments_count: issue.user_notes_count,
                pr_branch: None,
                labels: Vec::new(),
                created_at: parse_datetime(&issue.created_at, now),
                updated_at: parse_datetime(&issue.updated_at, now),
            })
            .collect();

        let next_cursor = if has_more {
            Some((page + 1).to_string())
        } else {
            None
        };
        Ok((new_items, has_more, next_cursor))
    }

    async fn fetch_prs_page(
        &self,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
        let now = Utc::now().naive_utc();
        let pid = self.project_id();
        let page: u32 = cursor.and_then(|c| c.parse().ok()).unwrap_or(1);

        let (mrs, has_more) = self
            .client
            .fetch_merge_requests_page(pid, page, since)
            .await
            .map_err(|e| Error::PlatformApi(e.to_string()))?;

        let new_items: Vec<sync::NewItem> = mrs
            .iter()
            .map(|mr| sync::NewItem {
                external_id: mr.iid,
                item_type: ItemType::PullRequest,
                title: mr.title.clone(),
                body: mr.description.clone().unwrap_or_default(),
                state: ItemState::from_gitlab_state(&mr.state),
                author: mr.author.username.clone(),
                url: mr.web_url.clone(),
                comments_count: mr.user_notes_count,
                pr_branch: Some(mr.source_branch.clone()),
                labels: Vec::new(),
                created_at: parse_datetime(&mr.created_at, now),
                updated_at: parse_datetime(&mr.updated_at, now),
            })
            .collect();

        let next_cursor = if has_more {
            Some((page + 1).to_string())
        } else {
            None
        };
        Ok((new_items, has_more, next_cursor))
    }

    async fn fetch_discussions_page(
        &self,
        _cursor: Option<&str>,
        _since: Option<&str>,
    ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
        // GitLab does not have a discussions concept equivalent to GitHub.
        Ok((Vec::new(), false, None))
    }
}

// ---------------------------------------------------------------------------
// Unified sync orchestration
// ---------------------------------------------------------------------------

/// Runs the common fetch-upsert-reconcile loop for any platform that
/// implements [`PlatformSync`].
pub async fn sync_platform_items(
    db: &DatabaseConnection,
    proj: &project::Model,
    platform: &dyn PlatformSync,
    progress: &dyn ProgressSink,
    is_full_reconciliation: bool,
    config: &SyncConfig,
) -> Result<usize, Error> {
    let mut total_synced: usize = 0;

    let mut items_index = sync::load_items_index(db, &proj.id).await?;

    let since_issues = compute_since(
        is_full_reconciliation,
        proj.last_sync_at,
        config.sync_from_date_issues.as_deref(),
    );
    let since_prs = compute_since(
        is_full_reconciliation,
        proj.last_sync_at,
        config.sync_from_date_prs.as_deref(),
    );
    let since_discussions = compute_since(
        is_full_reconciliation,
        proj.last_sync_at,
        config.sync_from_date_discussions.as_deref(),
    );

    let mut all_fetched_issue_ids: Vec<i32> = Vec::new();
    let mut all_fetched_pr_ids: Vec<i32> = Vec::new();
    let mut all_fetched_discussion_ids: Vec<i32> = Vec::new();

    // === Phase: Issues ===
    if config.sync_issues {
        let mut cursor: Option<String> = None;
        let mut issue_page = 1u32;
        loop {
            progress.emit_progress(
                "issues",
                issue_page,
                &format!(
                    "{}/{}: Fetching issues page {issue_page}...",
                    proj.owner, proj.name
                ),
            );

            let (items, has_next_page, end_cursor) = platform
                .fetch_issues_page(cursor.as_deref(), since_issues.as_deref())
                .await?;

            for item in &items {
                all_fetched_issue_ids.push(item.external_id);
            }

            if !items.is_empty() {
                let saved =
                    sync::upsert_items_batch(db, &proj.id, &mut items_index, items).await?;
                total_synced += saved.len();
                progress.emit_items(saved);
            }

            if !has_next_page {
                break;
            }
            cursor = end_cursor;
            issue_page += 1;
        }
        tracing::info!(project_id = %proj.id, "Issues phase completed");
    } else {
        tracing::info!(project_id = %proj.id, "Issues phase skipped (disabled in config)");
    }

    // === Phase: PRs / Merge Requests ===
    if config.sync_prs {
        let mut cursor: Option<String> = None;
        let mut pr_page = 1u32;
        loop {
            progress.emit_progress(
                "prs",
                pr_page,
                &format!(
                    "{}/{}: Fetching PRs page {pr_page}...",
                    proj.owner, proj.name
                ),
            );

            let (items, has_next_page, end_cursor) = platform
                .fetch_prs_page(cursor.as_deref(), since_prs.as_deref())
                .await?;

            for item in &items {
                all_fetched_pr_ids.push(item.external_id);
            }

            if !items.is_empty() {
                let saved =
                    sync::upsert_items_batch(db, &proj.id, &mut items_index, items).await?;
                total_synced += saved.len();
                progress.emit_items(saved);
            }

            if !has_next_page {
                break;
            }
            cursor = end_cursor;
            pr_page += 1;
        }
        tracing::info!(project_id = %proj.id, "PRs phase completed");
    } else {
        tracing::info!(project_id = %proj.id, "PRs phase skipped (disabled in config)");
    }

    // === Phase: Discussions ===
    if config.sync_discussions {
        let mut cursor: Option<String> = None;
        let mut disc_page = 1u32;
        loop {
            progress.emit_progress(
                "discussions",
                disc_page,
                &format!(
                    "{}/{}: Fetching discussions page {disc_page}...",
                    proj.owner, proj.name
                ),
            );

            let (items, has_next_page, end_cursor) = platform
                .fetch_discussions_page(cursor.as_deref(), since_discussions.as_deref())
                .await?;

            // If the very first page is empty and there is no next page, skip
            // the phase entirely (e.g. GitLab which has no discussions).
            if items.is_empty() && !has_next_page {
                break;
            }

            for item in &items {
                all_fetched_discussion_ids.push(item.external_id);
            }

            if !items.is_empty() {
                let saved =
                    sync::upsert_items_batch(db, &proj.id, &mut items_index, items).await?;
                total_synced += saved.len();
                progress.emit_items(saved);
            }

            if !has_next_page {
                break;
            }
            cursor = end_cursor;
            disc_page += 1;
        }
        tracing::info!(project_id = %proj.id, "Discussions phase completed");
    } else {
        tracing::info!(project_id = %proj.id, "Discussions phase skipped (disabled in config)");
    }

    // === Full Reconciliation: mark absent items as closed ===
    if is_full_reconciliation {
        if config.sync_issues {
            sync::mark_absent_items_closed(
                db,
                &proj.id,
                &all_fetched_issue_ids,
                &ItemType::Issue,
            )
            .await?;
        }

        if config.sync_prs {
            sync::mark_absent_items_closed(
                db,
                &proj.id,
                &all_fetched_pr_ids,
                &ItemType::PullRequest,
            )
            .await?;
        }

        if config.sync_discussions && !all_fetched_discussion_ids.is_empty() {
            sync::mark_absent_items_closed(
                db,
                &proj.id,
                &all_fetched_discussion_ids,
                &ItemType::Discussion,
            )
            .await?;
        }

        sync::update_reconciliation_timestamp(db, proj).await?;
    }

    // Ensure any item with JSON state=closed/merged gets item_status='resolved'.
    // This catches items that were closed in a previous sync but whose
    // status was reset (e.g. by a full sync reset).
    sync::deactivate_closed_items(db, &proj.id).await?;

    // Advance sync timestamp only after all phases complete successfully,
    // so a partial failure doesn't skip items on the next incremental sync.
    sync::advance_sync_timestamp(db, proj).await?;

    tracing::info!(project_id = %proj.id, total_synced = total_synced, "Platform sync completed");
    Ok(total_synced)
}

// ---------------------------------------------------------------------------
// Public entry points (unchanged signatures)
// ---------------------------------------------------------------------------

pub async fn sync_github_items(
    db: &DatabaseConnection,
    proj: &project::Model,
    token: &str,
    progress: &dyn ProgressSink,
    is_full_reconciliation: bool,
    config: &SyncConfig,
) -> Result<usize, Error> {
    let base_url = crate::services::auth::get_project_base_url(db, proj).await;
    let client = GitHubClient::with_base_url(token.to_string(), base_url);

    // Detect and store default branch if not already set
    if proj.default_branch.is_none() {
        match client.get_repo(&proj.owner, &proj.name).await {
            Ok(repo_info) => {
                if let Some(ref branch) = repo_info.default_branch {
                    tracing::info!(
                        project_id = %proj.id,
                        default_branch = %branch,
                        "Detected default branch from GitHub API"
                    );
                    let mut active: project::ActiveModel = proj.clone().into();
                    active.default_branch = Set(Some(branch.clone()));
                    active.update(db).await?;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to detect default branch, will use fallback");
            }
        }
    }

    let mut platform = GitHubPlatformSync::new(client, &proj.owner, &proj.name);
    platform.init(db, proj).await?;
    sync_platform_items(db, proj, &platform, progress, is_full_reconciliation, config).await
}

pub async fn sync_gitlab_items(
    db: &DatabaseConnection,
    proj: &project::Model,
    token: &str,
    base_url: Option<String>,
    progress: &dyn ProgressSink,
    is_full_reconciliation: bool,
    config: &SyncConfig,
) -> Result<usize, Error> {
    let client = GitLabClient::new(token.to_string(), base_url);
    let mut platform = GitLabPlatformSync::new(client, &proj.owner, &proj.name);
    platform.init(db, proj).await?;
    sync_platform_items(db, proj, &platform, progress, is_full_reconciliation, config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{ItemState, ItemStatus};
    use crate::models::item;
    use crate::sync;
    use crate::test_helpers::{dt, setup_test_db, ItemFactory, ProjectFactory};
    use sea_orm::EntityTrait;
    use std::sync::atomic::{AtomicBool, Ordering};

    // -----------------------------------------------------------------------
    // 6a: Unit tests for compute_since
    // -----------------------------------------------------------------------

    #[test]
    fn compute_since_full_recon_no_sync_from_date() {
        assert_eq!(compute_since(true, None, None), None);
    }

    #[test]
    fn compute_since_full_recon_with_sync_from_date() {
        assert_eq!(
            compute_since(true, None, Some("2025-06-01T00:00:00Z")),
            Some("2025-06-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn compute_since_full_recon_ignores_last_sync_at() {
        let last = dt("2025-08-01 00:00:00");
        assert_eq!(
            compute_since(true, Some(last), Some("2025-06-01T00:00:00Z")),
            Some("2025-06-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn compute_since_incremental_last_sync_newer() {
        let last = dt("2025-08-01 00:00:00");
        assert_eq!(
            compute_since(false, Some(last), Some("2025-06-01T00:00:00Z")),
            Some("2025-08-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn compute_since_incremental_last_sync_older() {
        let last = dt("2025-01-01 00:00:00");
        assert_eq!(
            compute_since(false, Some(last), Some("2025-06-01T00:00:00Z")),
            Some("2025-06-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn compute_since_incremental_no_last_sync_with_from_date() {
        assert_eq!(
            compute_since(false, None, Some("2025-06-01T00:00:00Z")),
            Some("2025-06-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn compute_since_incremental_last_sync_no_from_date() {
        let last = dt("2025-08-01 00:00:00");
        assert_eq!(
            compute_since(false, Some(last), None),
            Some("2025-08-01T00:00:00Z".to_string())
        );
    }

    #[test]
    fn compute_since_incremental_no_last_sync_no_from_date() {
        assert_eq!(compute_since(false, None, None), None);
    }

    // -----------------------------------------------------------------------
    // 6b: Integration tests for phase skipping
    // -----------------------------------------------------------------------

    /// Mock PlatformSync that tracks which methods were called.
    struct MockPlatform {
        issues_called: AtomicBool,
        prs_called: AtomicBool,
        discussions_called: AtomicBool,
    }

    impl MockPlatform {
        fn new() -> Self {
            Self {
                issues_called: AtomicBool::new(false),
                prs_called: AtomicBool::new(false),
                discussions_called: AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl PlatformSync for MockPlatform {
        async fn init(
            &mut self,
            _db: &DatabaseConnection,
            _proj: &project::Model,
        ) -> Result<(), Error> {
            Ok(())
        }

        async fn fetch_issues_page(
            &self,
            _cursor: Option<&str>,
            _since: Option<&str>,
        ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
            self.issues_called.store(true, Ordering::SeqCst);
            Ok((Vec::new(), false, None))
        }

        async fn fetch_prs_page(
            &self,
            _cursor: Option<&str>,
            _since: Option<&str>,
        ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
            self.prs_called.store(true, Ordering::SeqCst);
            Ok((Vec::new(), false, None))
        }

        async fn fetch_discussions_page(
            &self,
            _cursor: Option<&str>,
            _since: Option<&str>,
        ) -> Result<(Vec<sync::NewItem>, bool, Option<String>), Error> {
            self.discussions_called.store(true, Ordering::SeqCst);
            Ok((Vec::new(), false, None))
        }
    }

    struct NoopProgress;

    #[async_trait::async_trait]
    impl ProgressSink for NoopProgress {
        fn emit_progress(&self, _phase: &str, _page: u32, _message: &str) {}
        fn emit_items(&self, _items: Vec<crate::models::item::Model>) {}
        fn emit_complete(&self, _total: usize) {}
        fn emit_error(&self, _error: &str, _retry_in: Option<u64>) {}
    }

    #[tokio::test]
    async fn sync_issues_disabled_skips_issues_phase() {
        let db = setup_test_db().await;
        let proj = ProjectFactory::default().create(&db).await;
        let platform = MockPlatform::new();
        let config = SyncConfig {
            sync_issues: false,
            ..Default::default()
        };

        sync_platform_items(&db, &proj, &platform, &NoopProgress, false, &config)
            .await
            .unwrap();

        assert!(!platform.issues_called.load(Ordering::SeqCst));
        assert!(platform.prs_called.load(Ordering::SeqCst));
        assert!(platform.discussions_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn sync_prs_disabled_skips_prs_phase() {
        let db = setup_test_db().await;
        let proj = ProjectFactory::default().create(&db).await;
        let platform = MockPlatform::new();
        let config = SyncConfig {
            sync_prs: false,
            ..Default::default()
        };

        sync_platform_items(&db, &proj, &platform, &NoopProgress, false, &config)
            .await
            .unwrap();

        assert!(platform.issues_called.load(Ordering::SeqCst));
        assert!(!platform.prs_called.load(Ordering::SeqCst));
        assert!(platform.discussions_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn sync_discussions_disabled_skips_discussions_phase() {
        let db = setup_test_db().await;
        let proj = ProjectFactory::default().create(&db).await;
        let platform = MockPlatform::new();
        let config = SyncConfig {
            sync_discussions: false,
            ..Default::default()
        };

        sync_platform_items(&db, &proj, &platform, &NoopProgress, false, &config)
            .await
            .unwrap();

        assert!(platform.issues_called.load(Ordering::SeqCst));
        assert!(platform.prs_called.load(Ordering::SeqCst));
        assert!(!platform.discussions_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn sync_all_enabled_calls_all_phases() {
        let db = setup_test_db().await;
        let proj = ProjectFactory::default().create(&db).await;
        let platform = MockPlatform::new();
        let config = SyncConfig::default();

        sync_platform_items(&db, &proj, &platform, &NoopProgress, false, &config)
            .await
            .unwrap();

        assert!(platform.issues_called.load(Ordering::SeqCst));
        assert!(platform.prs_called.load(Ordering::SeqCst));
        assert!(platform.discussions_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn disabled_type_full_recon_preserves_existing_items() {
        let db = setup_test_db().await;
        let proj = ProjectFactory::default().create(&db).await;

        // Create an existing open issue
        let existing = ItemFactory::new(&proj.id, 42)
            .item_type(ItemType::Issue)
            .state(ItemState::Open)
            .create(&db)
            .await;

        let platform = MockPlatform::new();
        let config = SyncConfig {
            sync_issues: false,
            ..Default::default()
        };

        // Full reconciliation with issues disabled — should NOT mark existing issues closed
        sync_platform_items(&db, &proj, &platform, &NoopProgress, true, &config)
            .await
            .unwrap();

        assert!(!platform.issues_called.load(Ordering::SeqCst));
        let reloaded = item::Entity::find_by_id(&existing.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.item_status, ItemStatus::Pending);
    }
}
