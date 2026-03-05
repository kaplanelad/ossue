#[cfg(debug_assertions)]
pub const APP_DIR_NAME: &str = "ossue-dev";
#[cfg(not(debug_assertions))]
pub const APP_DIR_NAME: &str = "ossue";

pub mod db;
pub mod enums;
pub mod error;
pub mod logging;
pub mod migration;
pub mod models;
pub mod queries;
pub mod services;
pub mod sync;

#[cfg(test)]
pub mod test_helpers;
