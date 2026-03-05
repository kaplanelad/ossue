use sea_orm::{DatabaseConnection, EntityTrait};

use crate::enums::Platform;
use crate::models::{connector, project, settings};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),

    #[error("No token configured for {0}")]
    NoTokenConfigured(String),
}

/// Resolve a token for a project using 3-tier lookup:
/// 1. project.api_token (per-project override)
/// 2. connector token (via connector_id)
/// 3. legacy settings fallback (github_token / gitlab_token)
pub async fn get_project_token(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Result<String, Error> {
    // 1. Per-project token override
    if let Some(ref token) = project.api_token {
        tracing::debug!(project_id = %project.id, tier = "project", "Token resolved from per-project override");
        return Ok(token.clone());
    }

    // 2. Connector token
    if let Some(ref cid) = project.connector_id {
        if let Some(conn) = connector::Entity::find_by_id(cid).one(db).await? {
            tracing::debug!(project_id = %project.id, tier = "connector", connector_id = %cid, "Token resolved from connector");
            return Ok(conn.token);
        }
    }

    // 3. Legacy settings fallback
    tracing::debug!(project_id = %project.id, tier = "legacy_settings", "Token resolved from legacy settings");
    let token_key = match project.platform {
        Platform::GitHub => "github_token",
        Platform::GitLab => "gitlab_token",
    };

    let setting = settings::Entity::find_by_id(token_key)
        .one(db)
        .await?
        .ok_or_else(|| Error::NoTokenConfigured(format!("{:?}", project.platform)))?;

    Ok(setting.value)
}

/// Resolve the base_url for a project.
/// Checks connector first, then legacy settings.
pub async fn get_project_base_url(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Option<String> {
    // Check connector
    if let Some(ref cid) = project.connector_id {
        if let Ok(Some(conn)) = connector::Entity::find_by_id(cid).one(db).await {
            return conn.base_url;
        }
    }

    // Legacy settings fallback
    settings::Entity::find_by_id("gitlab_base_url")
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|s| s.value)
}
