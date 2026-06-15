use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::PriceEntry;
use vassl_db::shared::{current_user, log_audit};
use vassl_db::SharedDomain;
use vassl_inventory::db::InventoryDb;

pub struct PriceBookDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for PriceBookDb {
    const NAME: &'static str = "pricebook";
    const MIGRATIONS: &'static [&'static str] = &[
        // products is owned by the inventory domain; stub it here so the sqlez FK-cleanup pass
        // can resolve the REFERENCES products(id) constraint on price_book_entries.
        "CREATE TABLE IF NOT EXISTS products (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            sku                   TEXT UNIQUE NOT NULL,
            name                  TEXT NOT NULL,
            category              TEXT,
            unit                  TEXT NOT NULL,
            min_stock_level       REAL NOT NULL DEFAULT 0,
            notes                 TEXT,
            created_at            TEXT NOT NULL,
            description           TEXT,
            preferred_supplier_id INTEGER,
            model_number          TEXT,
            part_number           TEXT,
            duty_percent          REAL NOT NULL DEFAULT 0,
            end_of_life           INTEGER NOT NULL DEFAULT 0,
            replacement           TEXT
        )",
        "CREATE TABLE IF NOT EXISTS price_book_entries (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id        INTEGER NOT NULL REFERENCES products(id),
            cost_price_usd    REAL NOT NULL,
            duty_cost_usd     REAL NOT NULL DEFAULT 0,
            markup_percent    REAL NOT NULL DEFAULT 30,
            selling_price_usd REAL NOT NULL,
            effective_date    TEXT NOT NULL,
            notes             TEXT,
            quantity          REAL NOT NULL DEFAULT 1,
            currency          TEXT NOT NULL DEFAULT 'USD'
        )",
        // stock_entries is owned by inventory; stub here so PriceBookDb::open_test_db
        // has a working schema without needing InventoryDb migrations to run first.
        "CREATE TABLE IF NOT EXISTS stock_entries (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id       INTEGER NOT NULL REFERENCES products(id),
            quantity         REAL NOT NULL,
            unit_cost_usd    REAL NOT NULL,
            supplier         TEXT,
            acquired_at      TEXT NOT NULL,
            acquisition_type TEXT NOT NULL,
            project_id       INTEGER,
            invoice_ref      TEXT,
            notes            TEXT
        )",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool {
        true
    }
}

vassl_db::static_connection!(PriceBookDb, [SharedDomain, InventoryDb]);

impl PriceBookDb {
    pub fn list_entries_for_product(&self, product_id: i64) -> anyhow::Result<Vec<PriceEntry>> {
        self.select_bound::<i64, (
            i64,
            i64,
            f64,
            f64,
            f64,
            f64,
            f64,
            String,
            Option<String>,
            String,
        )>(
            "SELECT id, product_id, quantity, cost_price_usd, duty_cost_usd, markup_percent,
                    selling_price_usd, effective_date, notes, currency
             FROM price_book_entries WHERE product_id = ?1
             ORDER BY effective_date DESC",
        )
        .context("prepare list_entries_for_product")?(product_id)
        .context("execute list_entries_for_product")
        .map(|rows| {
            rows.into_iter()
                .map(
                    |(id, pid, quantity, cost, duty, markup, selling, date, notes, currency)| {
                        PriceEntry {
                            id,
                            product_id: pid,
                            quantity,
                            cost_price_usd: cost,
                            duty_cost_usd: duty,
                            markup_percent: markup,
                            selling_price_usd: selling,
                            effective_date: date,
                            notes,
                            currency,
                        }
                    },
                )
                .collect()
        })
    }

    pub fn list_products_with_latest_price(
        &self,
    ) -> anyhow::Result<Vec<(i64, String, String, Option<PriceEntry>)>> {
        type Row = (
            i64,
            String,
            String,
            Option<i64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        self.select::<Row>(
            "SELECT p.id, p.sku, p.name,
                    e.id, e.quantity, e.cost_price_usd, e.duty_cost_usd, e.markup_percent,
                    e.selling_price_usd, e.effective_date, e.notes, e.currency
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
            rows.into_iter()
                .map(
                    |(
                        pid,
                        sku,
                        name,
                        eid,
                        qty,
                        cost,
                        duty,
                        markup,
                        selling,
                        date,
                        notes,
                        currency,
                    )| {
                        let latest = eid.map(|id| PriceEntry {
                            id,
                            product_id: pid,
                            quantity: qty.unwrap_or(1.0),
                            cost_price_usd: cost.unwrap_or(0.0),
                            duty_cost_usd: duty.unwrap_or(0.0),
                            markup_percent: markup.unwrap_or(30.0),
                            selling_price_usd: selling.unwrap_or(0.0),
                            effective_date: date.unwrap_or_default(),
                            notes,
                            currency: currency.unwrap_or_else(|| "USD".to_string()),
                        });
                        (pid, sku, name, latest)
                    },
                )
                .collect()
        })
    }

    pub async fn delete_price_entry(&self, id: i64) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<i64>("DELETE FROM price_book_entries WHERE id = ?1")
                .context("prepare delete price_book_entries")?(id)
            .context("execute delete price_book_entries")?;
            let changed_by = current_user(conn)
                .ok()
                .flatten()
                .unwrap_or_else(|| "system".into());
            if let Err(e) = log_audit(
                conn,
                "price_book_entries",
                id,
                "DELETE",
                &changed_by,
                None,
                None,
            ) {
                tracing::warn!("audit log failed for delete_price_entry: {e:?}");
            }
            Ok(())
        })
        .await
    }

    pub async fn update_price_entry(
        &self,
        id: i64,
        quantity: f64,
        cost_price_usd: f64,
        duty_cost_usd: f64,
        markup_percent: f64,
        selling_price_usd: f64,
        notes: Option<&str>,
        currency: &str,
    ) -> anyhow::Result<()> {
        let notes = notes.map(String::from);
        let currency = currency.to_string();
        self.write(move |conn| {
            conn.exec_bound::<(f64, f64, f64, f64, f64, Option<String>, String, i64)>(
                "UPDATE price_book_entries
                 SET quantity=?1, cost_price_usd=?2, duty_cost_usd=?3,
                     markup_percent=?4, selling_price_usd=?5, notes=?6, currency=?7
                 WHERE id=?8",
            )
            .context("prepare update_price_entry")?((
                quantity,
                cost_price_usd,
                duty_cost_usd,
                markup_percent,
                selling_price_usd,
                notes,
                currency,
                id,
            ))
            .context("execute update_price_entry")?;
            let changed_by = current_user(conn)
                .ok()
                .flatten()
                .unwrap_or_else(|| "system".into());
            if let Err(e) = log_audit(
                conn,
                "price_book_entries",
                id,
                "UPDATE",
                &changed_by,
                None,
                None,
            ) {
                tracing::warn!("audit log failed for update_price_entry: {e:?}");
            }
            Ok(())
        })
        .await
    }

    pub async fn insert_entry(
        &self,
        product_id: i64,
        quantity: f64,
        cost_price_usd: f64,
        duty_cost_usd: f64,
        markup_percent: f64,
        selling_price_usd: f64,
        notes: Option<&str>,
        currency: &str,
    ) -> anyhow::Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        self.insert_entry_with_date(
            product_id,
            quantity,
            cost_price_usd,
            duty_cost_usd,
            markup_percent,
            selling_price_usd,
            notes,
            &now,
            currency,
        )
        .await
    }

    pub async fn insert_entry_with_date(
        &self,
        product_id: i64,
        quantity: f64,
        cost_price_usd: f64,
        duty_cost_usd: f64,
        markup_percent: f64,
        selling_price_usd: f64,
        notes: Option<&str>,
        effective_date: &str,
        currency: &str,
    ) -> anyhow::Result<i64> {
        let notes = notes.map(String::from);
        let date = effective_date.to_string();
        let currency = currency.to_string();
        self.write(move |conn| {
            conn.exec_bound::<(i64, f64, f64, f64, f64, f64, String, Option<String>, String)>(
                "INSERT INTO price_book_entries
                 (product_id, quantity, cost_price_usd, duty_cost_usd, markup_percent,
                  selling_price_usd, effective_date, notes, currency)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .context("prepare insert_entry_with_date")?((
                product_id,
                quantity,
                cost_price_usd,
                duty_cost_usd,
                markup_percent,
                selling_price_usd,
                date.clone(),
                notes,
                currency,
            ))
            .context("execute insert_entry_with_date")?;

            // Capture price_book_entries rowid before the stock insert changes last_insert_rowid
            let entry_id = conn
                .select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
            .context("execute last_insert_rowid")?
            .context("last_insert_rowid returned None")?;

            // Atomically add a Restock stock entry so inventory reflects the purchase
            conn.exec_bound::<(i64, f64, f64, String)>(
                "INSERT INTO stock_entries
                 (product_id, quantity, unit_cost_usd, acquired_at, acquisition_type)
                 VALUES (?1, ?2, ?3, ?4, 'restock')",
            )
            .context("prepare stock restock")?((
                product_id,
                quantity,
                cost_price_usd,
                date,
            ))
            .context("execute stock restock")?;

            let changed_by = current_user(conn)
                .ok()
                .flatten()
                .unwrap_or_else(|| "system".into());
            if let Err(e) = log_audit(
                conn,
                "price_book_entries",
                entry_id,
                "CREATE",
                &changed_by,
                None,
                None,
            ) {
                tracing::warn!("audit log failed for insert_entry_with_date: {e:?}");
            }

            Ok(entry_id)
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
            .context("prepare insert product")?((sku, name))
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
        let id = db
            .insert_entry(pid, 1.0, 100.0, 10.0, 30.0, 143.0, None, "USD")
            .await
            .unwrap();
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
        db.insert_entry_with_date(
            pid,
            1.0,
            100.0,
            0.0,
            30.0,
            130.0,
            None,
            "2025-01-01T00:00:00Z",
            "USD",
        )
        .await
        .unwrap();
        db.insert_entry_with_date(
            pid,
            1.0,
            200.0,
            0.0,
            30.0,
            260.0,
            None,
            "2026-06-01T00:00:00Z",
            "USD",
        )
        .await
        .unwrap();
        db.insert_entry_with_date(
            pid,
            1.0,
            150.0,
            0.0,
            30.0,
            195.0,
            None,
            "2026-01-01T00:00:00Z",
            "USD",
        )
        .await
        .unwrap();
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
        db.insert_entry_with_date(
            pid,
            1.0,
            300.0,
            0.0,
            30.0,
            390.0,
            None,
            "2025-01-01T00:00:00Z",
            "USD",
        )
        .await
        .unwrap();
        db.insert_entry_with_date(
            pid,
            1.0,
            400.0,
            0.0,
            30.0,
            520.0,
            None,
            "2026-01-01T00:00:00Z",
            "USD",
        )
        .await
        .unwrap();
        let rows = db.list_products_with_latest_price().unwrap();
        assert_eq!(rows.len(), 1);
        let latest = rows[0].3.as_ref().unwrap();
        assert_eq!(
            latest.cost_price_usd, 400.0,
            "should return the most recent entry"
        );
    }
}
