use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::enums::Platform;

#[derive(Clone, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub owner: String,
    pub platform: Platform,
    pub url: String,
    pub clone_path: Option<String>,
    pub default_branch: Option<String>,
    pub api_token: Option<String>,
    pub connector_id: Option<String>,
    pub external_project_id: Option<i64>,
    pub sync_enabled: bool,
    pub last_sync_at: Option<chrono::NaiveDateTime>,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_sync_error: Option<String>,
    pub full_reconciliation_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("owner", &self.owner)
            .field("platform", &self.platform)
            .field("url", &self.url)
            .field("clone_path", &self.clone_path)
            .field("default_branch", &self.default_branch)
            .field("api_token", &"[REDACTED]")
            .field("connector_id", &self.connector_id)
            .field("external_project_id", &self.external_project_id)
            .field("sync_enabled", &self.sync_enabled)
            .field("last_sync_at", &self.last_sync_at)
            .field("last_sync_error", &self.last_sync_error)
            .field("full_reconciliation_at", &self.full_reconciliation_at)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::item::Entity")]
    Items,
    #[sea_orm(has_many = "super::project_note::Entity")]
    ProjectNotes,
    #[sea_orm(
        belongs_to = "super::connector::Entity",
        from = "Column::ConnectorId",
        to = "super::connector::Column::Id"
    )]
    Connector,
}

impl Related<super::item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Items.def()
    }
}

impl Related<super::project_note::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectNotes.def()
    }
}

impl Related<super::connector::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Connector.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
