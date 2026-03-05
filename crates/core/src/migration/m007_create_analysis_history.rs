use sea_orm_migration::prelude::*;

use super::m004_create_items::Items;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m007_create_analysis_history"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AnalysisHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AnalysisHistory::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AnalysisHistory::ItemId).string().not_null())
                    .col(
                        ColumnDef::new(AnalysisHistory::ActionType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AnalysisHistory::ProviderMode)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AnalysisHistory::PromptHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AnalysisHistory::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(AnalysisHistory::Table, AnalysisHistory::ItemId)
                            .to(Items::Table, Items::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_analysis_history_item_id")
                    .table(AnalysisHistory::Table)
                    .col(AnalysisHistory::ItemId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AnalysisHistory::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum AnalysisHistory {
    Table,
    Id,
    ItemId,
    ActionType,
    ProviderMode,
    PromptHash,
    CreatedAt,
}
