// Re-export DomainMigration from the vendored `db` crate so that module
// crates (vassl-inventory, vassl-quotations, etc.) only need to depend on
// vassl-db — not on `db` directly.
pub use db::DomainMigration;

// Re-export AppMigrator (runs all inventory-registered DomainMigrations in
// topological dependency order) so callers can use it with `open_db`.
pub use db::AppMigrator;

// NOTE: The topological sort in `db::topological_sort` does NOT detect
// circular dependencies — it will infinite-loop if two DomainMigrations
// form a cycle.  Because that function is private to the vendored crate we
// cannot patch it here.  Mitigation: keep migration dependency graphs
// acyclic; a future task can add a two-color DFS cycle check by forking the
// vendored crate.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_migration_re_export_accessible() {
        // Verifies DomainMigration is accessible from vassl-db
        let _: fn() = || {
            let _ = std::mem::size_of::<DomainMigration>();
        };
    }

    #[test]
    fn app_migrator_re_export_accessible() {
        // Verifies AppMigrator is accessible from vassl-db
        let _ = std::mem::size_of::<AppMigrator>();
    }
}
