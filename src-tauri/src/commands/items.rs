use sea_orm::{sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::{ItemStatus, Platform};
use ossue_core::models::item;
use ossue_core::models::project;

use super::repos::{get_project_base_url, get_project_token};

#[derive(Clone, Serialize)]
pub struct SyncProgressPayload {
    pub project_id: String,
    pub phase: String,
    pub page: u32,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct SyncItemsPayload {
    pub project_id: String,
    pub items: Vec<ItemResponse>,
}

#[derive(Clone, Serialize)]
pub struct SyncCompletePayload {
    pub project_id: String,
    pub total_items: usize,
}

#[derive(Clone, Serialize)]
pub struct SyncErrorPayload {
    pub project_id: String,
    pub error: String,
    pub retry_in_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemResponse {
    pub id: String,
    pub project_id: String,
    pub item_type: String,
    pub title: String,
    pub body: String,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_deleted: bool,
    pub item_status: String,
    pub type_data: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<item::Model> for ItemResponse {
    fn from(m: item::Model) -> Self {
        Self {
            id: m.id,
            project_id: m.project_id,
            item_type: m.item_type.to_string(),
            title: m.title,
            body: m.body,
            is_read: m.is_read,
            is_starred: m.is_starred,
            is_deleted: m.is_deleted,
            item_status: m.item_status.to_string(),
            type_data: serde_json::from_str(&m.type_data).unwrap_or(serde_json::Value::Null),
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemPageResponse {
    pub items: Vec<ItemResponse>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub dismissed_counts: Vec<ossue_core::queries::DismissedCount>,
    pub item_type_counts: Vec<ossue_core::queries::ItemTypeCount>,
    pub starred_counts: Vec<ossue_core::queries::ItemTypeCount>,
    pub analyzed_counts: Vec<ossue_core::queries::ItemTypeCount>,
    pub draft_note_counts: Vec<ossue_core::queries::ItemTypeCount>,
}

#[tauri::command]
pub async fn list_items(
    state: State<'_, AppState>,
    project_id: Option<String>,
    item_type: Option<String>,
    starred_only: Option<bool>,
    search_query: Option<String>,
    cursor: Option<String>,
    page_size: Option<u32>,
) -> Result<ItemPageResponse, CommandError> {
    tracing::debug!(project_id = ?project_id, item_type = ?item_type, search_query = ?search_query, cursor = ?cursor, "Listing items");
    let db = state.get_db().await?;

    let (page, dismissed_counts, item_type_counts, starred_counts, analyzed_counts, draft_note_counts) = tokio::try_join!(
        async {
            ossue_core::queries::list_items(
                &db,
                ossue_core::queries::ListItemsParams {
                    project_id,
                    item_type,
                    starred_only: starred_only.unwrap_or(false),
                    search_query,
                    cursor,
                    page_size: page_size.unwrap_or(50),
                    dismissed: false,
                },
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to query items from database");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })
        },
        async {
            ossue_core::queries::count_dismissed_grouped(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count dismissed items");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_pending_by_type(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count items by type");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_starred_pending(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count starred items");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_analyzed_pending(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count analyzed items");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_draft_notes_grouped(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count draft notes");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        }
    )?;

    Ok(ItemPageResponse {
        items: page.items.into_iter().map(ItemResponse::from).collect(),
        next_cursor: page.next_cursor,
        has_more: page.has_more,
        dismissed_counts,
        item_type_counts,
        starred_counts,
        analyzed_counts,
        draft_note_counts,
    })
}

#[tauri::command]
pub async fn get_item(
    state: State<'_, AppState>,
    id: String,
) -> Result<ItemResponse, CommandError> {
    tracing::debug!(id = %id, "Getting item");
    let db = state.get_db().await?;

    let model = item::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query item from database");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Item not found");
            CommandError::NotFound {
                entity: "Item".to_string(),
                id: id.clone(),
            }
        })?;

    Ok(ItemResponse::from(model))
}

#[tauri::command]
pub async fn mark_item_read(
    state: State<'_, AppState>,
    id: String,
    is_read: bool,
) -> Result<(), CommandError> {
    tracing::debug!(id = %id, is_read = is_read, "Marking item read status");
    let db = state.get_db().await?;

    let item = item::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query item for read status update");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Item not found for read status update");
            CommandError::NotFound {
                entity: "Item".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: item::ActiveModel = item.into();
    active.is_read = Set(is_read);
    active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update item read status");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}

#[tauri::command]
pub async fn toggle_item_star(
    state: State<'_, AppState>,
    id: String,
    is_starred: bool,
) -> Result<(), CommandError> {
    tracing::debug!(id = %id, is_starred = is_starred, "Toggling item star status");
    let db = state.get_db().await?;

    let item = item::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query item for star status update");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Item not found for star status update");
            CommandError::NotFound {
                entity: "Item".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: item::ActiveModel = item.into();
    active.is_starred = Set(is_starred);
    active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update item star status");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}

#[tauri::command]
pub async fn delete_item(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    tracing::debug!(id = %id, "Soft-deleting item");
    let db = state.get_db().await?;

    let item = item::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query item for deletion");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Item not found for deletion");
            CommandError::NotFound {
                entity: "Item".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: item::ActiveModel = item.into();
    active.item_status = Set(ItemStatus::Dismissed);
    active.dismissed_at = Set(Some(chrono::Utc::now().naive_utc()));
    active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to dismiss item");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}

#[tauri::command]
pub async fn list_dismissed_items(
    state: State<'_, AppState>,
    project_id: Option<String>,
    item_type: Option<String>,
    search_query: Option<String>,
    cursor: Option<String>,
    page_size: Option<u32>,
) -> Result<ItemPageResponse, CommandError> {
    tracing::debug!(project_id = ?project_id, item_type = ?item_type, search_query = ?search_query, cursor = ?cursor, "Listing dismissed items");
    let db = state.get_db().await?;

    let (page, dismissed_counts, item_type_counts, starred_counts, analyzed_counts, draft_note_counts) = tokio::try_join!(
        async {
            ossue_core::queries::list_items(
                &db,
                ossue_core::queries::ListItemsParams {
                    project_id,
                    item_type,
                    starred_only: false,
                    search_query,
                    cursor,
                    page_size: page_size.unwrap_or(50),
                    dismissed: true,
                },
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to query dismissed items from database");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })
        },
        async {
            ossue_core::queries::count_dismissed_grouped(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count dismissed items");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_pending_by_type(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count items by type");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_starred_pending(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count starred items");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_analyzed_pending(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count analyzed items");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        },
        async {
            ossue_core::queries::count_draft_notes_grouped(&db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to count draft notes");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })
        }
    )?;

    Ok(ItemPageResponse {
        items: page.items.into_iter().map(ItemResponse::from).collect(),
        next_cursor: page.next_cursor,
        has_more: page.has_more,
        dismissed_counts,
        item_type_counts,
        starred_counts,
        analyzed_counts,
        draft_note_counts,
    })
}

#[tauri::command]
pub async fn restore_item(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    tracing::debug!(id = %id, "Restoring dismissed item");
    let db = state.get_db().await?;

    let item = item::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query item for restore");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Item not found for restore");
            CommandError::NotFound {
                entity: "Item".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: item::ActiveModel = item.into();
    active.item_status = Set(ItemStatus::Pending);
    active.dismissed_at = Set(None);
    active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to restore item");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}

#[tauri::command]
pub async fn clear_project_data(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), CommandError> {
    tracing::info!(project_id = %project_id, "Clearing all data for project");
    let db = state.get_db().await?;

    // Delete all items for this project (CASCADE handles chat_messages, analysis_history)
    let result = item::Entity::delete_many()
        .filter(item::Column::ProjectId.eq(&project_id))
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, "Failed to delete items");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    tracing::info!(project_id = %project_id, deleted_items = result.rows_affected, "Project data cleared");
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
pub enum SyncMode {
    Incremental,
    Full,
}

async fn start_sync(
    state: &AppState,
    project_id: String,
    app_handle: tauri::AppHandle,
    mode: SyncMode,
) -> Result<(), CommandError> {
    let sync_type = if mode == SyncMode::Full {
        "full sync"
    } else {
        "sync"
    };
    tracing::info!(project_id = %project_id, sync_type, "Starting sync for project items");

    let db_arc = state.db.clone();
    let syncing_arc = state.syncing_projects.clone();
    let retry_handles_arc = state.retry_handles.clone();

    // Cancel pending retry if any
    if let Some(handle) = retry_handles_arc.lock().await.remove(&project_id) {
        handle.abort();
        tracing::info!(project_id = %project_id, "Cancelled pending sync retry");
    }

    let db = db_arc
        .read()
        .await
        .clone()
        .ok_or(CommandError::DatabaseNotReady)?;

    let proj = project::Entity::find_by_id(&project_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, "Failed to query project for sync");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(project_id = %project_id, "Project not found for sync");
            CommandError::NotFound {
                entity: "Project".to_string(),
                id: project_id.clone(),
            }
        })?;

    if !proj.sync_enabled {
        tracing::info!(project_id = %project_id, "Project sync is disabled, skipping");
        let _ = app_handle.emit(
            "sync:complete",
            SyncCompletePayload {
                project_id: project_id.clone(),
                total_items: 0,
            },
        );
        return Ok(());
    }

    // Check concurrency guard AFTER validation succeeds
    {
        let mut syncing = syncing_arc.lock().await;
        if syncing.contains(&project_id) {
            tracing::info!(project_id = %project_id, "Project is already syncing, skipping");
            let _ = app_handle.emit(
                "sync:complete",
                SyncCompletePayload {
                    project_id: project_id.clone(),
                    total_items: 0,
                },
            );
            return Ok(());
        }
        syncing.insert(project_id.clone());
    }

    // Full sync: reset item_status to pending (except deleted items which are permanent)
    if mode == SyncMode::Full {
        if let Err(e) = item::Entity::update_many()
            .col_expr(item::Column::ItemStatus, Expr::value("pending"))
            .filter(item::Column::ProjectId.eq(&project_id))
            .filter(item::Column::ItemStatus.ne("deleted"))
            .exec(&db)
            .await
        {
            tracing::error!(error = %e, project_id = %project_id, "Failed to reset item_status for full sync");
            syncing_arc.lock().await.remove(&project_id);
            return Err(CommandError::Internal {
                message: e.to_string(),
            });
        }
    }

    let project_id_spawn = project_id.clone();
    let app_handle_spawn = app_handle.clone();
    let retry_handles_spawn = retry_handles_arc.clone();
    let syncing_spawn = syncing_arc.clone();
    let is_full = mode == SyncMode::Full;

    tauri::async_runtime::spawn(async move {
        let result = do_sync_for_project(&db, &proj, &app_handle_spawn, is_full).await;

        match result {
            Ok(total) => {
                // Cancel any pending retry on success
                if let Some(handle) = retry_handles_spawn.lock().await.remove(&project_id_spawn) {
                    handle.abort();
                }
                let _ = app_handle_spawn.emit(
                    "sync:complete",
                    SyncCompletePayload {
                        project_id: project_id_spawn.clone(),
                        total_items: total,
                    },
                );
                tracing::info!(project_id = %project_id_spawn, total_items = total, sync_type, "Sync completed");
            }
            Err(e) => {
                let retry_secs = schedule_sync_retry(
                    app_handle_spawn.clone(),
                    db.clone(),
                    project_id_spawn.clone(),
                    syncing_spawn.clone(),
                    retry_handles_spawn.clone(),
                    1,
                );
                let error_msg = e.to_string();
                let _ = app_handle_spawn.emit(
                    "sync:error",
                    SyncErrorPayload {
                        project_id: project_id_spawn.clone(),
                        error: error_msg.clone(),
                        retry_in_secs: retry_secs,
                    },
                );
                tracing::error!(project_id = %project_id_spawn, error = %error_msg, sync_type, "Sync failed");
            }
        }

        // Release concurrency guard
        syncing_arc.lock().await.remove(&project_id_spawn);
    });

    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(project_id = %project_id))]
pub async fn full_sync_project_items(
    state: State<'_, AppState>,
    project_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), CommandError> {
    start_sync(&state, project_id, app_handle, SyncMode::Full).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(project_id = %project_id))]
pub async fn sync_project_items(
    state: State<'_, AppState>,
    project_id: String,
    app_handle: tauri::AppHandle,
) -> Result<(), CommandError> {
    start_sync(&state, project_id, app_handle, SyncMode::Incremental).await
}

struct TauriProgressSink {
    app_handle: tauri::AppHandle,
    project_id: String,
}

#[async_trait::async_trait]
impl ossue_core::services::sync_orchestrator::ProgressSink for TauriProgressSink {
    fn emit_progress(&self, phase: &str, page: u32, message: &str) {
        let _ = self.app_handle.emit(
            "sync:progress",
            SyncProgressPayload {
                project_id: self.project_id.clone(),
                phase: phase.to_string(),
                page,
                message: message.to_string(),
            },
        );
    }
    fn emit_items(&self, items: Vec<ossue_core::models::item::Model>) {
        let _ = self.app_handle.emit(
            "sync:items",
            SyncItemsPayload {
                project_id: self.project_id.clone(),
                items: items.iter().cloned().map(ItemResponse::from).collect(),
            },
        );
    }
    fn emit_complete(&self, total: usize) {
        let _ = self.app_handle.emit(
            "sync:complete",
            SyncCompletePayload {
                project_id: self.project_id.clone(),
                total_items: total,
            },
        );
    }
    fn emit_error(&self, error: &str, retry_in: Option<u64>) {
        let _ = self.app_handle.emit(
            "sync:error",
            SyncErrorPayload {
                project_id: self.project_id.clone(),
                error: error.to_string(),
                retry_in_secs: retry_in,
            },
        );
    }
}

pub(crate) async fn do_sync_for_project(
    db: &sea_orm::DatabaseConnection,
    proj: &project::Model,
    app_handle: &tauri::AppHandle,
    is_full_reconciliation: bool,
) -> Result<usize, CommandError> {
    let token = get_project_token(db, proj).await.map_err(|e| {
        tracing::error!(project_id = %proj.id, error = %e, "Failed to resolve token for sync");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    let progress = TauriProgressSink {
        app_handle: app_handle.clone(),
        project_id: proj.id.clone(),
    };

    match proj.platform {
        Platform::GitHub => ossue_core::services::sync_orchestrator::sync_github_items(
            db,
            proj,
            &token,
            &progress,
            is_full_reconciliation,
        )
        .await
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        }),
        Platform::GitLab => {
            let base_url = get_project_base_url(db, proj).await;
            ossue_core::services::sync_orchestrator::sync_gitlab_items(
                db,
                proj,
                &token,
                base_url,
                &progress,
                is_full_reconciliation,
            )
            .await
            .map_err(|e| CommandError::Internal {
                message: e.to_string(),
            })
        }
    }
}

fn schedule_sync_retry(
    app_handle: tauri::AppHandle,
    db: sea_orm::DatabaseConnection,
    project_id: String,
    syncing_projects: Arc<Mutex<HashSet<String>>>,
    retry_handles: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    attempt: u32,
) -> Option<u64> {
    // Escalating backoff: 5min, 10min, 30min, 1h cap
    let delay_secs = match attempt {
        1 => 300,
        2 => 600,
        3 => 1800,
        _ => 3600,
    };

    tracing::warn!(
        project_id = %project_id,
        attempt = attempt,
        delay_secs = delay_secs,
        "Scheduling sync retry"
    );

    let project_id_clone = project_id.clone();
    let retry_handles_clone = retry_handles.clone();
    let handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

        // Remove self from retry_handles
        retry_handles_clone.lock().await.remove(&project_id_clone);

        tracing::info!(project_id = %project_id_clone, attempt = attempt, "Executing scheduled sync retry");

        // Look up the project and trigger sync
        let proj = {
            project::Entity::find_by_id(&project_id_clone)
                .one(&db)
                .await
                .ok()
                .flatten()
        };

        if let Some(proj) = proj {
            if !proj.sync_enabled {
                tracing::info!(project_id = %project_id_clone, "Sync disabled, skipping retry");
                return;
            }

            // Check concurrency guard
            {
                let mut syncing = syncing_projects.lock().await;
                if syncing.contains(&project_id_clone) {
                    return;
                }
                syncing.insert(project_id_clone.clone());
            }

            let result = do_sync_for_project(&db, &proj, &app_handle, false).await;

            match result {
                Ok(total) => {
                    // Cancel any pending retry on success
                    retry_handles_clone.lock().await.remove(&project_id_clone);
                    let _ = app_handle.emit(
                        "sync:complete",
                        SyncCompletePayload {
                            project_id: project_id_clone.clone(),
                            total_items: total,
                        },
                    );
                }
                Err(e) => {
                    let retry_secs = schedule_sync_retry(
                        app_handle.clone(),
                        db.clone(),
                        project_id_clone.clone(),
                        syncing_projects.clone(),
                        retry_handles_clone.clone(),
                        attempt + 1,
                    );
                    let _ = app_handle.emit(
                        "sync:error",
                        SyncErrorPayload {
                            project_id: project_id_clone.clone(),
                            error: e.to_string(),
                            retry_in_secs: retry_secs,
                        },
                    );
                }
            }

            syncing_projects.lock().await.remove(&project_id_clone);
        }
    });

    // Store handle so it can be cancelled
    tauri::async_runtime::spawn({
        let project_id = project_id.clone();
        async move {
            retry_handles.lock().await.insert(project_id, handle);
        }
    });

    Some(delay_secs)
}
