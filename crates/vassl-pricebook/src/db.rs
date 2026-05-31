use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::PriceEntry;
use vassl_db::SharedDomain;
use vassl_inventory::db::InventoryDb;

pub struct PriceBookDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for PriceBookDb {
    const NAME: &'static str = "pricebook";
    const MIGRATIONS: &'static [&'static str] = &[
        // products is owned by the inventory domain; we create it here so that
        // the sqlez FK-cleanup pass (delete_rows_with_orphaned_foreign_key_references)
        // can resolve the REFERENCES products(id) constraint on price_book_entries.
        "CREATE TABLE IF NOT EXISTS products (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            sku             TEXT UNIQUE NOT NULL,
            name            TEXT NOT NULL,
            category        TEXT,
            unit            TEXT NOT NULL,
            min_stock_level REAL NOT NULL DEFAULT 0,
            notes           TEXT,
            created_at      TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS price_book_entries (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id        INTEGER NOT NULL REFERENCES products(id),
            cost_price_usd    REAL NOT NULL,
            duty_cost_usd     REAL NOT NULL DEFAULT 0,
            markup_percent    REAL NOT NULL DEFAULT 30,
            selling_price_usd REAL NOT NULL,
            effective_date    TEXT NOT NULL,
            notes             TEXT
        )",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

vassl_db::static_connection!(PriceBookDb, [SharedDomain, InventoryDb]);

impl PriceBookDb {
    pub fn list_entries_for_product(&self, product_id: i64) -> anyhow::Result<Vec<PriceEntry>> {
        self.select_bound::<i64, (i64, i64, f64, f64, f64, f64, String, Option<String>)>(
            "SELECT id, product_id, cost_price_usd, duty_cost_usd, markup_percent,
                    selling_price_usd, effective_date, notes
             FROM price_book_entries WHERE product_id = ?1
             ORDER BY effective_date DESC",
        )
        .context("prepare list_entries_for_product")?
        (product_id)
        .context("execute list_entries_for_product")
        .map(|rows| {
            rows.into_iter().map(|(id, pid, cost, duty, markup, selling, date, notes)| {
                PriceEntry {
                    id, product_id: pid,
                    cost_price_usd: cost, duty_cost_usd: duty,
                    markup_percent: markup, selling_price_usd: selling,
                    effective_date: date, notes,
                }
            }).collect()
        })
    }

    pub fn list_products_with_latest_price(
        &self,
    ) -> anyhow::Result<Vec<(i64, String, String, Option<PriceEntry>)>> {
        type Row = (
            i64, String, String,
            Option<i64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>,
            Option<String>, Option<String>,
        );
        self.select::<Row>(
            "SELECT p.id, p.sku, p.name,
                    e.id, e.cost_price_usd, e.duty_cost_usd, e.markup_percent,
                    e.selling_price_usd, e.effective_date, e.notes
             FROM products p
             LEFT JOIN price_book_entries e ON e.id = (
                 SELECT id FROM price_book_entries
                 WHERE product_id = p.id
                 ORDER BY effective_date DESC LIMIT 1
             )
             ORDER BY p.name",
        )
        .context("prepare list_products_with_latest_price")?()
        .context("execute list_products_with_latest_price")
        .map(|rows| {
            rows.into_iter().map(|(pid, sku, name, eid, cost, duty, markup, selling, date, notes)| {
                let latest = eid.map(|id| PriceEntry {
                    id, product_id: pid,
                    cost_price_usd:    cost.unwrap_or(0.0),
                    duty_cost_usd:     duty.unwrap_or(0.0),
                    markup_percent:    markup.unwrap_or(30.0),
                    selling_price_usd: selling.unwrap_or(0.0),
                    effective_date:    date.unwrap_or_default(),
                    notes,
                });
                (pid, sku, name, latest)
            }).collect()
        })
    }

    pub async fn insert_entry(
        &self,
        product_id:        i64,
        cost_price_usd:    f64,
        duty_cost_usd:     f64,
        markup_percent:    f64,
        selling_price_usd: f64,
        notes:             Option<&str>,
    ) -> anyhow::Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        self.insert_entry_with_date(
            product_id, cost_price_usd, duty_cost_usd, markup_percent,
            selling_price_usd, notes, &now,
        ).await
    }

    pub async fn insert_entry_with_date(
        &self,
        product_id:        i64,
        cost_price_usd:    f64,
        duty_cost_usd:     f64,
        markup_percent:    f64,
        selling_price_usd: f64,
        notes:             Option<&str>,
        effective_date:    &str,
    ) -> anyhow::Result<i64> {
        let notes = notes.map(String::from);
        let date  = effective_date.to_string();
        self.write(move |conn| {
            conn.exec_bound::<(i64, f64, f64, f64, f64, String, Option<String>)>(
                "INSERT INTO price_book_entries
                 (product_id, cost_price_usd, duty_cost_usd, markup_percent,
                  selling_price_usd, effective_date, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .context("prepare insert_entry_with_date")?
            ((product_id, cost_price_usd, duty_cost_usd, markup_percent,
              selling_price_usd, date, notes))
            .context("execute insert_entry_with_date")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
                .context("execute last_insert_rowid")?
                .context("last_insert_rowid returned None")
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_product(db: &PriceBookDb, sku: &str, name: &str) -> i64 {
        let sku = sku.to_string();
        let name = name.to_string();
        db.write(move |conn| {
            conn.exec_bound::<(String, String)>(
                "INSERT INTO products (sku, name, unit, created_at)
                 VALUES (?1, ?2, 'pcs', datetime('now'))",
            )
            .context("prepare insert product")?
            ((sku, name))
            .context("exec insert product")?;

            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
                .context("exec rowid")?
                .context("rowid was None")
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn list_entries_empty() {
        let db = PriceBookDb::open_test_db("pb_entries_empty").await;
        let entries = db.list_entries_for_product(1).unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn insert_and_retrieve_entry() {
        let db = PriceBookDb::open_test_db("pb_insert_retrieve").await;
        let pid = setup_product(&db, "CAM-001", "IP Camera").await;
        let id = db.insert_entry(pid, 100.0, 10.0, 30.0, 143.0, None).await.unwrap();
        assert!(id > 0);
        let entries = db.list_entries_for_product(pid).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cost_price_usd, 100.0);
        assert_eq!(entries[0].selling_price_usd, 143.0);
    }

    #[tokio::test]
    async fn entries_returned_newest_first() {
        let db = PriceBookDb::open_test_db("pb_entries_order").await;
        let pid = setup_product(&db, "CAM-002", "PTZ Camera").await;
        db.insert_entry_with_date(pid, 100.0, 0.0, 30.0, 130.0, None, "2025-01-01T00:00:00Z")
            .await.unwrap();
        db.insert_entry_with_date(pid, 200.0, 0.0, 30.0, 260.0, None, "2026-06-01T00:00:00Z")
            .await.unwrap();
        db.insert_entry_with_date(pid, 150.0, 0.0, 30.0, 195.0, None, "2026-01-01T00:00:00Z")
            .await.unwrap();
        let entries = db.list_entries_for_product(pid).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].effective_date >= entries[1].effective_date);
        assert!(entries[1].effective_date >= entries[2].effective_date);
    }

    #[tokio::test]
    async fn list_products_with_latest_price_returns_all_products() {
        let db = PriceBookDb::open_test_db("pb_latest_price").await;
        let pid = setup_product(&db, "CAM-001", "IP Camera").await;
        let rows = db.list_products_with_latest_price().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, pid);
        assert!(rows[0].3.is_none(), "no price entry yet");
    }

    #[tokio::test]
    async fn list_products_with_latest_price_shows_most_recent_entry() {
        let db = PriceBookDb::open_test_db("pb_latest_price_entry").await;
        let pid = setup_product(&db, "NVR-001", "NVR").await;
        db.insert_entry_with_date(pid, 300.0, 0.0, 30.0, 390.0, None, "2025-01-01T00:00:00Z")
            .await.unwrap();
        db.insert_entry_with_date(pid, 400.0, 0.0, 30.0, 520.0, None, "2026-01-01T00:00:00Z")
            .await.unwrap();
        let rows = db.list_products_with_latest_price().unwrap();
        assert_eq!(rows.len(), 1);
        let latest = rows[0].3.as_ref().unwrap();
        assert_eq!(latest.cost_price_usd, 400.0, "should return the most recent entry");
    }
}
