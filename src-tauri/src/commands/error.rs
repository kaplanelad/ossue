use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum CommandError {
    DatabaseNotReady,
    NotFound { entity: String, id: String },
    SyncInProgress { project_id: String },
    AiNotConfigured,
    PlatformApi { message: String },
    Internal { message: String },
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DatabaseNotReady => write!(f, "Database not initialized"),
            Self::NotFound { entity, id } => write!(f, "{entity} not found: {id}"),
            Self::SyncInProgress { project_id } => {
                write!(f, "Sync already in progress for project {project_id}")
            }
            Self::AiNotConfigured => write!(f, "AI is not configured"),
            Self::PlatformApi { message } => write!(f, "Platform API error: {message}"),
            Self::Internal { message } => write!(f, "Internal error: {message}"),
        }
    }
}

impl From<sea_orm::DbErr> for CommandError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self::Internal {
            message: e.to_string(),
        }
    }
}

// Convert to String for Tauri (Tauri commands need Result<T, String> or Result<T, impl Serialize>)
// We serialize as JSON to preserve the structured error info
impl From<CommandError> for String {
    fn from(e: CommandError) -> String {
        serde_json::to_string(&e).unwrap_or_else(|_| e.to_string())
    }
}
