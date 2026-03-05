use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m002_create_connectors"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Connectors::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Connectors::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Connectors::Name).string().not_null())
                    .col(ColumnDef::new(Connectors::Platform).string().not_null())
                    .col(ColumnDef::new(Connectors::Token).text().not_null())
                    .col(ColumnDef::new(Connectors::BaseUrl).string().null())
                    .col(
                        ColumnDef::new(Connectors::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Connectors::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Connectors::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Connectors {
    Table,
    Id,
    Name,
    Platform,
    Token,
    BaseUrl,
    CreatedAt,
    UpdatedAt,
}
