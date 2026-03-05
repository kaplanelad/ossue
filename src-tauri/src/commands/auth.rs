use sea_orm::{EntityTrait, Set};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::error::CommandError;
use crate::AppState;
use ossue_core::models::connector;
use ossue_core::models::settings as settings_model;
use ossue_core::services::github::GitHubClient;
use ossue_core::services::gitlab::GitLabClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthStatus {
    pub github_connected: bool,
    pub gitlab_connected: bool,
}

#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatus, CommandError> {
    let db = state.get_db().await?;

    let github_token = settings_model::Entity::find_by_id("github_token")
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query github_token setting");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    let gitlab_token = settings_model::Entity::find_by_id("gitlab_token")
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query gitlab_token setting");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    let status = AuthStatus {
        github_connected: github_token.is_some(),
        gitlab_connected: gitlab_token.is_some(),
    };
    tracing::debug!(
        github = status.github_connected,
        gitlab = status.gitlab_connected,
        "Auth status checked"
    );
    Ok(status)
}

#[tauri::command]
pub async fn save_github_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<(), CommandError> {
    tracing::info!("Saving GitHub token (validating first)");
    let db = state.get_db().await?;

    // Validate token by trying to list repos
    let client = GitHubClient::new(token.clone());
    client.list_repos().await.map_err(|e| {
        tracing::error!(error = %e, "GitHub token validation failed");
        CommandError::PlatformApi {
            message: format!("Invalid token: {e}"),
        }
    })?;

    let setting = settings_model::ActiveModel {
        key: Set("github_token".to_string()),
        value: Set(token),
    };

    settings_model::Entity::insert(setting)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(settings_model::Column::Key)
                .update_column(settings_model::Column::Value)
                .to_owned(),
        )
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to save GitHub token to database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn save_gitlab_token(
    state: State<'_, AppState>,
    token: String,
    base_url: Option<String>,
) -> Result<(), CommandError> {
    tracing::info!("Saving GitLab token (validating first)");

    // Validate base_url
    if let Some(ref url) = base_url {
        super::connectors::validate_base_url(url)?;
    }

    let db = state.get_db().await?;

    // Validate token
    let client = GitLabClient::new(token.clone(), base_url.clone());
    client.list_projects().await.map_err(|e| {
        tracing::error!(error = %e, "GitLab token validation failed");
        CommandError::PlatformApi {
            message: format!("Invalid token: {e}"),
        }
    })?;

    let setting = settings_model::ActiveModel {
        key: Set("gitlab_token".to_string()),
        value: Set(token),
    };

    settings_model::Entity::insert(setting)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(settings_model::Column::Key)
                .update_column(settings_model::Column::Value)
                .to_owned(),
        )
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to save GitLab token to database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    if let Some(url) = base_url {
        let url_setting = settings_model::ActiveModel {
            key: Set("gitlab_base_url".to_string()),
            value: Set(url),
        };
        settings_model::Entity::insert(url_setting)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(settings_model::Column::Key)
                    .update_column(settings_model::Column::Value)
                    .to_owned(),
            )
            .exec(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to save GitLab base URL to database");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
    }

    Ok(())
}

#[tauri::command]
pub async fn disconnect_github(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::info!("Disconnecting GitHub account");
    let db = state.get_db().await?;

    settings_model::Entity::delete_by_id("github_token")
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to delete GitHub token from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn disconnect_gitlab(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::info!("Disconnecting GitLab account");
    let db = state.get_db().await?;

    settings_model::Entity::delete_by_id("gitlab_token")
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to delete GitLab token from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    settings_model::Entity::delete_by_id("gitlab_base_url")
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to delete GitLab base URL from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn list_github_repos(
    state: State<'_, AppState>,
    connector_id: Option<String>,
) -> Result<Vec<ossue_core::services::github::GitHubRepo>, CommandError> {
    let db = state.get_db().await?;

    let connector_id_log = connector_id.clone();
    let token = if let Some(cid) = connector_id {
        let conn = connector::Entity::find_by_id(&cid)
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, connector_id = %cid, "Failed to query connector");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| {
                tracing::warn!(connector_id = %cid, "Connector not found for GitHub repo listing");
                CommandError::NotFound {
                    entity: "Connector".to_string(),
                    id: cid.clone(),
                }
            })?;
        conn.token
    } else {
        settings_model::Entity::find_by_id("github_token")
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to query GitHub token setting");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| {
                tracing::warn!("GitHub not connected - no token found");
                CommandError::PlatformApi {
                    message: "GitHub not connected".to_string(),
                }
            })?
            .value
    };

    let client = GitHubClient::new(token);
    let repos = client.list_repos().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to list GitHub repos");
        CommandError::PlatformApi {
            message: e.to_string(),
        }
    })?;
    tracing::debug!(connector_id = ?connector_id_log, count = repos.len(), "Listed GitHub repos");
    Ok(repos)
}

#[tauri::command]
pub async fn list_gitlab_projects(
    state: State<'_, AppState>,
    connector_id: Option<String>,
) -> Result<Vec<ossue_core::services::gitlab::GitLabProject>, CommandError> {
    let db = state.get_db().await?;

    let connector_id_log = connector_id.clone();
    let (token, base_url) = if let Some(cid) = connector_id {
        let conn = connector::Entity::find_by_id(&cid)
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, connector_id = %cid, "Failed to query connector for GitLab");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| {
                tracing::warn!(connector_id = %cid, "Connector not found for GitLab project listing");
                CommandError::NotFound {
                    entity: "Connector".to_string(),
                    id: cid.clone(),
                }
            })?;
        (conn.token, conn.base_url)
    } else {
        let token = settings_model::Entity::find_by_id("gitlab_token")
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to query GitLab token setting");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| {
                tracing::warn!("GitLab not connected - no token found");
                CommandError::PlatformApi {
                    message: "GitLab not connected".to_string(),
                }
            })?
            .value;

        let base_url = settings_model::Entity::find_by_id("gitlab_base_url")
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to query GitLab base URL setting");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .map(|s| s.value);

        (token, base_url)
    };

    let client = GitLabClient::new(token, base_url);
    let projects = client.list_projects().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to list GitLab projects");
        CommandError::PlatformApi {
            message: e.to_string(),
        }
    })?;
    tracing::debug!(connector_id = ?connector_id_log, count = projects.len(), "Listed GitLab projects");
    Ok(projects)
}
