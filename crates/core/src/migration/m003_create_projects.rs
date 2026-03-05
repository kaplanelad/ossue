use sea_orm_migration::prelude::*;

use super::m002_create_connectors::Connectors;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m003_create_projects"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Projects::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Projects::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Projects::Name).string().not_null())
                    .col(ColumnDef::new(Projects::Owner).string().not_null())
                    .col(ColumnDef::new(Projects::Platform).string().not_null())
                    .col(ColumnDef::new(Projects::Url).string().not_null())
                    .col(ColumnDef::new(Projects::ClonePath).string().null())
                    .col(ColumnDef::new(Projects::ApiToken).string().null())
                    .col(ColumnDef::new(Projects::ConnectorId).string().null())
                    .col(
                        ColumnDef::new(Projects::ExternalProjectId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Projects::SyncEnabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Projects::LastSyncAt).timestamp().null())
                    .col(ColumnDef::new(Projects::LastSyncError).text().null())
                    .col(
                        ColumnDef::new(Projects::FullReconciliationAt)
                            .timestamp()
                            .null(),
                    )
                    .col(ColumnDef::new(Projects::DefaultBranch).string().null())
                    .col(
                        ColumnDef::new(Projects::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Projects::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Projects::Table, Projects::ConnectorId)
                            .to(Connectors::Table, Connectors::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Projects::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Projects {
    Table,
    Id,
    Name,
    Owner,
    Platform,
    Url,
    ClonePath,
    ApiToken,
    ConnectorId,
    ExternalProjectId,
    SyncEnabled,
    LastSyncAt,
    LastSyncError,
    FullReconciliationAt,
    DefaultBranch,
    CreatedAt,
    UpdatedAt,
}
