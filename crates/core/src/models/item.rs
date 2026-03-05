use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::enums::{ItemStatus, ItemType, ItemTypeData};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub project_id: String,
    pub item_type: ItemType,
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub body: String,
    #[sea_orm(column_type = "Text")]
    pub type_data: String,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_deleted: bool,
    pub item_status: ItemStatus,
    pub dismissed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl Model {
    pub fn parse_type_data(&self) -> Result<ItemTypeData, serde_json::Error> {
        serde_json::from_str(&self.type_data)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::project::Entity",
        from = "Column::ProjectId",
        to = "super::project::Column::Id"
    )]
    Project,
    #[sea_orm(has_many = "super::chat_message::Entity")]
    ChatMessages,
}

impl Related<super::project::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Project.def()
    }
}

impl Related<super::chat_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChatMessages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
