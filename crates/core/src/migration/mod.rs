use sea_orm_migration::prelude::*;

mod m001_create_settings;
pub mod m002_create_connectors;
pub mod m003_create_projects;
pub mod m004_create_items;
mod m005_create_chat_messages;
mod m006_create_project_notes;
mod m007_create_analysis_history;
mod m008_create_project_settings;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m001_create_settings::Migration),
            Box::new(m002_create_connectors::Migration),
            Box::new(m003_create_projects::Migration),
            Box::new(m004_create_items::Migration),
            Box::new(m005_create_chat_messages::Migration),
            Box::new(m006_create_project_notes::Migration),
            Box::new(m007_create_analysis_history::Migration),
            Box::new(m008_create_project_settings::Migration),
        ]
    }
}
