pub mod migrations;
pub mod shared; // populated in Task 4

pub use migrations::{AppMigrator, DomainMigration};
pub use shared::SharedDomain;

// Re-export write_and_log from the vendored db crate.  Module crates should
// use this instead of importing `db` directly.
pub use db::write_and_log;

// Re-export the static_connection! macro so module crates can register their
// domain without depending on `db` directly.
pub use db::static_connection;

use anyhow::Context as _;
use db::{GlobalDbScope, open_db};
use gpui::{App, Global};
use sqlez::thread_safe_connection::ThreadSafeConnection;
use std::path::PathBuf;

// Alias so set_global calls below are unambiguous.
use db::AppDatabase as VendoredAppDatabase;

/// The VASSL application database.
///
/// Wraps a `ThreadSafeConnection` (from `sqlez`) and is registered as a GPUI
/// [`Global`] on the [`App`] via [`init`].  All domain-specific DB wrappers
/// (inventory, quotations, pricebook …) obtain a clone of this connection via
/// [`AppDatabase::global`].
pub struct AppDatabase(pub ThreadSafeConnection);

impl Global for AppDatabase {}

impl AppDatabase {
    /// Returns a reference to the underlying connection stored in the per-App
    /// global.  Panics if [`init`] has not been called.
    pub fn global(cx: &App) -> &ThreadSafeConnection {
        &cx.global::<AppDatabase>().0
    }
}

/// Returns the path to the VASSL SQLite database file.
///
/// | Platform | Path |
/// |----------|------|
/// | macOS    | `~/Library/Application Support/VASSL/0-global/db.sqlite` |
/// | Windows  | `%LOCALAPPDATA%\VASSL\0-global\db.sqlite` |
/// | Linux    | `$XDG_DATA_HOME/VASSL/0-global/db.sqlite` |
pub fn db_path() -> PathBuf {
    // PANIC: if the OS has no local data directory there is nowhere to store the
    // database and the application cannot function.  No recovery path exists.
    dirs::data_local_dir()
        .expect("OS has no local data directory (required for database storage)")
        .join("VASSL")
        .join("0-global")
        .join("db.sqlite")
}

/// Opens the VASSL SQLite database, runs all inventory-registered
/// [`DomainMigration`]s in topological order, and sets the result as a GPUI
/// global on `cx`.
///
/// Must be called once during application startup, before any domain code
/// accesses the database.
pub fn init(cx: &mut App) -> anyhow::Result<()> {
    let full_path = db_path();
    let db_dir = full_path
        .parent()
        .context("db path has no parent")?
        .parent()
        .context("db path grandparent missing")?
        .to_path_buf();
    // db_dir is now data_local_dir()/VASSL
    std::fs::create_dir_all(&db_dir).context("failed to create VASSL data directory")?;
    let conn = gpui::block_on(open_db::<AppMigrator>(&db_dir, GlobalDbScope));
    // static_connection! (re-exported from vendored `db`) expands $crate::AppDatabase
    // to db::AppDatabase, so we must register both globals.
    cx.set_global(VendoredAppDatabase(conn.clone()));
    cx.set_global(AppDatabase(conn));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_contains_vassl() {
        let path = db_path();
        let s = path.to_string_lossy();
        assert!(s.contains("VASSL"), "db_path should contain VASSL, got: {s}");
        assert!(s.ends_with("db.sqlite"), "db_path should end with db.sqlite, got: {s}");
    }
}
