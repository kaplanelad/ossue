use sea_orm_migration::prelude::*;

use super::m003_create_projects::Projects;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m008_create_project_settings"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectSettings::ProjectId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProjectSettings::Key).string().not_null())
                    .col(ColumnDef::new(ProjectSettings::Value).text().not_null())
                    .primary_key(
                        Index::create()
                            .col(ProjectSettings::ProjectId)
                            .col(ProjectSettings::Key),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectSettings::Table, ProjectSettings::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProjectSettings::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ProjectSettings {
    Table,
    ProjectId,
    Key,
    Value,
}
