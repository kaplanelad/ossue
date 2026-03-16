use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::Platform;
use ossue_core::models::connector;
use ossue_core::models::project;
use ossue_core::services::github::GitHubClient;
use ossue_core::services::gitlab::GitLabClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct AddConnectorInput {
    pub name: String,
    pub platform: Platform,
    pub token: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateConnectorInput {
    pub name: Option<String>,
    pub token: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectorResponse {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub has_token: bool,
    pub token_preview: String,
    pub base_url: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

impl From<connector::Model> for ConnectorResponse {
    fn from(m: connector::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            platform: m.platform,
            has_token: !m.token.is_empty(),
            token_preview: mask_token(&m.token),
            base_url: m.base_url,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectorRepo {
    pub name: String,
    pub full_name: String,
    pub url: String,
    pub description: Option<String>,
    pub owner: String,
    pub stars: Option<i64>,
}

pub(crate) fn validate_base_url(url: &str) -> Result<(), CommandError> {
    if !url.starts_with("https://") {
        return Err(CommandError::Internal {
            message: "Only HTTPS URLs are allowed".to_string(),
        });
    }
    let host = url
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("");
    if host == "localhost"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("172.")
        || host == "::1"
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.contains("169.254.")
    {
        return Err(CommandError::Internal {
            message: "Private/internal URLs are not allowed".to_string(),
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn list_connectors(
    state: State<'_, AppState>,
) -> Result<Vec<ConnectorResponse>, CommandError> {
    let db = state.get_db().await?;

    let connectors = connector::Entity::find()
        .order_by_asc(connector::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query connectors from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;
    tracing::debug!(count = connectors.len(), "Listed connectors");
    Ok(connectors
        .into_iter()
        .map(ConnectorResponse::from)
        .collect())
}

#[tauri::command]
pub async fn add_connector(
    state: State<'_, AppState>,
    input: AddConnectorInput,
) -> Result<ConnectorResponse, CommandError> {
    tracing::info!(platform = %input.platform, name = %input.name, "Adding connector");
    let db = state.get_db().await?;

    // Validate base_url for GitLab
    if input.platform == Platform::GitLab {
        if let Some(ref url) = input.base_url {
            validate_base_url(url)?;
        }
    }

    // Validate token by calling the appropriate API
    match input.platform {
        Platform::GitHub => {
            let client = match input.base_url {
                Some(ref url) => {
                    GitHubClient::with_base_url(input.token.clone(), Some(url.clone()))
                }
                None => GitHubClient::new(input.token.clone()),
            };
            client
                .list_repos()
                .await
                .map_err(|e| {
                    tracing::error!(platform = "github", error = %e, "Connector token validation failed");
                    CommandError::PlatformApi {
                        message: format!("Invalid token: {e}"),
                    }
                })?;
        }
        Platform::GitLab => {
            let client = GitLabClient::new(input.token.clone(), input.base_url.clone());
            client
                .list_projects()
                .await
                .map_err(|e| {
                    tracing::error!(platform = "gitlab", error = %e, "Connector token validation failed");
                    CommandError::PlatformApi {
                        message: format!("Invalid token: {e}"),
                    }
                })?;
        }
    }

    let now = Utc::now().naive_utc();

    let model = connector::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        name: Set(input.name),
        platform: Set(input.platform),
        token: Set(input.token),
        base_url: Set(input.base_url),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let result = model.insert(&db).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to insert connector into database");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    Ok(ConnectorResponse::from(result))
}

#[tauri::command]
pub async fn update_connector(
    state: State<'_, AppState>,
    id: String,
    input: UpdateConnectorInput,
) -> Result<ConnectorResponse, CommandError> {
    tracing::info!(id = %id, "Updating connector");
    let db = state.get_db().await?;

    let existing = connector::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query connector for update");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Connector not found for update");
            CommandError::NotFound {
                entity: "Connector".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: connector::ActiveModel = existing.into();

    if let Some(name) = input.name {
        active.name = Set(name);
    }
    if let Some(token) = input.token {
        active.token = Set(token);
    }
    if let Some(base_url) = input.base_url {
        active.base_url = Set(Some(base_url));
    }

    active.updated_at = Set(Utc::now().naive_utc());

    let result = active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update connector in database");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    Ok(ConnectorResponse::from(result))
}

#[tauri::command]
pub async fn remove_connector(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    tracing::info!(id = %id, "Removing connector");
    let db = state.get_db().await?;

    // Check if any projects reference this connector
    let project_count = project::Entity::find()
        .filter(project::Column::ConnectorId.eq(&id))
        .count(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to count projects for connector");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    if project_count > 0 {
        tracing::warn!(id = %id, project_count = project_count, "Connector removal blocked by existing projects");
        return Err(CommandError::Internal {
            message: format!(
                "Cannot delete connector: {} project(s) still reference it. Remove or reassign them first.",
                project_count
            ),
        });
    }

    connector::Entity::delete_by_id(&id)
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to delete connector from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn list_connector_repos(
    state: State<'_, AppState>,
    connector_id: String,
) -> Result<Vec<ConnectorRepo>, CommandError> {
    let db = state.get_db().await?;

    let conn = connector::Entity::find_by_id(&connector_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, connector_id = %connector_id, "Failed to query connector for repo listing");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(connector_id = %connector_id, "Connector not found for repo listing");
            CommandError::NotFound {
                entity: "Connector".to_string(),
                id: connector_id.clone(),
            }
        })?;

    let result: Result<Vec<ConnectorRepo>, CommandError> = match conn.platform {
        Platform::GitHub => {
            let client = match conn.base_url {
                Some(ref url) => GitHubClient::with_base_url(conn.token, Some(url.clone())),
                None => GitHubClient::new(conn.token),
            };
            let repos = client.list_repos().await.map_err(|e| {
                tracing::error!(error = %e, connector_id = %connector_id, "Failed to list GitHub repos for connector");
                CommandError::PlatformApi {
                    message: e.to_string(),
                }
            })?;
            Ok(repos
                .into_iter()
                .map(|r| ConnectorRepo {
                    name: r.name,
                    full_name: r.full_name,
                    url: r.html_url,
                    description: r.description,
                    owner: r.owner.login,
                    stars: r.stargazers_count,
                })
                .collect())
        }
        Platform::GitLab => {
            let client = GitLabClient::new(conn.token, conn.base_url);
            let projects = client.list_projects().await.map_err(|e| {
                tracing::error!(error = %e, connector_id = %connector_id, "Failed to list GitLab projects for connector");
                CommandError::PlatformApi {
                    message: e.to_string(),
                }
            })?;
            Ok(projects
                .into_iter()
                .map(|p| ConnectorRepo {
                    name: p.name,
                    full_name: p.path_with_namespace.clone(),
                    url: p.web_url,
                    description: p.description,
                    owner: p.namespace.path,
                    stars: p.star_count,
                })
                .collect())
        }
    };
    if let Ok(ref repos) = result {
        tracing::debug!(connector_id = %connector_id, count = repos.len(), "Listed connector repos");
    }
    result
}
