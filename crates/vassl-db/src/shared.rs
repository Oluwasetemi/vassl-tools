use anyhow::{Context as _, Result};
use sqlez::connection::Connection;

use crate::migrations::DomainMigration;

// ---------------------------------------------------------------------------
// Migration registration
// ---------------------------------------------------------------------------

const SHARED_MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS audit_log (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        table_name TEXT NOT NULL,
        record_id  INTEGER NOT NULL,
        action     TEXT NOT NULL,
        changed_by TEXT NOT NULL,
        changed_at TEXT NOT NULL,
        old_value  TEXT,
        new_value  TEXT
    )",
];

inventory::submit! {
    DomainMigration {
        name: "shared",
        dependencies: &[],
        migrations: SHARED_MIGRATIONS,
        should_allow_migration_change: |_index, _old, _new| true,
    }
}

// ---------------------------------------------------------------------------
// Named type — lets downstream crates reference "shared" in static_connection!
// dep lists WITHOUT re-registering migrations (no inventory::submit! here).
// ---------------------------------------------------------------------------

/// Marker type for the "shared" domain.
///
/// Implement `sqlez::domain::Domain` so that `static_connection!` can list it
/// as a dependency, but do NOT call `inventory::submit!` — the real migration
/// is already submitted above.
pub struct SharedDomain;

impl sqlez::domain::Domain for SharedDomain {
    const NAME: &'static str = "shared";
    const MIGRATIONS: &'static [&'static str] = &[];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Read a setting from the `settings` table.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.select_row_bound::<&str, String>("SELECT value FROM settings WHERE key = (?)")
        .context("failed to prepare settings SELECT")?
        (key)
}

/// Write (upsert) a setting into the `settings` table.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.exec_bound::<(&str, &str)>(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ((?), (?))",
    )
    .context("failed to prepare settings UPSERT")?
    ((key, value))
}

/// Read the `current_user` setting (returns `None` when not yet set).
pub fn current_user(conn: &Connection) -> Result<Option<String>> {
    get_setting(conn, "current_user")
}

/// Persist the `current_user` setting.
pub fn set_current_user(conn: &Connection, name: &str) -> Result<()> {
    set_setting(conn, "current_user", name)
}

/// Update the `new_value` column of an existing audit entry (used to patch
/// the "in-progress" name-change entry as the user keeps typing, so only one
/// row is produced per editing session rather than one row per keystroke).
pub fn update_audit_new_value(conn: &Connection, id: i64, new_value: &str) -> Result<()> {
    conn.exec_bound::<(&str, i64)>(
        "UPDATE audit_log SET new_value = (?) WHERE id = (?)",
    )
    .context("failed to prepare audit_log UPDATE")?
    ((new_value, id))
}

/// Append a row to the `audit_log` table. Returns the new row's id.
#[allow(clippy::too_many_arguments)]
pub fn log_audit(
    conn: &Connection,
    table_name: &str,
    record_id: i64,
    action: &str,
    changed_by: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
) -> Result<i64> {
    let changed_at = chrono::Utc::now().to_rfc3339();
    conn.exec_bound::<(&str, i64, &str, &str, &str, Option<&str>, Option<&str>)>(
        "INSERT INTO audit_log \
            (table_name, record_id, action, changed_by, changed_at, old_value, new_value) \
            VALUES ((?), (?), (?), (?), (?), (?), (?))",
    )
    .context("failed to prepare audit_log INSERT")?
    ((table_name, record_id, action, changed_by, &changed_at, old_value, new_value))?;
    conn.select_row::<i64>("SELECT last_insert_rowid()")
        .context("failed to prepare last_insert_rowid")?()
        .context("failed to execute last_insert_rowid")?
        .context("last_insert_rowid returned None")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Open an in-memory SQLite connection and run the three shared migrations
    /// directly (without going through the full inventory machinery, which
    /// requires all registered crates to be linked).
    ///
    /// Each call uses a unique URI so parallel tests do not share the same
    /// named in-memory database (which would cause schema-locked errors).
    fn open_test_conn(name: &str) -> Connection {
        let conn = Connection::open_memory(Some(name));
        conn.migrate("shared", SHARED_MIGRATIONS, &mut |_, _, _| false)
            .expect("shared migrations failed");
        conn
    }

    #[test]
    fn test_settings_roundtrip() {
        let conn = open_test_conn("shared_settings_test");

        // Initially absent
        assert_eq!(get_setting(&conn, "current_user").unwrap(), None);

        // Set once
        set_setting(&conn, "current_user", "Alice").unwrap();
        assert_eq!(
            get_setting(&conn, "current_user").unwrap(),
            Some("Alice".to_string())
        );

        // Overwrite
        set_setting(&conn, "current_user", "Bob").unwrap();
        assert_eq!(
            get_setting(&conn, "current_user").unwrap(),
            Some("Bob".to_string())
        );
    }

    #[test]
    fn test_audit_log_insert() {
        let conn = open_test_conn("shared_audit_test");

        log_audit(&conn, "projects", 1, "CREATE", "Alice", None, Some("{\"name\":\"Proj\"}")).unwrap();

        let count: Option<i64> = conn
            .select_row("SELECT COUNT(*) FROM audit_log")
            .unwrap()()
            .unwrap();
        assert_eq!(count, Some(1));

        // Also verify the stored column values
        let row = conn
            .select_row_bound::<i64, (String, String, String)>(
                "SELECT table_name, action, changed_by FROM audit_log WHERE record_id = ?1",
            )
            .context("prepare audit select")
            .unwrap()(1)
            .unwrap();
        assert_eq!(
            row,
            Some((
                "projects".to_string(),
                "CREATE".to_string(),
                "Alice".to_string()
            ))
        );
    }

    #[test]
    fn test_projects_table_exists() {
        let conn = open_test_conn("shared_projects_test");

        let now = chrono::Utc::now().to_rfc3339();
        conn.exec_bound::<(&str, &str, &str, &str)>(
            "INSERT INTO projects (name, client_name, status, created_at) \
             VALUES ((?), (?), (?), (?))",
        )
        .unwrap()(("Test Project", "Test Client", "active", now.as_str()))
        .unwrap();

        let count: Option<i64> = conn
            .select_row("SELECT COUNT(*) FROM projects")
            .unwrap()()
            .unwrap();
        assert_eq!(count, Some(1));
    }
}
