use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::{AcquisitionType, Product, StockEntry};
use vassl_db::SharedDomain;

pub struct InventoryDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for InventoryDb {
    const NAME: &'static str = "inventory";
    const MIGRATIONS: &'static [&'static str] = &[
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
        "ALTER TABLE products ADD COLUMN description TEXT",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

vassl_db::static_connection!(InventoryDb, [SharedDomain]);

impl InventoryDb {
    /// All products ordered by name.
    pub fn list_products(&self) -> anyhow::Result<Vec<Product>> {
        self.select::<(i64, String, String, Option<String>, String, f64, Option<String>, Option<String>, String)>(
            "SELECT id, sku, name, category, unit, min_stock_level, description, notes, created_at
             FROM products ORDER BY name",
        )
        .context("prepare list_products")?()
        .context("execute list_products")
        .map(|rows| {
            rows.into_iter().map(|(id, sku, name, category, unit, min_stock_level, description, notes, created_at)| {
                Product { id, sku, name, category, unit, min_stock_level, description, notes, created_at }
            }).collect()
        })
    }

    /// Sum of all stock quantities for a product.
    pub fn current_stock(&self, product_id: i64) -> anyhow::Result<f64> {
        self.select_row_bound::<i64, Option<f64>>(
            "SELECT SUM(quantity) FROM stock_entries WHERE product_id = ?1",
        )
        .context("prepare current_stock")?
        (product_id)
        .context("execute current_stock")
        .map(|r| r.flatten().unwrap_or(0.0))
    }

    /// All stock entries for a product, newest first.
    pub fn list_stock_entries(&self, product_id: i64) -> anyhow::Result<Vec<StockEntry>> {
        self.select_bound::<i64, (i64, i64, f64, f64, Option<String>, String, String, Option<i64>, Option<String>, Option<String>)>(
            "SELECT id, product_id, quantity, unit_cost_usd, supplier, acquired_at,
                    acquisition_type, project_id, invoice_ref, notes
             FROM stock_entries WHERE product_id = ?1 ORDER BY acquired_at DESC",
        )
        .context("prepare list_stock_entries")?
        (product_id)
        .context("execute list_stock_entries")
        .map(|rows| {
            rows.into_iter()
                .map(|(id, product_id, quantity, unit_cost_usd, supplier,
                        acquired_at, acquisition_type_str, project_id, invoice_ref, notes)| {
                    let acquisition_type = match acquisition_type_str.as_str() {
                        "restock" => AcquisitionType::Restock,
                        "project" => AcquisitionType::Project,
                        other => return Err(anyhow::anyhow!(
                            "unknown acquisition_type in DB: {other:?}"
                        )),
                    };
                    Ok(StockEntry { id, product_id, quantity, unit_cost_usd, supplier,
                                    acquired_at, acquisition_type, project_id, invoice_ref, notes })
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .and_then(|r| r)
    }

    /// All products with current stock level.
    pub fn list_products_with_stock(&self) -> anyhow::Result<Vec<(Product, f64)>> {
        self.select::<(i64, String, String, Option<String>, String, f64, Option<String>, Option<String>, String, f64)>(
            "SELECT p.id, p.sku, p.name, p.category, p.unit, p.min_stock_level,
                    p.description, p.notes, p.created_at,
                    COALESCE(SUM(s.quantity), 0.0) AS current_stock
             FROM products p
             LEFT JOIN stock_entries s ON s.product_id = p.id
             GROUP BY p.id
             ORDER BY p.name",
        )
        .context("prepare list_products_with_stock")?()
        .context("execute list_products_with_stock")
        .map(|rows| {
            rows.into_iter().map(|(id, sku, name, category, unit, min_stock_level, description, notes, created_at, current_stock)| {
                (Product { id, sku, name, category, unit, min_stock_level, description, notes, created_at }, current_stock)
            }).collect()
        })
    }

    /// Products at or below their min_stock_level.
    pub fn products_below_min_stock(&self) -> anyhow::Result<Vec<Product>> {
        self.select::<(i64, String, String, Option<String>, String, f64, Option<String>, Option<String>, String)>(
            "SELECT p.id, p.sku, p.name, p.category, p.unit, p.min_stock_level,
                    p.description, p.notes, p.created_at
             FROM products p
             LEFT JOIN stock_entries s ON s.product_id = p.id
             WHERE p.min_stock_level > 0
             GROUP BY p.id
             HAVING COALESCE(SUM(s.quantity), 0) <= p.min_stock_level
             ORDER BY p.name",
        )
        .context("prepare products_below_min_stock")?()
        .context("execute products_below_min_stock")
        .map(|rows| {
            rows.into_iter().map(|(id, sku, name, category, unit, min_stock_level, description, notes, created_at)| {
                Product { id, sku, name, category, unit, min_stock_level, description, notes, created_at }
            }).collect()
        })
    }

    /// Insert a new product. Returns the new product id.
    pub async fn insert_product(
        &self,
        sku: &str,
        name: &str,
        category: Option<&str>,
        unit: &str,
        min_stock_level: f64,
        description: Option<&str>,
        notes: Option<&str>,
    ) -> anyhow::Result<i64> {
        let sku         = sku.to_string();
        let name        = name.to_string();
        let category    = category.map(String::from);
        let unit        = unit.to_string();
        let description = description.map(String::from);
        let notes       = notes.map(String::from);
        let now         = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(String, String, Option<String>, String, f64, Option<String>, Option<String>, String)>(
                "INSERT INTO products (sku, name, category, unit, min_stock_level, description, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .context("prepare insert_product")?
            ((sku, name, category, unit, min_stock_level, description, notes, now))
            .context("execute insert_product")?;

            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
                .context("execute last_insert_rowid")?
                .context("last_insert_rowid returned None")
        })
        .await
    }

    /// Insert a new stock entry.
    pub async fn insert_stock_entry(
        &self,
        product_id: i64,
        quantity: f64,
        unit_cost_usd: f64,
        supplier: Option<&str>,
        acquisition_type: AcquisitionType,
        project_id: Option<i64>,
        invoice_ref: Option<&str>,
        notes: Option<&str>,
    ) -> anyhow::Result<()> {
        let supplier    = supplier.map(String::from);
        let acq         = match acquisition_type {
            AcquisitionType::Restock => "restock",
            AcquisitionType::Project => "project",
        }.to_string();
        let invoice_ref = invoice_ref.map(String::from);
        let notes       = notes.map(String::from);
        let now         = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(i64, f64, f64, Option<String>, String, String, Option<i64>, Option<String>, Option<String>)>(
                "INSERT INTO stock_entries
                 (product_id, quantity, unit_cost_usd, supplier, acquired_at,
                  acquisition_type, project_id, invoice_ref, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .context("prepare insert_stock_entry")?
            ((product_id, quantity, unit_cost_usd, supplier, now,
              acq, project_id, invoice_ref, notes))
            .context("execute insert_stock_entry")
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_products_empty() {
        let db = InventoryDb::open_test_db("inv_test_list_empty").await;
        let products = db.list_products().unwrap();
        assert!(products.is_empty());
    }

    #[tokio::test]
    async fn insert_and_list_product() {
        let db = InventoryDb::open_test_db("inv_test_insert_list").await;
        let id = db.insert_product("CAM-001", "IP Camera", Some("CCTV"), "pcs", 5.0, None, None).await.unwrap();
        assert!(id > 0);
        let products = db.list_products().unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].sku, "CAM-001");
        assert_eq!(products[0].name, "IP Camera");
    }

    #[tokio::test]
    async fn current_stock_zero_when_no_entries() {
        let db = InventoryDb::open_test_db("inv_test_stock_zero").await;
        let id = db.insert_product("NVR-001", "NVR", None, "pcs", 2.0, None, None).await.unwrap();
        assert_eq!(db.current_stock(id).unwrap(), 0.0);
    }

    #[tokio::test]
    async fn insert_stock_entry_updates_current_stock() {
        let db = InventoryDb::open_test_db("inv_test_stock_update").await;
        let id = db.insert_product("CAB-001", "Cable", None, "meters", 100.0, None, None).await.unwrap();
        db.insert_stock_entry(id, 50.0, 2.5, Some("SupplierA"), AcquisitionType::Restock, None, None, None).await.unwrap();
        db.insert_stock_entry(id, 30.0, 2.8, None, AcquisitionType::Project, None, None, None).await.unwrap();
        assert_eq!(db.current_stock(id).unwrap(), 80.0);
    }

    #[tokio::test]
    async fn products_below_min_stock_detected() {
        let db = InventoryDb::open_test_db("inv_test_below_min").await;
        let id = db.insert_product("DVR-001", "DVR", None, "pcs", 5.0, None, None).await.unwrap();
        db.insert_stock_entry(id, 3.0, 150.0, None, AcquisitionType::Restock, None, None, None).await.unwrap();
        let below = db.products_below_min_stock().unwrap();
        assert_eq!(below.len(), 1);
        assert_eq!(below[0].sku, "DVR-001");
    }

    #[tokio::test]
    async fn products_at_zero_min_not_alerted() {
        let db = InventoryDb::open_test_db("inv_test_zero_min_ok").await;
        db.insert_product("MISC-001", "Misc", None, "pcs", 0.0, None, None).await.unwrap();
        let below = db.products_below_min_stock().unwrap();
        assert!(below.is_empty());
    }

    #[tokio::test]
    async fn list_products_with_stock_aggregates_correctly() {
        let db = InventoryDb::open_test_db("inv_test_list_with_stock_xyz").await;
        let id = db.insert_product("PTZ-001", "PTZ Camera", None, "pcs", 2.0, None, None).await.unwrap();
        db.insert_stock_entry(id, 5.0, 100.0, None, AcquisitionType::Restock, None, None, None).await.unwrap();
        db.insert_stock_entry(id, 3.0, 95.0, None, AcquisitionType::Restock, None, None, None).await.unwrap();
        let results = db.list_products_with_stock().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 8.0);
    }

    #[tokio::test]
    async fn description_round_trips_through_insert_and_list() {
        let db = InventoryDb::open_test_db("inv_test_desc_roundtrip").await;
        let id = db.insert_product(
            "CAM-001", "IP Camera", Some("CCTV"), "pcs", 5.0,
            Some("Wide-angle lens, 24mm"), None,
        ).await.unwrap();
        assert!(id > 0);
        let products = db.list_products().unwrap();
        assert_eq!(products[0].description, Some("Wide-angle lens, 24mm".to_string()));
    }

    #[tokio::test]
    async fn description_none_does_not_break_insert() {
        let db = InventoryDb::open_test_db("inv_test_desc_none").await;
        let id = db.insert_product("NVR-001", "NVR", None, "pcs", 2.0, None, None).await.unwrap();
        assert!(id > 0);
        let products = db.list_products().unwrap();
        assert_eq!(products[0].description, None);
    }
}
