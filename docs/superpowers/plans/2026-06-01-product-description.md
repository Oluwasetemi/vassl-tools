# Product Description Field Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional `description` text field to products — stored in the DB, exposed in the product creation form, not shown elsewhere.

**Architecture:** One new SQLite migration adds the column. `Product` and `NewProduct` in `vassl-core` gain the field. Three DB read queries and one write query in `vassl-inventory/src/db.rs` are updated. `ProductForm` gains a `description` TextInput as the last tab-order field, passed through to `insert_product` on save.

**Tech Stack:** GPUI entity/render pattern, sqlez for DB queries, vassl-core domain types.

---

## File Map

| File | Change |
|---|---|
| `crates/vassl-core/src/product.rs` | Add `description: Option<String>` to `Product` and `NewProduct` |
| `crates/vassl-inventory/src/db.rs` | Add migration, update 3 SELECT queries + `insert_product` signature, update 6 test call sites |
| `crates/vassl-inventory/src/product_form.rs` | Add `description` field, render row, Tab/BackTab handles, pass to `insert_product` |

---

### Task 1: Add `description` to `Product` and `NewProduct` in vassl-core

**Files:**
- Modify: `crates/vassl-core/src/product.rs`

- [ ] **Step 1: Write a failing test**

Add to the bottom of `crates/vassl-core/src/product.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_description_is_optional() {
        let p = Product {
            id: 1,
            sku: "CAM-001".into(),
            name: "IP Camera".into(),
            category: None,
            unit: "pcs".into(),
            min_stock_level: 0.0,
            description: Some("Wide-angle, 24mm".into()),
            notes: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(p.description.as_deref(), Some("Wide-angle, 24mm"));
    }

    #[test]
    fn new_product_description_is_optional() {
        let np = NewProduct {
            sku: "CAM-001".into(),
            name: "IP Camera".into(),
            category: None,
            unit: "pcs".into(),
            min_stock_level: 0.0,
            description: None,
            notes: None,
        };
        assert!(np.description.is_none());
    }
}
```

- [ ] **Step 2: Run test — expect compile error**

```bash
cargo test -p vassl-core 2>&1 | head -10
```

Expected: `error[E0560]: struct Product has no field named description`

- [ ] **Step 3: Add `description` field to `Product` and `NewProduct`**

Replace the entire `crates/vassl-core/src/product.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProduct {
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub description: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockEntry {
    pub id: i64,
    pub product_id: i64,
    pub quantity: f64,
    pub unit_cost_usd: f64,
    pub supplier: Option<String>,
    pub acquired_at: String,
    pub acquisition_type: AcquisitionType,
    pub project_id: Option<i64>,
    pub invoice_ref: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionType {
    Project,
    Restock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewStockEntry {
    pub product_id: i64,
    pub quantity: f64,
    pub unit_cost_usd: f64,
    pub supplier: Option<String>,
    pub acquired_at: String,
    pub acquisition_type: AcquisitionType,
    pub project_id: Option<i64>,
    pub invoice_ref: Option<String>,
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_description_is_optional() {
        let p = Product {
            id: 1,
            sku: "CAM-001".into(),
            name: "IP Camera".into(),
            category: None,
            unit: "pcs".into(),
            min_stock_level: 0.0,
            description: Some("Wide-angle, 24mm".into()),
            notes: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(p.description.as_deref(), Some("Wide-angle, 24mm"));
    }

    #[test]
    fn new_product_description_is_optional() {
        let np = NewProduct {
            sku: "CAM-001".into(),
            name: "IP Camera".into(),
            category: None,
            unit: "pcs".into(),
            min_stock_level: 0.0,
            description: None,
            notes: None,
        };
        assert!(np.description.is_none());
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vassl-core 2>&1 | tail -5
```

Expected: `test result: ok. 2 passed; 0 failed`

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-core/src/product.rs
git commit -m "feat(core): add description field to Product and NewProduct"
```

---

### Task 2: DB migration + update all queries in vassl-inventory

**Files:**
- Modify: `crates/vassl-inventory/src/db.rs`

This task updates the migration, all three SELECT queries, `insert_product`, and all 6 existing test call sites.

- [ ] **Step 1: Write 2 failing tests**

Add to the `tests` module at the bottom of `crates/vassl-inventory/src/db.rs`:

```rust
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
```

- [ ] **Step 2: Run — expect compile errors**

```bash
cargo test -p vassl-inventory 2>&1 | head -15
```

Expected: errors about `insert_product` argument count and `description` field.

- [ ] **Step 3: Replace the entire `crates/vassl-inventory/src/db.rs`**

Replace the file with this complete version:

```rust
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
```

- [ ] **Step 4: Build and run tests**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
cargo test -p vassl-inventory 2>&1 | tail -5
```

Expected: `Finished` and `test result: ok. 9 passed; 0 failed`

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-inventory/src/db.rs
git commit -m "feat(inventory): add description column migration, update all product queries"
```

---

### Task 3: Add description field to ProductForm

**Files:**
- Modify: `crates/vassl-inventory/src/product_form.rs`

- [ ] **Step 1: Run existing tests to confirm they fail due to `insert_product` arity change**

```bash
cargo build -p vassl-inventory 2>&1 | grep "^error"
```

Expected: `error[E0061]: this function takes 7 arguments but 6 were supplied` in `product_form.rs`

- [ ] **Step 2: Replace `crates/vassl-inventory/src/product_form.rs` with the updated version**

```rust
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

actions!(product_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum ProductFormEvent { Submitted, Cancelled }

impl EventEmitter<ProductFormEvent> for ProductForm {}

pub struct ProductForm {
    store:        Entity<InventoryStore>,
    pub sku:      Entity<TextInput>,
    name:         Entity<TextInput>,
    category:     Entity<TextInput>,
    unit:         Entity<TextInput>,
    min_stock:    Entity<TextInput>,
    description:  Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

fn validate_product(sku: &str, name: &str, unit: &str, min_stock: &str) -> Result<(String, String, String, f64), String> {
    let sku = sku.trim().to_string();
    if sku.is_empty()  { return Err("SKU is required.".to_string()); }
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Name is required.".to_string()); }
    let unit = unit.trim().to_string();
    if unit.is_empty() { return Err("Unit is required (e.g. 'pcs', 'meters').".to_string()); }
    let min: f64 = min_stock.trim().parse().unwrap_or(0.0);
    if min < 0.0  { return Err("Min stock must be ≥ 0.".to_string()); }
    Ok((sku, name, unit, min))
}

impl ProductForm {
    pub fn new(store: Entity<InventoryStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            sku:          cx.new(|cx| TextInput::with_placeholder("e.g. CAM-IP-2MP", cx)),
            name:         cx.new(|cx| TextInput::with_placeholder("e.g. IP Camera 2MP", cx)),
            category:     cx.new(|cx| TextInput::with_placeholder("optional: Cameras, Cabling…", cx)),
            unit:         cx.new(|cx| TextInput::with_placeholder("pcs, meters, rolls…", cx)),
            min_stock:    cx.new(|cx| TextInput::with_placeholder("0", cx)),
            description:  cx.new(|cx| TextInput::with_placeholder(
                "e.g. Wide-angle camera lens, 24mm, F/1.8, compatible with Sony E-mount", cx
            )),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let sku       = self.sku.read(cx).text().to_string();
        let name      = self.name.read(cx).text().to_string();
        let unit      = self.unit.read(cx).text().to_string();
        let min_s     = self.min_stock.read(cx).text().to_string();
        let category  = self.category.read(cx).text().trim().to_string();
        let cat_opt   = if category.is_empty() { None } else { Some(category) };
        let desc      = self.description.read(cx).text().trim().to_string();
        let desc_opt  = if desc.is_empty() { None } else { Some(desc) };

        match validate_product(&sku, &name, &unit, &min_s) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((sku, name, unit, min)) => {
                let db    = InventoryDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_product(
                        &sku, &name, cat_opt.as_deref(), &unit, min,
                        desc_opt.as_deref(), None,
                    ).await;
                    if let Err(e) = result { tracing::error!("insert_product failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(ProductFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for ProductForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for ProductForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c      = cx.global::<ThemeHandle>().0.clone();
        let sku_f  = self.sku.read(cx).focus_handle.is_focused(window);
        let name_f = self.name.read(cx).focus_handle.is_focused(window);
        let cat_f  = self.category.read(cx).focus_handle.is_focused(window);
        let unit_f = self.unit.read(cx).focus_handle.is_focused(window);
        let min_f  = self.min_stock.read(cx).focus_handle.is_focused(window);
        let desc_f = self.description.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("ProductForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(ProductFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.sku.read(cx).focus_handle.clone(),
                    this.name.read(cx).focus_handle.clone(),
                    this.category.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.min_stock.read(cx).focus_handle.clone(),
                    this.description.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.sku.read(cx).focus_handle.clone(),
                    this.name.read(cx).focus_handle.clone(),
                    this.category.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.min_stock.read(cx).focus_handle.clone(),
                    this.description.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(580.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1()
                                .text_size(px(13.)).text_color(rgb(c.text_default))
                                .child("New Product"))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("SKU"))
                                    .child(div().flex_1().child(text_field("", self.sku.clone(), sku_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Name"))
                                    .child(div().flex_1().child(text_field("", self.name.clone(), name_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Category"))
                                    .child(div().flex_1().child(text_field("", self.category.clone(), cat_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Unit"))
                                    .child(div().flex_1().child(text_field("", self.unit.clone(), unit_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Min Stock Level"))
                                    .child(div().flex_1().child(text_field("", self.min_stock.clone(), min_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_start().py(px(10.))
                                    .child(div().w(px(160.)).pt(px(6.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Description"))
                                    .child(div().flex_1().h(px(64.)).child(text_field("", self.description.clone(), desc_f, cx)))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(px(11.)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    // ── footer ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("prod-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(ProductFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("prod-btn-save").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save Product"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_product;
    #[test] fn rejects_empty_sku()  { assert!(validate_product("", "Camera", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_name() { assert!(validate_product("CAM-001", "", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_unit() { assert!(validate_product("CAM-001", "Camera", "", "5").is_err()); }
    #[test] fn accepts_zero_min()   { assert!(validate_product("CAM-001", "Camera", "pcs", "0").is_ok()); }
    #[test] fn accepts_valid()      { assert!(validate_product("CAM-001", "IP Camera", "pcs", "5.0").is_ok()); }
}
```

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

- [ ] **Step 4: Run all tests**

```bash
cargo test 2>&1 | tail -8
```

Expected: all tests pass across all crates.

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-inventory/src/product_form.rs
git commit -m "feat(inventory): add description field to product form, Tab/BackTab order updated"
```

---

## Self-Review Checklist

**Spec coverage:**
- ✅ `ALTER TABLE products ADD COLUMN description TEXT` migration — Task 2 MIGRATIONS array
- ✅ `description: Option<String>` in `Product` struct — Task 1
- ✅ `description: Option<String>` in `NewProduct` struct — Task 1
- ✅ `insert_product` updated with `description` param — Task 2
- ✅ All 3 SELECT queries updated — Task 2 (`list_products`, `list_products_with_stock`, `products_below_min_stock`)
- ✅ `ProductForm` gains `description: Entity<TextInput>` — Task 3
- ✅ Description row rendered below Min Stock Level — Task 3
- ✅ Tab/BackTab order extended to include description — Task 3
- ✅ `submit()` passes description to `insert_product` — Task 3
- ✅ Round-trip test: insert with description → list → assert value — Task 2
- ✅ Backward-compat test: `description = None` — Task 2

**Type consistency:** `insert_product` takes `description: Option<&str>` in Task 2 and is called with `desc_opt.as_deref()` in Task 3. ✅
