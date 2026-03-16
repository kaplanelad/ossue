use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::enums::MessageRole;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "chat_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub item_id: String,
    pub role: MessageRole,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub created_at: chrono::NaiveDateTime,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub model: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::item::Entity",
        from = "Column::ItemId",
        to = "super::item::Column::Id"
    )]
    Item,
}

impl Related<super::item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Item.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
