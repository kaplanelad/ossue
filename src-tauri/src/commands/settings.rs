use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::error::CommandError;
use crate::AppState;
use ossue_core::models::connector;
use ossue_core::models::project_settings;
use ossue_core::models::settings as settings_model;
use ossue_core::services::git::GitService;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppPaths {
    pub repo_cache_dir: String,
    pub repo_cache_size_bytes: u64,
    pub database_file: String,
    pub log_dir: String,
}

fn dir_size(path: &std::path::Path) -> u64 {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok().map(|m| (e.path(), m)))
        .map(|(path, meta)| {
            if meta.is_dir() {
                dir_size(&path)
            } else {
                meta.len()
            }
        })
        .sum()
}

#[tauri::command]
pub async fn get_app_paths(state: State<'_, AppState>) -> Result<AppPaths, CommandError> {
    let (repo_cache_dir, repo_cache_size_bytes) = GitService::get_cache_dir()
        .map(|p| {
            let size = dir_size(&p);
            (p.display().to_string(), size)
        })
        .unwrap_or_default();

    let database_file = ossue_core::db::app_data_dir()
        .map(|p| p.join("data.db").display().to_string())
        .unwrap_or_default();

    let log_dir = state.log_dir.display().to_string();

    Ok(AppPaths {
        repo_cache_dir,
        repo_cache_size_bytes,
        database_file,
        log_dir,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub ai_mode: String,
    pub ai_provider: String,
    pub has_ai_api_key: bool,
    pub ai_model: String,
    pub refresh_interval: u64,
    pub github_connected: bool,
    pub gitlab_connected: bool,
    pub log_level: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiSettings {
    pub ai_mode: String,
    pub ai_provider: String,
    pub ai_model: String,
    pub has_ai_api_key: bool,
    pub ai_focus_areas: Vec<String>,
    pub ai_review_strictness: String,
    pub ai_response_tone: String,
    pub ai_custom_instructions: Option<String>,
}

const DEFAULT_FOCUS_AREAS: &[&str] = &[
    "security",
    "performance",
    "api_compatibility",
    "test_coverage",
    "code_style",
    "documentation",
];

#[tauri::command]
pub async fn get_ai_settings(state: State<'_, AppState>) -> Result<AiSettings, CommandError> {
    tracing::debug!("Getting AI settings");
    let db = state.get_db().await?;

    let ai_mode = get_setting(&db, "ai_mode")
        .await
        .unwrap_or_else(|| "api".to_string());
    let ai_provider = get_setting(&db, "ai_provider")
        .await
        .unwrap_or_else(|| "anthropic".to_string());
    let ai_model = get_setting(&db, "ai_model").await.unwrap_or_default();
    let has_ai_api_key = get_setting(&db, "ai_api_key").await.is_some();

    let ai_focus_areas = match get_setting(&db, "ai_focus_areas").await {
        Some(raw) => serde_json::from_str::<Vec<String>>(&raw).unwrap_or_else(|_| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }),
        None => DEFAULT_FOCUS_AREAS.iter().map(|s| s.to_string()).collect(),
    };

    let ai_review_strictness = get_setting(&db, "ai_review_strictness")
        .await
        .unwrap_or_else(|| "pragmatic".to_string());
    let ai_response_tone = get_setting(&db, "ai_response_tone")
        .await
        .unwrap_or_else(|| "friendly".to_string());
    let ai_custom_instructions = get_setting(&db, "ai_custom_instructions").await;

    Ok(AiSettings {
        ai_mode,
        ai_provider,
        ai_model,
        has_ai_api_key,
        ai_focus_areas,
        ai_review_strictness,
        ai_response_tone,
        ai_custom_instructions,
    })
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    tracing::debug!("Getting settings");
    let db = state.get_db().await?;

    let ai_mode = get_setting(&db, "ai_mode")
        .await
        .unwrap_or_else(|| "api".to_string());
    let ai_provider = get_setting(&db, "ai_provider")
        .await
        .unwrap_or_else(|| "anthropic".to_string());
    let has_ai_api_key = get_setting(&db, "ai_api_key").await.is_some();
    let ai_model = get_setting(&db, "ai_model").await.unwrap_or_default();
    let refresh_interval = get_setting(&db, "refresh_interval")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800);
    let github_connected = get_setting(&db, "github_token").await.is_some();
    let gitlab_connected = get_setting(&db, "gitlab_token").await.is_some();
    let log_level = get_setting(&db, "log_level")
        .await
        .unwrap_or_else(|| "ERROR".to_string());

    Ok(AppSettings {
        ai_mode,
        ai_provider,
        has_ai_api_key,
        ai_model,
        refresh_interval,
        github_connected,
        gitlab_connected,
        log_level,
    })
}

const ALLOWED_SETTING_KEYS: &[&str] = &[
    "ai_mode",
    "ai_provider",
    "ai_api_key",
    "ai_model",
    "ai_focus_areas",
    "ai_review_strictness",
    "ai_response_tone",
    "ai_custom_instructions",
    "refresh_interval",
    "log_level",
    "attention_sensitive_paths",
];

#[tauri::command]
pub async fn update_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), CommandError> {
    if !ALLOWED_SETTING_KEYS.contains(&key.as_str()) {
        return Err(CommandError::Internal {
            message: format!("Setting key '{}' is not allowed", key),
        });
    }
    tracing::info!(key = %key, "Updating setting");
    let db = state.get_db().await?;

    let setting = settings_model::ActiveModel {
        key: Set(key.clone()),
        value: Set(value),
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
            tracing::error!(error = %e, key = %key, "Failed to update setting in database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn delete_setting(state: State<'_, AppState>, key: String) -> Result<(), CommandError> {
    if !ALLOWED_SETTING_KEYS.contains(&key.as_str()) {
        return Err(CommandError::Internal {
            message: format!("Setting key '{}' is not allowed", key),
        });
    }
    tracing::info!(key = %key, "Deleting setting");
    let db = state.get_db().await?;

    settings_model::Entity::delete_by_id(&key)
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, key = %key, "Failed to delete setting from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn is_onboarding_complete(state: State<'_, AppState>) -> Result<bool, CommandError> {
    let db = state.get_db().await?;

    // Check connectors table first
    let has_connectors = connector::Entity::find()
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query connectors for onboarding check");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .is_some();

    if has_connectors {
        tracing::debug!(
            complete = true,
            reason = "has_connectors",
            "Onboarding check"
        );
        return Ok(true);
    }

    // Legacy settings fallback
    let has_token = get_setting(&db, "github_token").await.is_some()
        || get_setting(&db, "gitlab_token").await.is_some();

    tracing::debug!(
        complete = has_token,
        reason = "legacy_token",
        "Onboarding check"
    );
    Ok(has_token)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectSettingEntry {
    pub key: String,
    pub value: String,
}

#[tauri::command]
pub async fn get_project_settings(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ProjectSettingEntry>, CommandError> {
    tracing::debug!(project_id = %project_id, "Getting project settings");
    let db = state.get_db().await?;

    let settings = project_settings::Entity::find()
        .filter(project_settings::Column::ProjectId.eq(&project_id))
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, "Failed to query project settings");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(settings
        .into_iter()
        .map(|s| ProjectSettingEntry {
            key: s.key,
            value: s.value,
        })
        .collect())
}

#[tauri::command]
pub async fn update_project_setting(
    state: State<'_, AppState>,
    project_id: String,
    key: String,
    value: String,
) -> Result<(), CommandError> {
    tracing::info!(project_id = %project_id, key = %key, "Updating project setting");
    let db = state.get_db().await?;

    let setting = project_settings::ActiveModel {
        project_id: Set(project_id.clone()),
        key: Set(key.clone()),
        value: Set(value),
    };

    project_settings::Entity::insert(setting)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                project_settings::Column::ProjectId,
                project_settings::Column::Key,
            ])
            .update_column(project_settings::Column::Value)
            .to_owned(),
        )
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, key = %key, "Failed to update project setting");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn delete_project_setting(
    state: State<'_, AppState>,
    project_id: String,
    key: String,
) -> Result<(), CommandError> {
    tracing::info!(project_id = %project_id, key = %key, "Deleting project setting");
    let db = state.get_db().await?;

    project_settings::Entity::delete_many()
        .filter(project_settings::Column::ProjectId.eq(&project_id))
        .filter(project_settings::Column::Key.eq(&key))
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, key = %key, "Failed to delete project setting");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

async fn get_setting(db: &sea_orm::DatabaseConnection, key: &str) -> Option<String> {
    settings_model::Entity::find_by_id(key)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|s| s.value)
}
