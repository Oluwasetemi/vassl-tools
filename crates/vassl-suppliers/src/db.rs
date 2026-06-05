use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::Supplier;
use vassl_db::SharedDomain;
use vassl_db::shared::{current_user, log_audit};

pub struct SupplierDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for SupplierDb {
    const NAME: &'static str = "suppliers";
    const MIGRATIONS: &'static [&'static str] = &[
        "CREATE TABLE IF NOT EXISTS suppliers (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            name           TEXT UNIQUE NOT NULL,
            contact_person TEXT,
            email          TEXT,
            phone          TEXT,
            notes          TEXT,
            created_at     TEXT NOT NULL
        )",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

vassl_db::static_connection!(SupplierDb, [SharedDomain]);

impl SupplierDb {
    pub fn list_suppliers(&self) -> anyhow::Result<Vec<Supplier>> {
        self.select::<(i64, String, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, name, contact_person, email, phone, notes, created_at
             FROM suppliers ORDER BY name",
        )
        .context("prepare list_suppliers")?()
        .context("execute list_suppliers")
        .map(|rows| {
            rows.into_iter().map(|(id, name, contact_person, email, phone, notes, created_at)| {
                Supplier { id, name, contact_person, email, phone, notes, created_at }
            }).collect()
        })
    }

    pub async fn insert_supplier(
        &self,
        name:           &str,
        contact_person: Option<&str>,
        email:          Option<&str>,
        phone:          Option<&str>,
        notes:          Option<&str>,
    ) -> anyhow::Result<i64> {
        let name    = name.to_string();
        let contact = contact_person.map(String::from);
        let email   = email.map(String::from);
        let phone   = phone.map(String::from);
        let notes   = notes.map(String::from);
        let now     = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(String, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
                "INSERT INTO suppliers (name, contact_person, email, phone, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .context("prepare insert_supplier")?
            ((name, contact, email, phone, notes, now))
            .context("execute insert_supplier")?;
            let new_id = conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
                .context("execute last_insert_rowid")?
                .ok_or_else(|| anyhow::anyhow!("no rowid after insert"))?;

            let changed_by = current_user(conn).ok().flatten().unwrap_or_else(|| "system".into());
            if let Err(e) = log_audit(conn, "suppliers", new_id, "CREATE", &changed_by, None, None) {
                tracing::warn!("audit log failed for insert_supplier: {e:?}");
            }

            Ok(new_id)
        })
        .await
    }

    pub async fn update_supplier(
        &self,
        id:             i64,
        name:           &str,
        contact_person: Option<&str>,
        email:          Option<&str>,
        phone:          Option<&str>,
        notes:          Option<&str>,
    ) -> anyhow::Result<()> {
        let name    = name.to_string();
        let contact = contact_person.map(String::from);
        let email   = email.map(String::from);
        let phone   = phone.map(String::from);
        let notes   = notes.map(String::from);

        self.write(move |conn| {
            conn.exec_bound::<(String, Option<String>, Option<String>, Option<String>, Option<String>, i64)>(
                "UPDATE suppliers
                 SET name = ?1, contact_person = ?2, email = ?3, phone = ?4, notes = ?5
                 WHERE id = ?6",
            )
            .context("prepare update_supplier")?
            ((name, contact, email, phone, notes, id))
            .context("execute update_supplier")?;

            let changed_by = current_user(conn).ok().flatten().unwrap_or_else(|| "system".into());
            if let Err(e) = log_audit(conn, "suppliers", id, "UPDATE", &changed_by, None, None) {
                tracing::warn!("audit log failed for update_supplier: {e:?}");
            }

            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_suppliers_empty() {
        let db = SupplierDb::open_test_db("sup_test_empty").await;
        assert!(db.list_suppliers().unwrap().is_empty());
    }

    #[tokio::test]
    async fn insert_and_list_supplier() {
        let db = SupplierDb::open_test_db("sup_test_insert").await;
        let id = db.insert_supplier("Acme Ltd", Some("John"), Some("j@acme.com"), None, None).await.unwrap();
        assert!(id > 0);
        let rows = db.list_suppliers().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Acme Ltd");
        assert_eq!(rows[0].contact_person.as_deref(), Some("John"));
        assert_eq!(rows[0].email.as_deref(), Some("j@acme.com"));
    }

    #[tokio::test]
    async fn duplicate_name_returns_error() {
        let db = SupplierDb::open_test_db("sup_test_dup").await;
        db.insert_supplier("Acme Ltd", None, None, None, None).await.unwrap();
        let result = db.insert_supplier("Acme Ltd", None, None, None, None).await;
        assert!(result.is_err(), "duplicate name should fail");
    }

    #[tokio::test]
    async fn update_supplier_changes_fields() {
        let db = SupplierDb::open_test_db("sup_test_update").await;
        let id = db.insert_supplier("Old Name", None, None, None, None).await.unwrap();
        db.update_supplier(id, "New Name", Some("Alice"), None, None, None).await.unwrap();
        let rows = db.list_suppliers().unwrap();
        assert_eq!(rows[0].name, "New Name");
        assert_eq!(rows[0].contact_person.as_deref(), Some("Alice"));
    }
}
