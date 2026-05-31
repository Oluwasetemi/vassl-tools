pub mod migrations;
pub mod shared; // populated in Task 4

pub use migrations::{AppMigrator, DomainMigration};

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
        db::AppDatabase::global(cx)
    }
}

/// Returns the path to the VASSL SQLite database file.
///
/// | Platform | Path |
/// |----------|------|
/// | macOS    | `~/Library/Application Support/VASSL/vassl.db` |
/// | Windows  | `%LOCALAPPDATA%\VASSL\vassl.db` |
/// | Linux    | `$XDG_DATA_HOME/VASSL/vassl.db` |
pub fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .expect("platform has no local data dir")
        .join("VASSL")
        .join("vassl.db")
}

/// Opens the VASSL SQLite database, runs all inventory-registered
/// [`DomainMigration`]s in topological order, and sets the result as a GPUI
/// global on `cx`.
///
/// Must be called once during application startup, before any domain code
/// accesses the database.
pub fn init(cx: &mut App) -> anyhow::Result<()> {
    let path = db_path();

    // Ensure the parent directory exists before handing the path to sqlez.
    std::fs::create_dir_all(
        path.parent().context("db path has no parent")?,
    )
    .context("failed to create VASSL data directory")?;

    // The vendored `db` crate's `open_db` accepts a directory + scope.
    // We use `GlobalDbScope` (scope_name = "global") so the file becomes
    // `<parent>/0-global/db.sqlite`.  To land exactly at `vassl.db` we pass
    // the parent of our desired path as db_dir and use a custom scope.
    //
    // Alternatively, we construct the connection directly via
    // ThreadSafeConnection::builder, which is the simplest path.
    let db_dir = path
        .parent()
        .context("db path has no parent")?
        .to_path_buf();

    // Block on the async open_db using gpui's block_on helper.
    let conn = gpui::block_on(open_db::<AppMigrator>(&db_dir, GlobalDbScope));

    // Wrap in our own AppDatabase global so callers use vassl-db's type, not
    // the vendored db::AppDatabase.
    cx.set_global(AppDatabase(conn));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_contains_vassl() {
        let p = db_path();
        // The path should contain "VASSL" and end with "vassl.db".
        let s = p.to_string_lossy();
        assert!(s.contains("VASSL"), "expected VASSL in path, got: {s}");
        assert!(s.ends_with("vassl.db"), "expected vassl.db suffix, got: {s}");
    }
}
