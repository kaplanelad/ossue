use sea_orm::{EntityTrait, Set};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::OAuthStatus;
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

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthStartResponse {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[tauri::command]
pub async fn start_github_oauth(
    state: State<'_, AppState>,
) -> Result<OAuthStartResponse, CommandError> {
    tracing::info!("Starting GitHub OAuth device flow");
    let client_id = ossue_core::services::oauth::GITHUB_CLIENT_ID;
    let resp = ossue_core::services::oauth::request_device_code(client_id, "repo")
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to request device code");
            CommandError::PlatformApi {
                message: e.to_string(),
            }
        })?;

    let device_state = crate::OAuthDeviceState {
        device_code: resp.device_code,
        interval: resp.interval,
        client_id: client_id.to_string(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(resp.expires_in),
    };
    *state.oauth_device_state.lock().await = Some(device_state);

    Ok(OAuthStartResponse {
        user_code: resp.user_code,
        verification_uri: resp.verification_uri,
        interval: resp.interval,
        expires_in: resp.expires_in,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthPollResponse {
    pub status: OAuthStatus,
    pub access_token: Option<String>,
}

#[tauri::command]
pub async fn poll_github_oauth(
    state: State<'_, AppState>,
) -> Result<OAuthPollResponse, CommandError> {
    let device_state = state.oauth_device_state.lock().await;
    let ds = device_state
        .as_ref()
        .ok_or_else(|| CommandError::Internal {
            message: "No OAuth flow in progress".to_string(),
        })?;

    if ds.expires_at < std::time::Instant::now() {
        return Ok(OAuthPollResponse {
            status: OAuthStatus::Expired,
            access_token: None,
        });
    }

    let result = ossue_core::services::oauth::poll_for_token(&ds.client_id, &ds.device_code)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to poll for OAuth token");
            CommandError::PlatformApi {
                message: e.to_string(),
            }
        })?;
    drop(device_state);

    match result {
        ossue_core::services::oauth::PollResult::Pending => Ok(OAuthPollResponse {
            status: OAuthStatus::Pending,
            access_token: None,
        }),
        ossue_core::services::oauth::PollResult::Success { access_token, .. } => {
            *state.oauth_device_state.lock().await = None;
            Ok(OAuthPollResponse {
                status: OAuthStatus::Success,
                access_token: Some(access_token),
            })
        }
        ossue_core::services::oauth::PollResult::SlowDown => Ok(OAuthPollResponse {
            status: OAuthStatus::SlowDown,
            access_token: None,
        }),
        ossue_core::services::oauth::PollResult::Expired => {
            *state.oauth_device_state.lock().await = None;
            Ok(OAuthPollResponse {
                status: OAuthStatus::Expired,
                access_token: None,
            })
        }
        ossue_core::services::oauth::PollResult::Denied => {
            *state.oauth_device_state.lock().await = None;
            Ok(OAuthPollResponse {
                status: OAuthStatus::Denied,
                access_token: None,
            })
        }
        ossue_core::services::oauth::PollResult::Error { message: _ } => Ok(OAuthPollResponse {
            status: OAuthStatus::Error,
            access_token: None,
        }),
    }
}

#[tauri::command]
pub async fn cancel_github_oauth(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::info!("Cancelling GitHub OAuth device flow");
    *state.oauth_device_state.lock().await = None;
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
