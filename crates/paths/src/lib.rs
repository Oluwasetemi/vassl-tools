//! Minimal stub of the Zed `paths` crate.
//! Provides only `database_dir`, which `db` re-exports.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Application name used to derive data directory.
pub const APP_NAME: &str = "Vassl";

/// Returns the application's data directory for storing the SQLite database.
pub fn data_dir() -> &'static PathBuf {
    static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
    DATA_DIR.get_or_init(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    })
}

/// Returns the path to the database directory (`<data_dir>/db`).
pub fn database_dir() -> &'static PathBuf {
    static DATABASE_DIR: OnceLock<PathBuf> = OnceLock::new();
    DATABASE_DIR.get_or_init(|| data_dir().join("db"))
}
