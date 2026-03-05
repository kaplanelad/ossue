use sea_orm::{EntityTrait, Set};
use tauri::State;

use super::error::CommandError;
use crate::AppState;
use ossue_core::logging::{read_log_entries, LogEntriesResponse};
use ossue_core::models::settings as settings_model;

#[tauri::command]
pub async fn get_log_level(state: State<'_, AppState>) -> Result<String, CommandError> {
    let handle = &state.log_reload_handle;
    let current = handle
        .with_current(|filter| filter.to_string().to_uppercase())
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to read current log level");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;
    Ok(current)
}

#[tauri::command]
pub async fn set_log_level(state: State<'_, AppState>, level: String) -> Result<(), CommandError> {
    let old_level = state
        .log_reload_handle
        .with_current(|filter| filter.to_string().to_uppercase())
        .unwrap_or_else(|_| "UNKNOWN".to_string());

    let filter: tracing_subscriber::filter::LevelFilter =
        level.parse().map_err(|_| CommandError::Internal {
            message: format!("Invalid log level: {}", level),
        })?;

    state
        .log_reload_handle
        .modify(|f| *f = filter)
        .map_err(|e| {
            tracing::error!(error = %e, level = %level, "Failed to apply new log level");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    tracing::info!(old_level = %old_level, new_level = %level, "Log level changed");

    // Persist to settings
    if let Some(db) = state.db.read().await.clone() {
        let setting = settings_model::ActiveModel {
            key: Set("log_level".to_string()),
            value: Set(level),
        };
        let _ = settings_model::Entity::insert(setting)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(settings_model::Column::Key)
                    .update_column(settings_model::Column::Value)
                    .to_owned(),
            )
            .exec(&db)
            .await;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_log_entries(
    state: State<'_, AppState>,
    level_filter: Option<String>,
    text_filter: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<LogEntriesResponse, CommandError> {
    let log_dir = &state.log_dir;
    let limit = limit.unwrap_or(200);
    let offset = offset.unwrap_or(0);

    Ok(read_log_entries(
        log_dir,
        level_filter.as_deref(),
        text_filter.as_deref(),
        limit,
        offset,
    ))
}

#[tauri::command]
pub async fn clear_logs(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::info!("Clearing all log files");
    let log_dir = &state.log_dir;

    if log_dir.exists() {
        for entry in std::fs::read_dir(log_dir).map_err(|e| {
            tracing::error!(error = %e, "Failed to read log directory for clearing");
            CommandError::Internal {
                message: e.to_string(),
            }
        })? {
            let entry = entry.map_err(|e| {
                tracing::error!(error = %e, "Failed to read log directory entry");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("app.log") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    Ok(())
}
