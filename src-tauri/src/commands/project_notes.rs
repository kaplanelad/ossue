use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::NoteType;
use ossue_core::models::project_note;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectNoteResponse {
    pub id: String,
    pub project_id: String,
    pub note_type: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<project_note::Model> for ProjectNoteResponse {
    fn from(m: project_note::Model) -> Self {
        Self {
            id: m.id,
            project_id: m.project_id,
            note_type: m.note_type.to_string(),
            content: m.content,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
        }
    }
}

#[tauri::command]
pub async fn list_project_notes(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ProjectNoteResponse>, CommandError> {
    tracing::debug!(project_id = %project_id, "Listing project notes");
    let db = state.get_db().await?;

    let notes = project_note::Entity::find()
        .filter(project_note::Column::ProjectId.eq(&project_id))
        .order_by_asc(project_note::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %project_id, "Failed to query project notes");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    tracing::debug!(project_id = %project_id, count = notes.len(), "Retrieved project notes");

    Ok(notes.into_iter().map(ProjectNoteResponse::from).collect())
}

#[tauri::command]
pub async fn add_project_note(
    state: State<'_, AppState>,
    project_id: String,
    content: String,
) -> Result<ProjectNoteResponse, CommandError> {
    tracing::info!(project_id = %project_id, "Adding project note");
    let db = state.get_db().await?;

    let now = chrono::Utc::now().naive_utc();

    let model = project_note::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(project_id.clone()),
        note_type: Set(NoteType::Manual),
        content: Set(content),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let saved = model.insert(&db).await.map_err(|e| {
        tracing::error!(error = %e, project_id = %project_id, "Failed to insert project note");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(ProjectNoteResponse::from(saved))
}

#[tauri::command]
pub async fn remove_project_note(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), CommandError> {
    tracing::info!(id = %id, "Removing project note");
    let db = state.get_db().await?;

    project_note::Entity::delete_by_id(&id)
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to delete project note");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}
