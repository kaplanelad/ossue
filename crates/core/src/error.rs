#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("Could not find data directory")]
    DataDirectoryNotFound,

    #[error("Database already initialized")]
    AlreadyInitialized,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}
