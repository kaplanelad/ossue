use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::Platform;
use ossue_core::models::project;
use ossue_core::services::git::GitService;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),

    #[error("No token configured for {0}")]
    NoTokenConfigured(String),

    #[error("Could not parse repository URL. Expected format: https://github.com/owner/repo")]
    InvalidRepoUrl,
}

impl From<Error> for CommandError {
    fn from(e: Error) -> Self {
        match e {
            Error::Database(db_err) => CommandError::Internal {
                message: db_err.to_string(),
            },
            Error::NoTokenConfigured(platform) => CommandError::Internal {
                message: format!("No token configured for {platform}"),
            },
            Error::InvalidRepoUrl => CommandError::Internal {
                message:
                    "Could not parse repository URL. Expected format: https://github.com/owner/repo"
                        .to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddProjectInput {
    pub name: String,
    pub owner: String,
    pub platform: Platform,
    pub url: String,
    pub connector_id: Option<String>,
}

#[tauri::command]
pub async fn list_projects(
    state: State<'_, AppState>,
) -> Result<Vec<project::Model>, CommandError> {
    let db = state.get_db().await?;

    project::Entity::find()
        .order_by_asc(project::Column::Name)
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query projects from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })
}

#[tauri::command]
pub async fn add_project(
    state: State<'_, AppState>,
    input: AddProjectInput,
) -> Result<project::Model, CommandError> {
    tracing::info!(owner = %input.owner, name = %input.name, platform = %input.platform, "Adding project");
    let db = state.get_db().await?;

    // Check for existing project with same owner+name+platform
    let existing = project::Entity::find()
        .filter(project::Column::Owner.eq(&input.owner))
        .filter(project::Column::Name.eq(&input.name))
        .filter(project::Column::Platform.eq(input.platform.clone()))
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query existing project");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    if let Some(existing) = existing {
        tracing::debug!(id = %existing.id, "Project already exists, returning existing");
        return Ok(existing);
    }

    let now = Utc::now().naive_utc();

    let model = project::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        name: Set(input.name),
        owner: Set(input.owner),
        platform: Set(input.platform),
        url: Set(input.url),
        clone_path: Set(None),
        default_branch: Set(None),
        api_token: Set(None),
        connector_id: Set(input.connector_id),
        external_project_id: Set(None),
        sync_enabled: Set(true),
        last_sync_at: Set(None),
        last_sync_error: Set(None),
        full_reconciliation_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let result = model.insert(&db).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to insert project into database");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    Ok(result)
}

#[tauri::command]
pub async fn add_project_by_url(
    state: State<'_, AppState>,
    url: String,
    connector_id: Option<String>,
) -> Result<project::Model, CommandError> {
    // Parse URL to extract platform, owner, name
    let (platform, owner, name) = parse_repo_url(&url).map_err(|e| {
        tracing::error!(error = %e, url = %url, "Failed to parse repository URL");
        CommandError::from(e)
    })?;
    tracing::info!(url = %url, platform = %platform, owner = %owner, name = %name, "Adding project by URL");

    let input = AddProjectInput {
        name,
        owner,
        platform,
        url,
        connector_id,
    };

    add_project(state, input).await
}

#[tauri::command]
pub async fn remove_project(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    tracing::info!(id = %id, "Removing project");
    let db = state.get_db().await?;

    project::Entity::delete_by_id(&id)
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to delete project from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn prepare_repo(
    state: State<'_, AppState>,
    project_id: String,
    branch: Option<String>,
    pr_number: Option<i32>,
) -> Result<String, CommandError> {
    tracing::info!(project_id = %project_id, branch = ?branch, pr_number = ?pr_number, "Preparing repo");
    let db = state.get_db().await?;

    let proj = project::Entity::find_by_id(&project_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, "Failed to query project for prepare_repo");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(project_id = %project_id, "Project not found for prepare_repo");
            CommandError::NotFound {
                entity: "Project".to_string(),
                id: project_id.clone(),
            }
        })?;

    // Get token
    let token = get_project_token(&db, &proj).await.map_err(|e| {
        tracing::error!(error = %e, project_id = %project_id, "Failed to resolve token for prepare_repo");
        CommandError::from(e)
    })?;

    let repo_path =
        GitService::repo_path(&proj.platform, &proj.owner, &proj.name, None).map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, "Failed to determine repo path");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    if !GitService::is_cloned(&repo_path) {
        tracing::debug!(path = %repo_path.display(), "Repo not cloned, cloning");
        GitService::clone_repo(&proj.url, &repo_path, &token).map_err(|e| {
            tracing::error!(error = %e, url = %proj.url, "Failed to clone repo");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;
    } else {
        tracing::debug!(path = %repo_path.display(), "Repo exists, fetching updates");
        GitService::fetch_repo(&repo_path, &token).map_err(|e| {
            tracing::error!(error = %e, path = %repo_path.display(), "Failed to fetch repo");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;
    }

    if let Some(pr_num) = pr_number {
        GitService::fetch_pr_branch(&repo_path, pr_num, &token).map_err(|e| {
            tracing::error!(error = %e, pr_number = pr_num, "Failed to fetch PR branch");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;
    } else if let Some(branch_name) = branch {
        GitService::checkout_branch(&repo_path, &branch_name).map_err(|e| {
            tracing::error!(error = %e, branch = %branch_name, "Failed to checkout branch");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;
    } else {
        // Default to main or master
        if GitService::checkout_branch(&repo_path, "main").is_err() {
            tracing::warn!("Branch 'main' not found, falling back to 'master'");
            GitService::checkout_branch(&repo_path, "master").map_err(|e| {
                tracing::error!(error = %e, "Failed to checkout 'master' branch (fallback)");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
        }
    }

    Ok(repo_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn clear_repo_cache(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::info!("Clearing repo cache");
    GitService::clear_cache(state.repo_manager.fetch_cache()).map_err(|e| {
        tracing::error!(error = %e, "Failed to clear repo cache");
        CommandError::Internal {
            message: e.to_string(),
        }
    })
}

/// Resolve a token for a project using 3-tier lookup.
/// Delegates to [`ossue_core::services::auth::get_project_token`].
pub async fn get_project_token(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Result<String, Error> {
    ossue_core::services::auth::get_project_token(db, project)
        .await
        .map_err(|e| match e {
            ossue_core::services::auth::Error::Database(db_err) => Error::Database(db_err),
            ossue_core::services::auth::Error::NoTokenConfigured(platform) => {
                Error::NoTokenConfigured(platform)
            }
        })
}

/// Resolve the base_url for a project (GitLab only).
/// Delegates to [`ossue_core::services::auth::get_project_base_url`].
pub async fn get_project_base_url(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Option<String> {
    ossue_core::services::auth::get_project_base_url(db, project).await
}

#[tauri::command]
pub async fn toggle_project_sync(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    tracing::info!(id = %id, enabled = enabled, "Toggling project sync");
    let db = state.get_db().await?;

    let proj = project::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query project for toggle sync");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Project not found for toggle sync");
            CommandError::NotFound {
                entity: "Project".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: project::ActiveModel = proj.into();
    active.sync_enabled = Set(enabled);
    active.updated_at = Set(Utc::now().naive_utc());
    active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update project sync_enabled");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}

/// Parse a repository URL into (platform, owner, name).
/// NOTE: Currently only recognizes `github.com` and `gitlab.com` hostnames.
/// This will need updating when custom GitHub Enterprise domains are supported.
fn parse_repo_url(url: &str) -> Result<(Platform, String, String), Error> {
    let url = url.trim_end_matches('/');
    let url = url.trim_end_matches(".git");

    if url.contains("github.com") {
        let parts: Vec<&str> = url.split('/').collect();
        let len = parts.len();
        if len >= 2 {
            let owner = parts[len - 2].to_string();
            let name = parts[len - 1].to_string();
            return Ok((Platform::GitHub, owner, name));
        }
    } else if url.contains("gitlab.com") || url.contains("gitlab") {
        let parts: Vec<&str> = url.split('/').collect();
        let len = parts.len();
        if len >= 2 {
            let owner = parts[len - 2].to_string();
            let name = parts[len - 1].to_string();
            return Ok((Platform::GitLab, owner, name));
        }
    }

    Err(Error::InvalidRepoUrl)
}
