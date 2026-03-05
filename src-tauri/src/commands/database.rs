use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::error::CommandError;
use crate::AppState;
use ossue_core::db::{app_data_dir, backups_dir};

fn is_valid_backup_filename(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("data_backup_") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(".db") else {
        return false;
    };
    // Expected format: YYYYMMDD_HHMMSS (15 chars with underscore at position 8)
    rest.len() == 15
        && rest.as_bytes()[8] == b'_'
        && rest[..8].chars().all(|c| c.is_ascii_digit())
        && rest[9..].chars().all(|c| c.is_ascii_digit())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupInfo {
    pub filename: String,
    pub created_at: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> Result<BackupInfo, CommandError> {
    tracing::info!("Creating database backup");
    let app_dir = app_data_dir().map_err(|e| {
        tracing::error!(error = %e, "Could not determine application data directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    let db_path = app_dir.join("data.db");

    if !db_path.exists() {
        tracing::error!("Database file not found for backup");
        return Err(CommandError::Internal {
            message: "Database file not found".to_string(),
        });
    }

    // WAL checkpoint: force WAL content into main DB file
    {
        let db = state.get_db().await?;
        sea_orm::ConnectionTrait::execute_unprepared(&db, "PRAGMA wal_checkpoint(TRUNCATE)")
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "WAL checkpoint failed");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
    }

    let backup_dir = backups_dir().map_err(|e| {
        tracing::error!(error = %e, "Failed to create backups directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("data_backup_{}.db", timestamp);
    let backup_path = backup_dir.join(&filename);

    std::fs::copy(&db_path, &backup_path).map_err(|e| {
        tracing::error!(error = %e, backup_path = %backup_path.display(), "Failed to copy database for backup");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    let metadata = std::fs::metadata(&backup_path).map_err(|e| {
        tracing::error!(error = %e, backup_path = %backup_path.display(), "Failed to read backup file metadata");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    let info = BackupInfo {
        filename,
        created_at: chrono::Utc::now().to_rfc3339(),
        size_bytes: metadata.len(),
    };
    tracing::info!(filename = %info.filename, size_bytes = info.size_bytes, "Backup created");
    Ok(info)
}

#[tauri::command]
pub async fn list_backups() -> Result<Vec<BackupInfo>, CommandError> {
    tracing::debug!("Listing backups");
    let backup_dir = backups_dir().map_err(|e| {
        tracing::error!(error = %e, "Failed to access backups directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    let mut backups: Vec<BackupInfo> = std::fs::read_dir(&backup_dir)
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to read backups directory");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("data_backup_") || !name.ends_with(".db") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            let created = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
                .map(|d| {
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
                .unwrap_or_default();

            Some(BackupInfo {
                filename: name,
                created_at: created,
                size_bytes: metadata.len(),
            })
        })
        .collect();

    // Sort newest first
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    tracing::debug!(count = backups.len(), "Listed backups");
    Ok(backups)
}

#[tauri::command]
pub async fn restore_backup(
    state: State<'_, AppState>,
    filename: String,
) -> Result<(), CommandError> {
    tracing::info!(filename = %filename, "Restoring database from backup");
    if !is_valid_backup_filename(&filename) {
        tracing::warn!(filename = %filename, "Invalid backup filename in restore_backup");
        return Err(CommandError::Internal {
            message: "Invalid filename".to_string(),
        });
    }

    let app_dir = app_data_dir().map_err(|e| {
        tracing::error!(error = %e, "Could not determine application data directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    let backup_dir = backups_dir().map_err(|e| {
        tracing::error!(error = %e, "Failed to access backups directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    let backup_path = backup_dir.join(&filename);

    if !backup_path.exists() {
        tracing::error!(filename = %filename, "Backup file not found for restore");
        return Err(CommandError::NotFound {
            entity: "Backup".to_string(),
            id: filename,
        });
    }

    let db_path = app_dir.join("data.db");
    let wal_path = app_dir.join("data.db-wal");
    let shm_path = app_dir.join("data.db-shm");

    // Cancel all running sync tasks and retry handles
    {
        let mut retry_handles = state.retry_handles.lock().await;
        for (project_id, handle) in retry_handles.drain() {
            handle.abort();
            tracing::info!(project_id = %project_id, "Cancelled pending sync retry during restore");
        }
    }
    {
        let mut syncing = state.syncing_projects.lock().await;
        syncing.clear();
    }

    // Close current DB connection pool so the file is unlocked
    {
        let mut db_guard = state.db.write().await;
        if let Some(db) = db_guard.take() {
            db.close().await.map_err(|e| {
                tracing::error!(error = %e, "Failed to close database connection for restore");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
        }
    }

    // Copy backup over data.db
    std::fs::copy(&backup_path, &db_path).map_err(|e| {
        tracing::error!(error = %e, filename = %filename, "Failed to copy backup file for restore");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    // Remove WAL/SHM files
    let _ = std::fs::remove_file(&wal_path);
    let _ = std::fs::remove_file(&shm_path);

    // Reconnect with proper pool config (init_database already runs migrations)
    let db = ossue_core::db::init_database().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to reconnect to database after restore");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    // Restore log level from the backup's settings
    if let Ok(Some(setting)) = ossue_core::models::settings::Entity::find_by_id("log_level")
        .one(&db)
        .await
    {
        if let Ok(level) = setting
            .value
            .parse::<tracing_subscriber::filter::LevelFilter>()
        {
            let _ = state.log_reload_handle.modify(|f| *f = level);
        }
    }

    // Store the new connection so subsequent operations work without restart
    *state.db.write().await = Some(db);

    tracing::info!(filename = %filename, "Database restored from backup");
    Ok(())
}

#[tauri::command]
pub async fn reset_database(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::warn!("Resetting database - all data will be lost");
    let app_dir = app_data_dir().map_err(|e| {
        tracing::error!(error = %e, "Could not determine application data directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    let db_path = app_dir.join("data.db");
    let wal_path = app_dir.join("data.db-wal");
    let shm_path = app_dir.join("data.db-shm");

    // Cancel all running sync tasks and retry handles
    {
        let mut retry_handles = state.retry_handles.lock().await;
        for (project_id, handle) in retry_handles.drain() {
            handle.abort();
            tracing::info!(project_id = %project_id, "Cancelled pending sync retry during reset");
        }
    }
    {
        let mut syncing = state.syncing_projects.lock().await;
        syncing.clear();
    }

    // Close current DB connection pool so the file is unlocked
    {
        let mut db_guard = state.db.write().await;
        if let Some(db) = db_guard.take() {
            db.close().await.map_err(|e| {
                tracing::error!(error = %e, "Failed to close database connection for reset");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
        }
    }

    // Delete DB files
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&wal_path);
    let _ = std::fs::remove_file(&shm_path);

    // Reconnect with proper pool config (init_database already runs migrations)
    let db = ossue_core::db::init_database().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to reconnect to database after reset");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    // Store the new connection so subsequent operations work without restart
    *state.db.write().await = Some(db);

    // Reset log level to default (WARN) since persisted setting is gone
    let _ = state
        .log_reload_handle
        .modify(|f| *f = tracing_subscriber::filter::LevelFilter::WARN);

    tracing::info!("Database reset completed");
    Ok(())
}

#[tauri::command]
pub async fn delete_backup(filename: String) -> Result<(), CommandError> {
    tracing::info!(filename = %filename, "Deleting backup");
    if !is_valid_backup_filename(&filename) {
        tracing::warn!(filename = %filename, "Invalid backup filename in delete_backup");
        return Err(CommandError::Internal {
            message: "Invalid filename".to_string(),
        });
    }

    let backup_dir = backups_dir().map_err(|e| {
        tracing::error!(error = %e, "Failed to access backups directory");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;
    let backup_path = backup_dir.join(&filename);

    if !backup_path.exists() {
        tracing::error!(filename = %filename, "Backup file not found for deletion");
        return Err(CommandError::NotFound {
            entity: "Backup".to_string(),
            id: filename,
        });
    }

    std::fs::remove_file(&backup_path).map_err(|e| {
        tracing::error!(error = %e, filename = %filename, "Failed to delete backup file");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}
