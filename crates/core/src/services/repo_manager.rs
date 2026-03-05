use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::enums::{ItemType, Platform};
use crate::services::git::GitService;

const FETCH_COOLDOWN: Duration = Duration::from_secs(30);

/// Injectable cache that tracks when each repo was last fetched,
/// replacing the former `static FETCH_CACHE`.
#[derive(Debug)]
pub struct FetchCache {
    inner: Mutex<HashMap<PathBuf, Instant>>,
}

impl Default for FetchCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FetchCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if the repo should be fetched (either forced, never
    /// fetched, or the cooldown has elapsed).
    fn should_fetch(&self, repo_path: &Path, force: bool) -> bool {
        if force {
            return true;
        }
        let cache = self.inner.lock().unwrap();
        match cache.get(repo_path) {
            Some(last_fetch) => last_fetch.elapsed() >= FETCH_COOLDOWN,
            None => true,
        }
    }

    /// Record that a fetch just happened for `repo_path`.
    fn record_fetch(&self, repo_path: &Path) {
        let mut cache = self.inner.lock().unwrap();
        cache.insert(repo_path.to_path_buf(), Instant::now());
    }

    /// Clear the entire fetch timestamp cache (e.g. when clearing repo cache).
    pub fn invalidate_all(&self) {
        let mut cache = self.inner.lock().unwrap();
        cache.clear();
    }

    /// Invalidate the fetch cache for a specific repo.
    pub fn invalidate_repo(&self, repo_path: &Path) {
        let mut cache = self.inner.lock().unwrap();
        cache.remove(repo_path);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Git(#[from] crate::services::git::Error),

    #[error("Failed to read file: {0}")]
    ReadFile(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Default)]
pub struct ProjectFiles {
    pub contributing: Option<String>,
    pub pr_template: Option<String>,
    pub readme_excerpt: Option<String>,
}

/// Result of creating an analysis worktree.
#[derive(Debug, Clone)]
pub struct AnalysisWorktree {
    /// Path to the worktree directory.
    pub worktree_path: PathBuf,
    /// Path to the base repo (for cleanup).
    pub repo_path: PathBuf,
}

pub struct RepoManager {
    fetch_cache: FetchCache,
}

impl Default for RepoManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoManager {
    pub fn new() -> Self {
        Self {
            fetch_cache: FetchCache::new(),
        }
    }

    /// Access the underlying [`FetchCache`].
    pub fn fetch_cache(&self) -> &FetchCache {
        &self.fetch_cache
    }

    /// Ensure the repo is cloned and fetched. Does NOT check out any branch.
    /// This is the first phase: acquire lock, clone/fetch, release lock.
    /// Returns the repo path.
    #[tracing::instrument(skip(self, token))]
    pub fn ensure_fetched(
        &self,
        platform: &Platform,
        owner: &str,
        name: &str,
        url: &str,
        token: &str,
        force_fetch: bool,
    ) -> Result<PathBuf> {
        let repo_path = GitService::repo_path(platform, owner, name, None)?;

        if !GitService::is_cloned(&repo_path) {
            tracing::info!(path = %repo_path.display(), url = %url, owner = %owner, name = %name, "Shallow cloning repo for first time");
            match GitService::shallow_clone(url, &repo_path, token) {
                Ok(()) => {
                    self.fetch_cache.record_fetch(&repo_path);
                }
                Err(e) => {
                    tracing::warn!(error = %e, url = %url, path = %repo_path.display(), owner = %owner, name = %name, "Shallow clone failed, falling back to full clone");
                    GitService::clone_repo(url, &repo_path, token)?;
                    self.fetch_cache.record_fetch(&repo_path);
                }
            }
        } else if self.fetch_cache.should_fetch(&repo_path, force_fetch) {
            tracing::info!(path = %repo_path.display(), owner = %owner, name = %name, "Fetching latest for existing clone");
            if let Err(e) = GitService::fetch_repo(&repo_path, token) {
                tracing::warn!(error = %e, path = %repo_path.display(), url = %url, owner = %owner, name = %name, "Fetch failed, repo may be corrupt — re-cloning");
                let _ = std::fs::remove_dir_all(&repo_path);
                match GitService::shallow_clone(url, &repo_path, token) {
                    Ok(()) => {}
                    Err(e2) => {
                        tracing::warn!(error = %e2, url = %url, path = %repo_path.display(), owner = %owner, name = %name, "Shallow re-clone failed, trying full clone");
                        GitService::clone_repo(url, &repo_path, token)?;
                    }
                }
            }
            self.fetch_cache.record_fetch(&repo_path);
        } else {
            tracing::info!(path = %repo_path.display(), owner = %owner, name = %name, "Skipping fetch (within cooldown)");
        }

        Ok(repo_path)
    }

    /// Create an isolated worktree for analysis.
    /// For PRs: fetches PR ref and creates worktree at that commit.
    /// For issues/discussions: creates worktree at the default branch.
    #[tracing::instrument(skip(token))]
    pub fn create_analysis_worktree(
        repo_path: &Path,
        item_type: &ItemType,
        pr_number: Option<i32>,
        default_branch: Option<&str>,
        token: &str,
    ) -> Result<AnalysisWorktree> {
        let worktree_id = uuid::Uuid::new_v4().to_string();
        let worktrees_dir = repo_path.join(".worktrees");
        let worktree_path = worktrees_dir.join(format!("analysis-{worktree_id}"));

        // Determine the git ref to check out
        let git_ref = match item_type {
            ItemType::PullRequest => {
                if let Some(pr_num) = pr_number {
                    tracing::info!(
                        pr_number = pr_num,
                        repo_path = %repo_path.display(),
                        "Fetching and resolving PR ref for worktree"
                    );
                    // Fetch the PR ref first
                    match GitService::fetch_pr_ref(repo_path, pr_num, token) {
                        Ok(ref_name) => ref_name,
                        Err(e) => {
                            tracing::warn!(error = %e, pr_number = pr_num, repo_path = %repo_path.display(), "Failed to fetch PR ref via shell, trying git2");
                            // Fallback to git2
                            GitService::fetch_pr_branch(repo_path, pr_num, token)?;
                            format!("refs/remotes/origin/pr-{pr_num}")
                        }
                    }
                } else {
                    let branch = default_branch.unwrap_or("main");
                    format!("refs/remotes/origin/{branch}")
                }
            }
            ItemType::Issue | ItemType::Discussion | ItemType::Note => {
                let branch = default_branch.unwrap_or("main");
                format!("refs/remotes/origin/{branch}")
            }
        };

        // Create the worktree
        GitService::worktree_add(repo_path, &worktree_path, &git_ref)?;

        tracing::info!(
            worktree = %worktree_path.display(),
            git_ref = %git_ref,
            repo_path = %repo_path.display(),
            item_type = ?item_type,
            "Analysis worktree created"
        );

        Ok(AnalysisWorktree {
            worktree_path,
            repo_path: repo_path.to_path_buf(),
        })
    }

    /// Clean up an analysis worktree.
    #[tracing::instrument(skip(worktree), fields(worktree_path = %worktree.worktree_path.display()))]
    pub fn cleanup_worktree(worktree: &AnalysisWorktree) {
        if let Err(e) = GitService::worktree_remove(&worktree.repo_path, &worktree.worktree_path) {
            tracing::warn!(
                error = %e,
                worktree = %worktree.worktree_path.display(),
                repo_path = %worktree.repo_path.display(),
                "Failed to remove worktree via git, cleaning up manually"
            );
            if worktree.worktree_path.exists() {
                let _ = std::fs::remove_dir_all(&worktree.worktree_path);
            }
            let _ = GitService::worktree_prune(&worktree.repo_path);
        }
    }

    /// Clean up stale worktrees from previous crashes.
    /// Should be called on app startup.
    #[tracing::instrument]
    pub fn cleanup_stale_worktrees(platform: &Platform, owner: &str, name: &str) {
        let repo_path = match GitService::repo_path(platform, owner, name, None) {
            Ok(p) => p,
            Err(_) => return,
        };

        if !GitService::is_cloned(&repo_path) {
            return;
        }

        let worktrees_dir = repo_path.join(".worktrees");
        if !worktrees_dir.exists() {
            return;
        }

        // Remove all analysis worktree directories
        if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with("analysis-") {
                        tracing::info!(worktree = %path.display(), repo_path = %repo_path.display(), "Removing stale analysis worktree");
                        let _ = GitService::worktree_remove(&repo_path, &path);
                    }
                }
            }
        }

        // Prune any remaining stale entries
        let _ = GitService::worktree_prune(&repo_path);
    }

    /// Legacy ensure_ready for backwards compatibility.
    /// Clones/fetches and checks out the appropriate branch.
    #[allow(clippy::too_many_arguments)]
    pub fn ensure_ready(
        &self,
        platform: &Platform,
        owner: &str,
        name: &str,
        url: &str,
        token: &str,
        item_type: &ItemType,
        pr_number: Option<i32>,
        default_branch: Option<&str>,
    ) -> Result<PathBuf> {
        let repo_path = self.ensure_fetched(platform, owner, name, url, token, false)?;

        match item_type {
            ItemType::PullRequest => {
                if let Some(pr_num) = pr_number {
                    tracing::info!(pr_number = pr_num, repo_path = %repo_path.display(), "Checking out PR branch");
                    GitService::fetch_pr_branch(&repo_path, pr_num, token)?;
                }
            }
            ItemType::Issue | ItemType::Discussion | ItemType::Note => {
                let branch = default_branch.unwrap_or("main");
                tracing::info!(branch, repo_path = %repo_path.display(), "Checking out default branch");
                GitService::checkout_branch(&repo_path, branch)?;
            }
        }

        Ok(repo_path)
    }

    /// Read a file from the repo clone if it exists.
    pub fn read_file(repo_path: &Path, relative_path: &str) -> Result<Option<String>> {
        let file_path = repo_path.join(relative_path);
        if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Read common project files for context (CONTRIBUTING.md, PR templates, etc.).
    pub fn read_project_context(repo_path: &Path) -> ProjectFiles {
        let contributing = Self::read_file(repo_path, "CONTRIBUTING.md").ok().flatten();
        let pr_template = Self::read_file(repo_path, ".github/PULL_REQUEST_TEMPLATE.md")
            .ok()
            .flatten()
            .or_else(|| {
                Self::read_file(repo_path, ".github/pull_request_template.md")
                    .ok()
                    .flatten()
            });
        let readme_excerpt = Self::read_file(repo_path, "README.md")
            .ok()
            .flatten()
            .map(|content| content.chars().take(2000).collect::<String>());

        ProjectFiles {
            contributing,
            pr_template,
            readme_excerpt,
        }
    }

    /// Read project context files from a specific git ref (e.g. the default branch).
    /// Falls back to disk reading if the ref is not found.
    pub fn read_project_context_from_ref(repo_path: &Path, default_branch: &str) -> ProjectFiles {
        let ref_name = format!("refs/remotes/origin/{default_branch}");

        let contributing = GitService::read_file_from_ref(repo_path, &ref_name, "CONTRIBUTING.md")
            .ok()
            .flatten()
            .or_else(|| Self::read_file(repo_path, "CONTRIBUTING.md").ok().flatten());

        let pr_template = GitService::read_file_from_ref(
            repo_path,
            &ref_name,
            ".github/PULL_REQUEST_TEMPLATE.md",
        )
        .ok()
        .flatten()
        .or_else(|| {
            GitService::read_file_from_ref(repo_path, &ref_name, ".github/pull_request_template.md")
                .ok()
                .flatten()
        })
        .or_else(|| {
            Self::read_file(repo_path, ".github/PULL_REQUEST_TEMPLATE.md")
                .ok()
                .flatten()
                .or_else(|| {
                    Self::read_file(repo_path, ".github/pull_request_template.md")
                        .ok()
                        .flatten()
                })
        });

        let readme_excerpt = GitService::read_file_from_ref(repo_path, &ref_name, "README.md")
            .ok()
            .flatten()
            .or_else(|| Self::read_file(repo_path, "README.md").ok().flatten())
            .map(|content| content.chars().take(2000).collect::<String>());

        ProjectFiles {
            contributing,
            pr_template,
            readme_excerpt,
        }
    }

    /// Get disk usage of a repo clone in bytes.
    pub fn disk_usage(repo_path: &Path) -> u64 {
        if !repo_path.exists() {
            return 0;
        }
        walkdir::WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    /// Delete a repo clone.
    pub fn delete_clone(repo_path: &Path) -> Result<()> {
        if repo_path.exists() {
            std::fs::remove_dir_all(repo_path)?;
        }
        Ok(())
    }
}
