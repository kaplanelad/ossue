use sea_orm_migration::prelude::*;

use super::m003_create_projects::Projects;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m004_create_items"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Items::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Items::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Items::ProjectId).string().not_null())
                    .col(ColumnDef::new(Items::ItemType).string().not_null())
                    .col(ColumnDef::new(Items::Title).string().not_null())
                    .col(ColumnDef::new(Items::Body).text().not_null().default(""))
                    .col(
                        ColumnDef::new(Items::TypeData)
                            .text()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Items::IsRead)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Items::IsStarred)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Items::IsDeleted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Items::ItemStatus)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(Items::DismissedAt).timestamp().null())
                    .col(
                        ColumnDef::new(Items::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Items::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Items::Table, Items::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_items_project_id")
                    .table(Items::Table)
                    .col(Items::ProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_items_item_type")
                    .table(Items::Table)
                    .col(Items::ItemType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_items_updated_at")
                    .table(Items::Table)
                    .col(Items::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Items::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Items {
    Table,
    Id,
    ProjectId,
    ItemType,
    Title,
    Body,
    TypeData,
    IsRead,
    IsStarred,
    IsDeleted,
    ItemStatus,
    DismissedAt,
    CreatedAt,
    UpdatedAt,
}
