use std::path::{Path, PathBuf};
use std::process::Command;

use crate::enums::Platform;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not find data directory")]
    DataDirectoryNotFound,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Git(#[from] git2::Error),

    #[error("Git command failed: {0}")]
    GitCommand(String),
}

type Result<T> = std::result::Result<T, Error>;

pub struct GitService;

impl GitService {
    pub fn get_cache_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_dir().ok_or(Error::DataDirectoryNotFound)?;
        let cache_dir = data_dir.join(crate::APP_DIR_NAME).join("repos");
        std::fs::create_dir_all(&cache_dir).map_err(|e| {
            tracing::error!(error = %e, path = %cache_dir.display(), "Failed to create git cache directory");
            e
        })?;
        tracing::debug!(path = %cache_dir.display(), "Git cache directory");
        Ok(cache_dir)
    }

    pub fn repo_path(
        platform: &Platform,
        owner: &str,
        name: &str,
        host: Option<&str>,
    ) -> Result<PathBuf> {
        let cache_dir = Self::get_cache_dir()?;
        let host = host.unwrap_or(match platform {
            Platform::GitHub => "github.com",
            Platform::GitLab => "gitlab.com",
        });
        Ok(cache_dir.join(host).join(owner).join(name))
    }

    #[tracing::instrument(skip(token))]
    pub fn clone_repo(url: &str, path: &Path, token: &str) -> Result<git2::Repository> {
        tracing::info!(url = %url, path = %path.display(), "Cloning repository");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                tracing::error!(error = %e, path = %parent.display(), url = %url, "Failed to create parent directory for clone");
                e
            })?;
        }

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(move |_url, _username, _allowed| {
            git2::Cred::userpass_plaintext("x-access-token", token)
        });

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder.clone(url, path).map_err(|e| {
            tracing::error!(error = %e, url = %url, path = %path.display(), "Clone failed");
            e
        })?;

        Ok(repo)
    }

    #[tracing::instrument(skip(token))]
    pub fn fetch_repo(path: &Path, token: &str) -> Result<()> {
        tracing::info!(path = %path.display(), "Fetching repository");
        let repo = git2::Repository::open(path).map_err(|e| {
            tracing::error!(error = %e, path = %path.display(), "Failed to open repository for fetch");
            e
        })?;

        let mut remote = repo.find_remote("origin").map_err(|e| {
            tracing::error!(error = %e, path = %path.display(), "Failed to find remote 'origin'");
            e
        })?;

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(move |_url, _username, _allowed| {
            git2::Cred::userpass_plaintext("x-access-token", token)
        });

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        remote
            .fetch(
                &["refs/heads/*:refs/remotes/origin/*"],
                Some(&mut fetch_opts),
                None,
            )
            .map_err(|e| {
                tracing::error!(error = %e, path = %path.display(), "Git fetch failed");
                e
            })?;

        Ok(())
    }

    #[tracing::instrument]
    pub fn checkout_branch(path: &Path, branch: &str) -> Result<()> {
        tracing::info!(path = %path.display(), branch = %branch, "Checking out branch");
        let repo = git2::Repository::open(path).map_err(|e| {
            tracing::error!(error = %e, path = %path.display(), branch = %branch, "Failed to open repository for checkout");
            e
        })?;

        // Try the requested branch first
        let ref_name = format!("refs/remotes/origin/{branch}");
        if let Ok(reference) = repo.find_reference(&ref_name) {
            return Self::checkout_ref(&repo, &reference, branch);
        }

        tracing::warn!(branch = %branch, path = %path.display(), "Requested branch not found, trying fallbacks");

        // Try common fallback branch names
        for fallback in &["main", "master", "develop"] {
            if *fallback == branch {
                continue;
            }
            let fallback_ref = format!("refs/remotes/origin/{fallback}");
            if let Ok(reference) = repo.find_reference(&fallback_ref) {
                tracing::info!(requested = %branch, actual = %fallback, path = %path.display(), "Using fallback branch");
                return Self::checkout_ref(&repo, &reference, fallback);
            }
        }

        // Last resort: try to use HEAD as-is (repo was just cloned/fetched)
        tracing::warn!(branch = %branch, path = %path.display(), "No remote branch found, staying on current HEAD");
        Ok(())
    }

    fn checkout_ref(
        repo: &git2::Repository,
        reference: &git2::Reference,
        branch: &str,
    ) -> Result<()> {
        let repo_path = repo.path();
        let commit = reference.peel_to_commit().map_err(|e| {
            tracing::error!(error = %e, branch = %branch, path = %repo_path.display(), "Failed to peel reference to commit");
            e
        })?;

        repo.set_head_detached(commit.id()).map_err(|e| {
            tracing::error!(error = %e, branch = %branch, path = %repo_path.display(), "Failed to set HEAD detached");
            e
        })?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| {
                tracing::error!(error = %e, branch = %branch, path = %repo_path.display(), "Failed to checkout HEAD");
                e
            })?;

        Ok(())
    }

    #[tracing::instrument(skip(token))]
    pub fn fetch_pr_branch(path: &Path, pr_number: i32, token: &str) -> Result<()> {
        tracing::info!(pr_number = pr_number, path = %path.display(), "Fetching PR branch");
        let repo = git2::Repository::open(path).map_err(|e| {
            tracing::error!(error = %e, path = %path.display(), pr_number = pr_number, "Failed to open repository for PR fetch");
            e
        })?;

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| {
                tracing::error!(error = %e, path = %path.display(), pr_number = pr_number, "Failed to find remote 'origin' for PR fetch");
                e
            })?;

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(move |_url, _username, _allowed| {
            git2::Cred::userpass_plaintext("x-access-token", token)
        });

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let refspec = format!("refs/pull/{pr_number}/head:refs/remotes/origin/pr-{pr_number}");
        remote
            .fetch(&[&refspec], Some(&mut fetch_opts), None)
            .map_err(|e| {
                tracing::error!(error = %e, pr_number = pr_number, path = %path.display(), refspec = %refspec, "Failed to fetch PR refspec");
                e
            })?;

        let reference = repo.find_reference(&format!("refs/remotes/origin/pr-{pr_number}")).map_err(|e| {
            tracing::error!(error = %e, pr_number = pr_number, path = %path.display(), "Failed to find PR reference after fetch");
            e
        })?;
        let commit = reference.peel_to_commit().map_err(|e| {
            tracing::error!(error = %e, pr_number = pr_number, path = %path.display(), "Failed to peel PR reference to commit");
            e
        })?;

        repo.set_head_detached(commit.id()).map_err(|e| {
            tracing::error!(error = %e, pr_number = pr_number, path = %path.display(), "Failed to set HEAD detached for PR");
            e
        })?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force())).map_err(|e| {
            tracing::error!(error = %e, pr_number = pr_number, path = %path.display(), "Failed to checkout HEAD for PR");
            e
        })?;

        Ok(())
    }

    /// Read a file from a specific git ref without changing the working tree.
    /// Uses git2 to traverse the tree at the given ref and read the blob.
    pub fn read_file_from_ref(
        path: &Path,
        ref_name: &str,
        file_path: &str,
    ) -> Result<Option<String>> {
        let repo = git2::Repository::open(path).map_err(|e| {
            tracing::error!(error = %e, path = %path.display(), ref_name = %ref_name, file_path = %file_path, "Failed to open repository for ref read");
            e
        })?;

        let reference = match repo.find_reference(ref_name) {
            Ok(r) => r,
            Err(_) => {
                tracing::debug!(ref_name = %ref_name, file_path = %file_path, path = %path.display(), "Reference not found");
                return Ok(None);
            }
        };

        let commit = match reference.peel_to_commit() {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let tree = commit.tree().map_err(|e| {
            tracing::error!(error = %e, path = %path.display(), ref_name = %ref_name, file_path = %file_path, "Failed to get tree from commit");
            e
        })?;

        let entry = match tree.get_path(std::path::Path::new(file_path)) {
            Ok(e) => e,
            Err(_) => return Ok(None), // File doesn't exist at this ref
        };

        let object = entry.to_object(&repo).map_err(|e| {
            tracing::error!(error = %e, file_path = %file_path, path = %path.display(), ref_name = %ref_name, "Failed to get object from tree entry");
            e
        })?;

        match object.as_blob() {
            Some(blob) => match std::str::from_utf8(blob.content()) {
                Ok(content) => Ok(Some(content.to_string())),
                Err(_) => {
                    tracing::debug!(file_path = %file_path, path = %path.display(), ref_name = %ref_name, "File is not valid UTF-8, skipping");
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    pub fn is_cloned(path: &Path) -> bool {
        let result = path.join(".git").exists() && git2::Repository::open(path).is_ok();
        tracing::debug!(path = %path.display(), is_cloned = result, "Checking if repo is cloned");
        result
    }

    pub fn clear_cache(fetch_cache: &super::repo_manager::FetchCache) -> Result<()> {
        tracing::info!("Clearing git cache");
        fetch_cache.invalidate_all();
        let cache_dir = Self::get_cache_dir()?;
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir).map_err(|e| {
                tracing::error!(error = %e, path = %cache_dir.display(), "Failed to remove git cache directory");
                e
            })?;
            std::fs::create_dir_all(&cache_dir).map_err(|e| {
                tracing::error!(error = %e, path = %cache_dir.display(), "Failed to recreate git cache directory");
                e
            })?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Worktree operations (using shell git commands)
    // -----------------------------------------------------------------------

    /// Create a git worktree at the given path, detached at the specified ref.
    pub fn worktree_add(repo_path: &Path, worktree_path: &Path, git_ref: &str) -> Result<()> {
        tracing::info!(
            repo = %repo_path.display(),
            worktree = %worktree_path.display(),
            git_ref = %git_ref,
            "Creating git worktree"
        );

        if let Some(parent) = worktree_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                tracing::error!(error = %e, path = %parent.display(), repo = %repo_path.display(), worktree = %worktree_path.display(), "Failed to create worktree parent directory");
                e
            })?;
        }

        let output = Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(worktree_path)
            .arg(git_ref)
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                tracing::error!(error = %e, repo = %repo_path.display(), worktree = %worktree_path.display(), git_ref = %git_ref, "Failed to run git worktree add");
                Error::GitCommand(format!("Failed to run git worktree add: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(stderr = %stderr, repo = %repo_path.display(), worktree = %worktree_path.display(), git_ref = %git_ref, "git worktree add failed");
            return Err(Error::GitCommand(format!(
                "git worktree add failed: {stderr}"
            )));
        }

        tracing::debug!(worktree = %worktree_path.display(), "Worktree created successfully");
        Ok(())
    }

    /// Remove a git worktree.
    pub fn worktree_remove(repo_path: &Path, worktree_path: &Path) -> Result<()> {
        tracing::info!(
            repo = %repo_path.display(),
            worktree = %worktree_path.display(),
            "Removing git worktree"
        );

        let output = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(worktree_path)
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                tracing::error!(error = %e, repo = %repo_path.display(), worktree = %worktree_path.display(), "Failed to run git worktree remove");
                Error::GitCommand(format!("Failed to run git worktree remove: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(stderr = %stderr, repo = %repo_path.display(), worktree = %worktree_path.display(), "git worktree remove failed, cleaning up manually");
            // Fallback: remove directory manually
            if worktree_path.exists() {
                std::fs::remove_dir_all(worktree_path).map_err(|e| {
                    tracing::error!(error = %e, worktree = %worktree_path.display(), "Failed to manually remove worktree directory");
                    e
                })?;
            }
            // Prune stale worktree entries
            let _ = Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(repo_path)
                .output();
        }

        Ok(())
    }

    /// List all worktrees for a repo (porcelain format).
    /// Returns paths of all worktrees (excluding the main working tree).
    pub fn worktree_list(repo_path: &Path) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                tracing::error!(error = %e, repo = %repo_path.display(), "Failed to run git worktree list");
                Error::GitCommand(format!("Failed to run git worktree list: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::GitCommand(format!(
                "git worktree list failed: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut is_first = true;

        for line in stdout.lines() {
            if let Some(path_str) = line.strip_prefix("worktree ") {
                if is_first {
                    // Skip the main working tree
                    is_first = false;
                    continue;
                }
                worktrees.push(PathBuf::from(path_str));
            }
        }

        Ok(worktrees)
    }

    /// Prune stale worktree entries.
    pub fn worktree_prune(repo_path: &Path) -> Result<()> {
        tracing::debug!(repo = %repo_path.display(), "Pruning stale worktrees");
        let _ = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(repo_path)
            .output();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Shallow clone support
    // -----------------------------------------------------------------------

    /// Clone a repo with --depth 1 --single-branch for fast initial setup.
    #[tracing::instrument(skip(token))]
    pub fn shallow_clone(url: &str, path: &Path, token: &str) -> Result<()> {
        tracing::info!(url = %url, path = %path.display(), "Shallow cloning repository");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                tracing::error!(error = %e, path = %parent.display(), url = %url, "Failed to create parent directory for shallow clone");
                e
            })?;
        }

        let credential_helper = Self::build_credential_helper(token)?;

        let output = Command::new("git")
            .args([
                "-c",
                &format!("credential.helper={credential_helper}"),
                "clone",
                "--depth",
                "1",
                "--single-branch",
            ])
            .arg(url)
            .arg(path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| {
                tracing::error!(error = %e, url = %url, path = %path.display(), "Failed to run git clone");
                Error::GitCommand(format!("Failed to run git clone: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(stderr = %stderr, url = %url, path = %path.display(), "Shallow clone failed");
            return Err(Error::GitCommand(format!("Shallow clone failed: {stderr}")));
        }

        Ok(())
    }

    /// Fetch a specific PR ref (works with shallow repos).
    #[tracing::instrument(skip(token))]
    pub fn fetch_pr_ref(repo_path: &Path, pr_number: i32, token: &str) -> Result<String> {
        tracing::info!(pr_number = pr_number, path = %repo_path.display(), "Fetching PR ref via shell git");

        let refspec = format!("refs/pull/{pr_number}/head:refs/remotes/origin/pr-{pr_number}");
        let credential_helper = Self::build_credential_helper(token)?;

        let output = Command::new("git")
            .args([
                "-c",
                &format!("credential.helper={credential_helper}"),
                "fetch",
                "origin",
                &refspec,
            ])
            .env("GIT_TERMINAL_PROMPT", "0")
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                tracing::error!(error = %e, pr_number = pr_number, path = %repo_path.display(), "Failed to fetch PR ref");
                Error::GitCommand(format!("Failed to fetch PR ref: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(stderr = %stderr, pr_number = pr_number, path = %repo_path.display(), "Failed to fetch PR ref");
            return Err(Error::GitCommand(format!(
                "Failed to fetch PR ref: {stderr}"
            )));
        }

        Ok(format!("refs/remotes/origin/pr-{pr_number}"))
    }

    /// Resolve a ref name to a commit hash.
    pub fn resolve_ref(repo_path: &Path, ref_name: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", ref_name])
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                tracing::error!(error = %e, ref_name = %ref_name, path = %repo_path.display(), "Failed to resolve ref");
                Error::GitCommand(format!("Failed to resolve ref: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::GitCommand(format!(
                "Failed to resolve ref {ref_name}: {stderr}"
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    // -----------------------------------------------------------------------
    // Credential helpers
    // -----------------------------------------------------------------------

    /// Build a credential helper string that provides the token via git's
    /// credential system. This avoids embedding credentials in URLs, which
    /// breaks on macOS curl versions that reject userinfo in URLs.
    fn build_credential_helper(token: &str) -> Result<String> {
        if !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        {
            return Err(Error::GitCommand(
                "Token contains invalid characters".into(),
            ));
        }
        Ok(format!(
            "!f() {{ echo username=x-access-token; echo password={token}; }}; f"
        ))
    }
}
