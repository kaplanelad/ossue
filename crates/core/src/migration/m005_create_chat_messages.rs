use sea_orm_migration::prelude::*;

use super::m004_create_items::Items;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m005_create_chat_messages"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChatMessages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChatMessages::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ChatMessages::ItemId).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Role).string().not_null())
                    .col(ColumnDef::new(ChatMessages::Content).text().not_null())
                    .col(ColumnDef::new(ChatMessages::InputTokens).integer().null())
                    .col(ColumnDef::new(ChatMessages::OutputTokens).integer().null())
                    .col(ColumnDef::new(ChatMessages::Model).string().null())
                    .col(
                        ColumnDef::new(ChatMessages::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ChatMessages::Table, ChatMessages::ItemId)
                            .to(Items::Table, Items::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_chat_messages_item_id")
                    .table(ChatMessages::Table)
                    .col(ChatMessages::ItemId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ChatMessages::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ChatMessages {
    Table,
    Id,
    ItemId,
    Role,
    Content,
    CreatedAt,
    InputTokens,
    OutputTokens,
    Model,
}
