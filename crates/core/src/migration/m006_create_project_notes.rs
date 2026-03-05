use sea_orm_migration::prelude::*;

use super::m003_create_projects::Projects;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m006_create_project_notes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectNotes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectNotes::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProjectNotes::ProjectId).string().not_null())
                    .col(ColumnDef::new(ProjectNotes::NoteType).string().not_null())
                    .col(ColumnDef::new(ProjectNotes::Content).text().not_null())
                    .col(
                        ColumnDef::new(ProjectNotes::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ProjectNotes::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectNotes::Table, ProjectNotes::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_notes_project_id")
                    .table(ProjectNotes::Table)
                    .col(ProjectNotes::ProjectId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProjectNotes::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ProjectNotes {
    Table,
    Id,
    ProjectId,
    NoteType,
    Content,
    CreatedAt,
    UpdatedAt,
}
