use std::path::PathBuf;

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::error::InitError;
use crate::migration::Migrator;

pub fn app_data_dir() -> Result<PathBuf, InitError> {
    dirs::data_dir()
        .map(|d| d.join(crate::APP_DIR_NAME))
        .ok_or(InitError::DataDirectoryNotFound)
}

pub fn backups_dir() -> Result<PathBuf, std::io::Error> {
    let dir = app_data_dir()
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not find data directory",
            )
        })?
        .join("backups");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub async fn init_database() -> Result<DatabaseConnection, InitError> {
    let app_dir = app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("data.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    tracing::debug!(db_url = %db_url, "Connecting to database");

    let mut opts = ConnectOptions::new(&db_url);
    opts.max_connections(4)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(opts).await?;

    // Enable WAL mode for better concurrent read/write performance
    db.execute_unprepared("PRAGMA journal_mode=WAL").await?;
    // Enable foreign key enforcement (disabled by default in SQLite)
    db.execute_unprepared("PRAGMA foreign_keys=ON").await?;

    // Run migrations
    Migrator::up(&db, None).await?;

    tracing::info!("Database initialized successfully");

    Ok(db)
}
