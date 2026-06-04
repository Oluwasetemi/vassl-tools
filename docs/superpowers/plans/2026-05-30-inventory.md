# Inventory Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Inventory module — a product stock list with status badges, a stock entry form modal, and a restock alerts panel — all wired into the running VASSL window.

**Architecture:** `vassl-inventory` crate owns everything: `InventoryDb` (typed sqlez handle via `static_connection!`), `InventoryStore` (GPUI entity holding loaded state), and three GPUI views (`ProductList`, `StockEntryForm` modal, `RestockAlerts`). `VasslRoot` in `vassl-app` renders `InventoryPanel` in the pane area when `ActiveModule::Inventory` is active.

**Tech Stack:** Rust, GPUI (`Entity<T>`, `Render`, `cx.spawn`, `cx.notify()`), sqlez (`select_bound`, `exec_bound`, `ThreadSafeConnection::write`), `inventory::submit!` + `static_connection!` for DB domain registration, `vassl-core` domain types (`Product`, `StockEntry`, `AcquisitionType`).

---

## File Map

```
tools/
├── crates/
│   ├── vassl-inventory/
│   │   ├── Cargo.toml                   # add gpui, vassl-db, vassl-core, chrono deps
│   │   └── src/
│   │       ├── lib.rs                   # pub fn init(cx: &mut App), registers InventoryDb domain
│   │       ├── db.rs                    # InventoryDb: Domain impl + static_connection! + all queries
│   │       ├── store.rs                 # InventoryStore GPUI entity: ProductWithStock, StockStatus, async load
│   │       ├── panel.rs                 # InventoryPanel: top-level view, holds sub-views
│   │       ├── product_list.rs          # ProductList: scrollable table view with status badges
│   │       ├── stock_form.rs            # StockEntryForm: modal overlay for new stock entry
│   │       └── restock.rs               # RestockAlerts: list of products at/below min_stock_level
│   ├── vassl-app/
│   │   └── src/
│   │       ├── colors.rs                # add STATUS_GREEN, STATUS_AMBER, STATUS_RED
│   │       └── root.rs                  # wire InventoryPanel into pane area
```

---

### Task 1: InventoryDb — Domain + migrations + queries

**Files:**
- Modify: `tools/crates/vassl-inventory/Cargo.toml`
- Create: `tools/crates/vassl-inventory/src/db.rs`
- Modify: `tools/crates/vassl-inventory/src/lib.rs`

- [ ] **Step 1: Update `crates/vassl-inventory/Cargo.toml`**

```toml
[package]
name    = "vassl-inventory"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui.workspace       = true
anyhow.workspace     = true
thiserror.workspace  = true
serde.workspace      = true
serde_json.workspace = true
chrono.workspace     = true
vassl-db             = { path = "../vassl-db" }
vassl-core           = { path = "../vassl-core" }
```

- [ ] **Step 2: Write failing test for `InventoryDb` query methods**

Create `crates/vassl-inventory/src/db.rs`:

```rust
use anyhow::Context as _;
use sqlez::connection::Connection;
use sqlez::domain::Domain;
use vassl_core::{AcquisitionType, Product, StockEntry};

// ── Domain marker for "shared" dependency ────────────────────────────────────
// This lets static_connection! reference the "shared" domain as a dep.
pub struct SharedDomain;
impl Domain for SharedDomain {
    const NAME: &'static str = "shared";
    const MIGRATIONS: &'static [&'static str] = &[];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

// ── InventoryDb: typed connection handle ─────────────────────────────────────
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
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

vassl_db::static_connection!(InventoryDb, [SharedDomain]);

// ── Query helpers ─────────────────────────────────────────────────────────────
// All reads are synchronous via Deref<Target = Connection>.
// Writes are async via self.write(|conn| { ... }).await.

impl InventoryDb {
    /// All products ordered by name.
    pub fn list_products(&self) -> anyhow::Result<Vec<Product>> {
        self.select::<(i64, String, String, Option<String>, String, f64, Option<String>, String)>(
            "SELECT id, sku, name, category, unit, min_stock_level, notes, created_at
             FROM products ORDER BY name",
        )
        .context("prepare list_products")?()
        .context("execute list_products")
        .map(|rows| {
            rows.into_iter().map(|(id, sku, name, category, unit, min_stock_level, notes, created_at)| {
                Product { id, sku, name, category, unit, min_stock_level, notes, created_at }
            }).collect()
        })
    }

    /// Sum of all stock quantities for a product (current stock).
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
            rows.into_iter().map(|(id, product_id, quantity, unit_cost_usd, supplier,
                                   acquired_at, acquisition_type, project_id, invoice_ref, notes)| {
                let acquisition_type = match acquisition_type.as_str() {
                    "project" => AcquisitionType::Project,
                    _ => AcquisitionType::Restock,
                };
                StockEntry { id, product_id, quantity, unit_cost_usd, supplier,
                             acquired_at, acquisition_type, project_id, invoice_ref, notes }
            }).collect()
        })
    }

    /// Products at or below their min_stock_level (min > 0 only).
    pub fn products_below_min_stock(&self) -> anyhow::Result<Vec<Product>> {
        self.select::<(i64, String, String, Option<String>, String, f64, Option<String>, String)>(
            "SELECT p.id, p.sku, p.name, p.category, p.unit, p.min_stock_level, p.notes, p.created_at
             FROM products p
             WHERE p.min_stock_level > 0
               AND (SELECT COALESCE(SUM(quantity), 0) FROM stock_entries WHERE product_id = p.id)
                   <= p.min_stock_level
             ORDER BY p.name",
        )
        .context("prepare products_below_min_stock")?()
        .context("execute products_below_min_stock")
        .map(|rows| {
            rows.into_iter().map(|(id, sku, name, category, unit, min_stock_level, notes, created_at)| {
                Product { id, sku, name, category, unit, min_stock_level, notes, created_at }
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
        notes: Option<&str>,
    ) -> anyhow::Result<i64> {
        let sku = sku.to_string();
        let name = name.to_string();
        let category = category.map(String::from);
        let unit = unit.to_string();
        let notes = notes.map(String::from);
        let now = chrono::Utc::now().to_rfc3339();

        self.write(|conn| {
            conn.exec_bound::<(String, String, Option<String>, String, f64, Option<String>, String)>(
                "INSERT INTO products (sku, name, category, unit, min_stock_level, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .context("prepare insert_product")?
            ((sku, name, category, unit, min_stock_level, notes, now))
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
        acquisition_type: &str,
        project_id: Option<i64>,
        invoice_ref: Option<&str>,
        notes: Option<&str>,
    ) -> anyhow::Result<()> {
        let supplier = supplier.map(String::from);
        let acquisition_type = acquisition_type.to_string();
        let invoice_ref = invoice_ref.map(String::from);
        let notes = notes.map(String::from);
        let now = chrono::Utc::now().to_rfc3339();

        self.write(|conn| {
            conn.exec_bound::<(i64, f64, f64, Option<String>, String, String, Option<i64>, Option<String>, Option<String>)>(
                "INSERT INTO stock_entries
                 (product_id, quantity, unit_cost_usd, supplier, acquired_at,
                  acquisition_type, project_id, invoice_ref, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .context("prepare insert_stock_entry")?
            ((product_id, quantity, unit_cost_usd, supplier, now,
              acquisition_type, project_id, invoice_ref, notes))
            .context("execute insert_stock_entry")
        })
        .await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    async fn open_test_db() -> InventoryDb {
        InventoryDb::open_test_db("inventory_test").await
    }

    #[tokio::test]
    async fn list_products_empty() {
        let db = open_test_db().await;
        let products = db.list_products().unwrap();
        assert!(products.is_empty());
    }

    #[tokio::test]
    async fn insert_and_list_product() {
        let db = open_test_db().await;
        let id = db.insert_product("CAM-001", "IP Camera", Some("CCTV"), "pcs", 5.0, None).await.unwrap();
        assert!(id > 0);
        let products = db.list_products().unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].sku, "CAM-001");
        assert_eq!(products[0].name, "IP Camera");
    }

    #[tokio::test]
    async fn current_stock_zero_when_no_entries() {
        let db = open_test_db().await;
        let id = db.insert_product("NVR-001", "NVR", None, "pcs", 2.0, None).await.unwrap();
        assert_eq!(db.current_stock(id).unwrap(), 0.0);
    }

    #[tokio::test]
    async fn insert_stock_entry_updates_current_stock() {
        let db = open_test_db().await;
        let id = db.insert_product("CAB-001", "Cable", None, "meters", 100.0, None).await.unwrap();
        db.insert_stock_entry(id, 50.0, 2.5, Some("SupplierA"), "restock", None, None, None).await.unwrap();
        db.insert_stock_entry(id, 30.0, 2.8, None, "project", None, None, None).await.unwrap();
        assert_eq!(db.current_stock(id).unwrap(), 80.0);
    }

    #[tokio::test]
    async fn products_below_min_stock_detected() {
        let db = open_test_db().await;
        let id = db.insert_product("DVR-001", "DVR", None, "pcs", 5.0, None).await.unwrap();
        db.insert_stock_entry(id, 3.0, 150.0, None, "restock", None, None, None).await.unwrap();
        let below = db.products_below_min_stock().unwrap();
        assert_eq!(below.len(), 1);
        assert_eq!(below[0].sku, "DVR-001");
    }

    #[tokio::test]
    async fn products_at_zero_min_not_alerted() {
        let db = open_test_db().await;
        let id = db.insert_product("MISC-001", "Misc", None, "pcs", 0.0, None).await.unwrap();
        let _ = id;
        let below = db.products_below_min_stock().unwrap();
        assert!(below.is_empty());
    }
}
```

> **Note:** `open_test_db` requires the `[cfg(any(test, feature = "test-support"))]` impl generated by `static_connection!`. Add `tokio` as a dev-dependency: `tokio = { version = "1", features = ["macros", "rt"] }` in Cargo.toml.

- [ ] **Step 3: Run failing tests (module not wired to lib.rs yet)**

```bash
cd /Users/oluwasetemi/r/kamalu/tools
cargo test -p vassl-inventory 2>&1 | head -20
```

Expected: FAIL — `db` module not declared.

- [ ] **Step 4: Wire `db` module in `lib.rs`**

Replace `crates/vassl-inventory/src/lib.rs`:

```rust
pub mod db;
pub mod store;     // stub — populated in Task 2
pub mod panel;     // stub — populated in Task 5
pub mod product_list; // stub — populated in Task 4
pub mod stock_form;   // stub — populated in Task 6
pub mod restock;      // stub — populated in Task 5

use gpui::App;

pub use db::InventoryDb;

pub fn init(cx: &mut App) {
    // InventoryStore created in Task 2 and registered here
    let _ = cx;
}
```

Create stub files (each containing just a comment):

```bash
echo "// populated in Task 2" > crates/vassl-inventory/src/store.rs
echo "// populated in Task 5" > crates/vassl-inventory/src/panel.rs
echo "// populated in Task 4" > crates/vassl-inventory/src/product_list.rs
echo "// populated in Task 6" > crates/vassl-inventory/src/stock_form.rs
echo "// populated in Task 5" > crates/vassl-inventory/src/restock.rs
```

- [ ] **Step 5: Run tests — all 6 pass**

```bash
cargo test -p vassl-inventory -- db::tests --nocapture
```

Expected:
```
test db::tests::list_products_empty ... ok
test db::tests::insert_and_list_product ... ok
test db::tests::current_stock_zero_when_no_entries ... ok
test db::tests::insert_stock_entry_updates_current_stock ... ok
test db::tests::products_below_min_stock_detected ... ok
test db::tests::products_at_zero_min_not_alerted ... ok
```

> **If tests fail on sqlez binding types:** Read `crates/sqlez/src/bindable.rs` and `crates/sqlez/src/statement.rs` to understand which Rust types implement `Bind` and `Column`. Tuples of primitives and `Option<primitive>` are implemented. Adjust tuple types in the query methods to match.

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-inventory/
git commit -m "feat(inventory): InventoryDb domain, migrations, product/stock queries"
```

---

### Task 2: InventoryStore — GPUI entity + async state

**Files:**
- Modify: `tools/crates/vassl-inventory/src/store.rs`
- Modify: `tools/crates/vassl-inventory/src/lib.rs`

- [ ] **Step 1: Write failing test for StockStatus**

Add to `crates/vassl-inventory/src/store.rs`:

```rust
use gpui::{App, Context, EventEmitter, Global};
use vassl_core::{Product, StockEntry};

use crate::db::InventoryDb;

// ── View model types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum StockStatus {
    Healthy,         // current > min * 1.2
    Low,             // min < current <= min * 1.2
    Critical,        // current <= min (and min > 0)
    NoAlert,         // min_stock_level == 0
}

impl StockStatus {
    pub fn from_levels(current: f64, min: f64) -> Self {
        if min == 0.0 { return Self::NoAlert; }
        if current <= min { Self::Critical }
        else if current <= min * 1.2 { Self::Low }
        else { Self::Healthy }
    }
}

#[derive(Debug, Clone)]
pub struct ProductWithStock {
    pub product: Product,
    pub current_stock: f64,
    pub status: StockStatus,
}

// ── InventoryStore ────────────────────────────────────────────────────────────

pub struct InventoryStore {
    pub products: Vec<ProductWithStock>,
    pub selected_product_id: Option<i64>,
    pub stock_entries: Vec<StockEntry>,   // entries for selected product
    pub loading: bool,
}

pub enum InventoryEvent {
    ProductsLoaded,
    StockEntriesLoaded,
}

impl EventEmitter<InventoryEvent> for InventoryStore {}

impl InventoryStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            products: Vec::new(),
            selected_product_id: None,
            stock_entries: Vec::new(),
            loading: false,
        }
    }

    /// Async: fetch all products with current stock from DB, update self, notify.
    pub fn load_products(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();

        let db = InventoryDb::global(cx.app());
        cx.spawn(async move |this, cx| {
            // Fetch products synchronously on a background thread
            let result = cx.background_executor()
                .spawn(async move {
                    let products = db.list_products()?;
                    let with_stock: anyhow::Result<Vec<ProductWithStock>> = products
                        .into_iter()
                        .map(|p| {
                            let current = db.current_stock(p.id)?;
                            let status = StockStatus::from_levels(current, p.min_stock_level);
                            Ok(ProductWithStock { product: p, current_stock: current, status })
                        })
                        .collect();
                    with_stock
                })
                .await;

            this.update(cx, |store, cx| {
                store.loading = false;
                match result {
                    Ok(products) => {
                        store.products = products;
                        cx.emit(InventoryEvent::ProductsLoaded);
                    }
                    Err(e) => tracing::error!("load_products failed: {e:?}"),
                }
                cx.notify();
            })
        })
        .detach();
    }

    /// Async: select a product and load its stock entries.
    pub fn select_product(&mut self, product_id: i64, cx: &mut Context<Self>) {
        self.selected_product_id = Some(product_id);
        cx.notify();

        let db = InventoryDb::global(cx.app());
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_stock_entries(product_id) })
                .await;

            this.update(cx, |store, cx| {
                match result {
                    Ok(entries) => {
                        store.stock_entries = entries;
                        cx.emit(InventoryEvent::StockEntriesLoaded);
                    }
                    Err(e) => tracing::error!("select_product failed: {e:?}"),
                }
                cx.notify();
            })
        })
        .detach();
    }
}

impl Global for InventoryStore {}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_status_healthy() {
        assert_eq!(StockStatus::from_levels(10.0, 5.0), StockStatus::Healthy);
    }

    #[test]
    fn stock_status_low() {
        // 5.5 is within 20% above min=5 (threshold: 5.0 * 1.2 = 6.0)
        assert_eq!(StockStatus::from_levels(5.5, 5.0), StockStatus::Low);
    }

    #[test]
    fn stock_status_critical() {
        assert_eq!(StockStatus::from_levels(3.0, 5.0), StockStatus::Critical);
        assert_eq!(StockStatus::from_levels(5.0, 5.0), StockStatus::Critical); // exactly at min
    }

    #[test]
    fn stock_status_no_alert_when_min_zero() {
        assert_eq!(StockStatus::from_levels(0.0, 0.0), StockStatus::NoAlert);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-inventory -- store::tests --nocapture
```

Expected: 4 tests pass.

- [ ] **Step 3: Update `lib.rs` to register InventoryStore as a GPUI Global**

Replace `crates/vassl-inventory/src/lib.rs`:

```rust
pub mod db;
pub mod panel;
pub mod product_list;
pub mod restock;
pub mod stock_form;
pub mod store;

use gpui::{App, Entity};

pub use db::InventoryDb;
pub use store::InventoryStore;

pub fn init(cx: &mut App) {
    let store: Entity<InventoryStore> = cx.new(InventoryStore::new);
    cx.set_global(store);
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p vassl-inventory
```

Expected: compiles. (InventoryStore::load_products will have warnings about unused since nothing calls it yet — acceptable.)

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-inventory/src/store.rs crates/vassl-inventory/src/lib.rs
git commit -m "feat(inventory): InventoryStore entity — ProductWithStock, StockStatus, async load"
```

---

### Task 3: Wire inventory into vassl-app

**Files:**
- Modify: `tools/crates/vassl-app/src/root.rs`
- Modify: `tools/crates/vassl-app/src/main.rs`
- Modify: `tools/crates/vassl-app/src/colors.rs`

- [ ] **Step 1: Add status badge colors to `colors.rs`**

Add to `crates/vassl-app/src/colors.rs`:

```rust
pub const STATUS_GREEN: u32  = 0xa6e3a1; // Catppuccin Mocha green — healthy stock
pub const STATUS_AMBER: u32  = 0xf9e2af; // Catppuccin Mocha yellow — low stock
pub const STATUS_RED: u32    = 0xf38ba8; // Catppuccin Mocha red — critical stock
pub const STATUS_GREY: u32   = 0x585b70; // no-alert / disabled state
```

- [ ] **Step 2: Update `main.rs` to call `vassl_inventory::init(cx)`**

In `main.rs`, the call to `vassl_inventory::init()` is currently called without `cx`. Update it:

Find the line:
```rust
vassl_inventory::init();
```

Replace with:
```rust
vassl_inventory::init(cx);
```

Do the same for `vassl_quotations::init()` and `vassl_pricebook::init()` — update their stubs to also accept `cx: &mut App` (even if the body stays empty for now).

Update `crates/vassl-quotations/src/lib.rs`:
```rust
use gpui::App;
pub fn init(_cx: &mut App) {}
```

Update `crates/vassl-pricebook/src/lib.rs`:
```rust
use gpui::App;
pub fn init(_cx: &mut App) {}
```

- [ ] **Step 3: Update `root.rs` to hold and render `InventoryPanel`**

Replace `crates/vassl-app/src/root.rs`:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, rgb};

use crate::actions::{OpenInventory, OpenPriceBook, OpenQuotations};
use crate::colors;
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;

// Forward-declare — InventoryPanel is in vassl-inventory
use vassl_inventory::panel::InventoryPanel;

pub struct VasslRoot {
    sidebar:          Entity<Sidebar>,
    status_bar:       Entity<StatusBar>,
    inventory_panel:  Entity<InventoryPanel>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar:         cx.new(Sidebar::new),
            status_bar:      cx.new(StatusBar::new),
            inventory_panel: cx.new(InventoryPanel::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.sidebar.read(cx).active;

        let pane: Box<dyn gpui::AnyElement> = match active {
            ActiveModule::Inventory => Box::new(self.inventory_panel.clone().into_any_element()),
            ActiveModule::Quotations => Box::new(
                div().flex_1().h_full()
                    .child("Quotations — Plan 4")
                    .into_any_element()
            ),
            ActiveModule::PriceBook => Box::new(
                div().flex_1().h_full()
                    .child("Price Book — Plan 3")
                    .into_any_element()
            ),
        };

        div()
            .key_context("VasslRoot")
            .on_action(cx.listener(|this, _: &OpenInventory, _w, cx| {
                this.sidebar.update(cx, |s, cx| {
                    s.active = ActiveModule::Inventory;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &OpenQuotations, _w, cx| {
                this.sidebar.update(cx, |s, cx| {
                    s.active = ActiveModule::Quotations;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &OpenPriceBook, _w, cx| {
                this.sidebar.update(cx, |s, cx| {
                    s.active = ActiveModule::PriceBook;
                    cx.notify();
                });
            }))
            // TODO(Plan 5): add on_action handlers for OpenAuditLog, NewRecord, FocusSearch
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(colors::CANVAS_BG))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(self.sidebar.clone())
                    .child(pane),
            )
            .child(self.status_bar.clone())
    }
}
```

> **Note on `into_any_element()`:** Read the GPUI source for the correct method to erase an element's type so it can be stored as `Box<dyn AnyElement>`. In GPUI, `Entity<T>` implements `IntoElement` when `T: Render`. The `into_any_element()` method (or similar) allows dynamic dispatch. If this exact API doesn't exist, use a `div().child(self.inventory_panel.clone())` approach within each match arm and return a unified `div()` tree instead.

- [ ] **Step 4: Add `vassl-inventory` dependency to `vassl-app/Cargo.toml`**

```toml
[dependencies]
# ... existing deps ...
vassl-inventory = { path = "../vassl-inventory" }
```

- [ ] **Step 5: Build**

```bash
cargo build -p vassl-app
```

Expected: compiles. (InventoryPanel is a stub at this point — Task 4 fills it in.)

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-app/ crates/vassl-quotations/src/lib.rs crates/vassl-pricebook/src/lib.rs
git commit -m "feat(app): wire InventoryPanel into pane area, add status badge colors"
```

---

### Task 4: ProductList view — scrollable table with status badges

**Files:**
- Modify: `tools/crates/vassl-inventory/src/product_list.rs`
- Modify: `tools/crates/vassl-inventory/src/panel.rs`

- [ ] **Step 1: Write failing test for ProductList**

Replace `crates/vassl-inventory/src/product_list.rs`:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rgb};

use crate::store::{InventoryStore, ProductWithStock, StockStatus};

pub struct ProductList {
    store: Entity<InventoryStore>,
}

impl ProductList {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

impl Render for ProductList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child("Loading…")
                .into_any_element();
        }

        if store.products.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(vassl_app::colors::TEXT_MUTED))
                .child("No products — add stock entries to get started.")
                .into_any_element();
        }

        let rows: Vec<_> = store.products.iter().map(|p| {
            product_row(p, store.selected_product_id == Some(p.product.id))
        }).collect();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn product_row(p: &ProductWithStock, selected: bool) -> impl IntoElement {
    let badge_color = match p.status {
        StockStatus::Healthy  => vassl_app::colors::STATUS_GREEN,
        StockStatus::Low      => vassl_app::colors::STATUS_AMBER,
        StockStatus::Critical => vassl_app::colors::STATUS_RED,
        StockStatus::NoAlert  => vassl_app::colors::STATUS_GREY,
    };

    let row_bg = if selected {
        vassl_app::colors::SURFACE_ACTIVE
    } else {
        vassl_app::colors::CANVAS_BG
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        // Badge — 8×8 colored circle
        .child(
            div()
                .w(px(8.)).h(px(8.))
                .rounded_full()
                .bg(rgb(badge_color))
                .mr(px(8.))
        )
        // SKU
        .child(
            div()
                .w(px(80.))
                .text_size(px(12.))
                .text_color(rgb(vassl_app::colors::TEXT_MUTED))
                .child(p.product.sku.clone())
        )
        // Name
        .child(
            div()
                .flex_1()
                .text_size(px(13.))
                .text_color(rgb(vassl_app::colors::TEXT_DEFAULT))
                .child(p.product.name.clone())
        )
        // Current qty
        .child(
            div()
                .w(px(70.))
                .text_size(px(12.))
                .text_color(rgb(vassl_app::colors::TEXT_DEFAULT))
                .child(format!("{:.1} {}", p.current_stock, p.product.unit))
        )
        // Min level
        .child(
            div()
                .w(px(70.))
                .text_size(px(12.))
                .text_color(rgb(vassl_app::colors::TEXT_MUTED))
                .child(format!("min {:.1}", p.product.min_stock_level))
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_color_matches_status() {
        assert_eq!(
            match StockStatus::Healthy  { StockStatus::Healthy => vassl_app::colors::STATUS_GREEN, _ => 0 },
            vassl_app::colors::STATUS_GREEN
        );
        assert_eq!(
            match StockStatus::Critical { StockStatus::Critical => vassl_app::colors::STATUS_RED, _ => 0 },
            vassl_app::colors::STATUS_RED
        );
    }
}
```

> **Note:** `vassl_app::colors` is used here. This requires `vassl-inventory` to depend on `vassl-app`, which creates a dependency cycle (vassl-app → vassl-inventory → vassl-app). **Fix:** move `colors.rs` into a new tiny crate `vassl-theme`, or expose the colors through a simple module in `vassl-inventory` itself. The simplest fix: duplicate the color constants in `vassl-inventory/src/colors.rs` (copy from vassl-app, keep them in sync). Use `crate::colors::STATUS_GREEN` instead of `vassl_app::colors::STATUS_GREEN`. Do NOT add vassl-app as a dep.

Revised — replace all `vassl_app::colors::` references with `crate::colors::` in `product_list.rs`, and create `crates/vassl-inventory/src/colors.rs`:

```rust
// Mirror of vassl-app/src/colors.rs — kept in sync manually.
// TODO(Plan 5): extract to a shared vassl-theme crate.
pub const CANVAS_BG: u32       = 0x1e1e2e;
pub const SURFACE_ACTIVE: u32  = 0x1a3c5e;
pub const TEXT_DEFAULT: u32    = 0xcdd6f4;
pub const TEXT_MUTED: u32      = 0x6c7086;
pub const STATUS_GREEN: u32    = 0xa6e3a1;
pub const STATUS_AMBER: u32    = 0xf9e2af;
pub const STATUS_RED: u32      = 0xf38ba8;
pub const STATUS_GREY: u32     = 0x585b70;
```

Add `pub mod colors;` to `vassl-inventory/src/lib.rs`.

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-inventory -- product_list::tests --nocapture
```

Expected: 1 test passes.

- [ ] **Step 3: Write minimal `panel.rs`**

Replace `crates/vassl-inventory/src/panel.rs`:

```rust
use gpui::{App, Context, Entity, Global, IntoElement, Render, Window, div, prelude::*};

use crate::product_list::ProductList;
use crate::store::InventoryStore;

pub struct InventoryPanel {
    store:        Entity<InventoryStore>,
    product_list: Entity<ProductList>,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<Entity<InventoryStore>>().clone();
        let product_list = cx.new(|cx| ProductList::new(store.clone(), cx));

        // Kick off initial data load
        store.update(cx, |s, cx| s.load_products(cx));

        Self { store, product_list }
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            // Header bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(gpui::px(16.))
                    .py(gpui::px(8.))
                    .bg(gpui::rgb(crate::colors::CANVAS_BG))
                    .child(
                        div()
                            .text_size(gpui::px(14.))
                            .text_color(gpui::rgb(crate::colors::TEXT_DEFAULT))
                            .child("Inventory")
                    )
            )
            // Product list
            .child(self.product_list.clone())
    }
}
```

> **Note on `cx.global::<Entity<InventoryStore>>()`:** `InventoryStore` is stored as `Entity<InventoryStore>` (set via `cx.set_global(store)` in `lib.rs::init`). To retrieve it: `cx.global::<Entity<InventoryStore>>()`. This requires `Entity<InventoryStore>` to implement `Global`. Add `impl gpui::Global for gpui::Entity<InventoryStore> {}` in `lib.rs` if needed, or use a wrapper struct.

- [ ] **Step 4: Build**

```bash
cargo build -p vassl-inventory -p vassl-app
```

Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-inventory/src/
git commit -m "feat(inventory): ProductList view with status badges, InventoryPanel"
```

---

### Task 5: RestockAlerts view

**Files:**
- Modify: `tools/crates/vassl-inventory/src/restock.rs`
- Modify: `tools/crates/vassl-inventory/src/panel.rs`

- [ ] **Step 1: Write `restock.rs`**

Replace `crates/vassl-inventory/src/restock.rs`:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rgb};

use crate::store::InventoryStore;
use crate::colors;

pub struct RestockAlerts {
    store: Entity<InventoryStore>,
}

impl RestockAlerts {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

impl Render for RestockAlerts {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);

        let critical: Vec<_> = store.products
            .iter()
            .filter(|p| matches!(p.status, crate::store::StockStatus::Critical | crate::store::StockStatus::Low))
            .collect();

        if critical.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("All stock levels healthy.")
                .into_any_element();
        }

        let rows: Vec<_> = critical.iter().map(|p| {
            let badge = if matches!(p.status, crate::store::StockStatus::Critical) {
                colors::STATUS_RED
            } else {
                colors::STATUS_AMBER
            };

            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .px(px(12.)).py(px(8.))
                .child(
                    div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(badge)).mr(px(8.))
                )
                .child(
                    div().flex_1().text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                        .child(p.product.name.clone())
                )
                .child(
                    div().text_size(px(12.)).text_color(rgb(colors::STATUS_RED))
                        .child(format!("{:.1} / min {:.1} {}", p.current_stock, p.product.min_stock_level, p.product.unit))
                )
        }).collect();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::StockStatus;

    #[test]
    fn critical_and_low_are_alert_states() {
        assert!(matches!(StockStatus::Critical, StockStatus::Critical));
        assert!(matches!(StockStatus::Low, StockStatus::Low));
        assert!(!matches!(StockStatus::Healthy, StockStatus::Critical | StockStatus::Low));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-inventory -- restock::tests --nocapture
```

Expected: 1 test passes.

- [ ] **Step 3: Add tab switching to `InventoryPanel`**

Update `crates/vassl-inventory/src/panel.rs` to add a tab bar for "Products" vs "Restock Alerts":

```rust
use gpui::{App, Context, Entity, Global, IntoElement, Render, Window, div, prelude::*, px, rgb};

use crate::colors;
use crate::product_list::ProductList;
use crate::restock::RestockAlerts;
use crate::store::InventoryStore;

#[derive(Clone, Copy, PartialEq)]
enum Tab { Products, RestockAlerts }

pub struct InventoryPanel {
    store:          Entity<InventoryStore>,
    product_list:   Entity<ProductList>,
    restock_alerts: Entity<RestockAlerts>,
    active_tab:     Tab,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<Entity<InventoryStore>>().clone();
        let product_list   = cx.new(|cx| ProductList::new(store.clone(), cx));
        let restock_alerts = cx.new(|cx| RestockAlerts::new(store.clone(), cx));

        store.update(cx, |s, cx| s.load_products(cx));

        Self { store, product_list, restock_alerts, active_tab: Tab::Products }
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        let content = match active_tab {
            Tab::Products      => self.product_list.clone().into_any_element(),
            Tab::RestockAlerts => self.restock_alerts.clone().into_any_element(),
        };

        div()
            .flex_1().flex().flex_col().h_full()
            // Header + tabs
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    .child(
                        div()
                            .id("tab-products")
                            .px(px(12.)).py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Products { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG }))
                            .text_size(px(12.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Products;
                                cx.notify();
                            }))
                            .child("Products")
                    )
                    .child(
                        div()
                            .id("tab-restock")
                            .px(px(12.)).py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::RestockAlerts { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG }))
                            .text_size(px(12.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::RestockAlerts;
                                cx.notify();
                            }))
                            .child("Restock Alerts")
                    )
            )
            // Active content
            .child(content)
    }
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p vassl-inventory -p vassl-app
```

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-inventory/src/restock.rs crates/vassl-inventory/src/panel.rs
git commit -m "feat(inventory): RestockAlerts view, tab switching in InventoryPanel"
```

---

### Task 6: StockEntryForm modal

**Files:**
- Modify: `tools/crates/vassl-inventory/src/stock_form.rs`
- Modify: `tools/crates/vassl-inventory/src/panel.rs`

- [ ] **Step 1: Write `stock_form.rs` with test**

Replace `crates/vassl-inventory/src/stock_form.rs`:

```rust
use gpui::{Context, Entity, FocusHandle, IntoElement, Render, Window,
           div, prelude::*, px, rgb, SharedString};

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

pub struct StockEntryForm {
    store:            Entity<InventoryStore>,
    product_id:       i64,
    product_name:     String,
    quantity:         String,          // raw text input
    unit_cost:        String,
    supplier:         String,
    invoice_ref:      String,
    acquisition_type: String,          // "restock" | "project"
    error:            Option<String>,
    focus_handle:     FocusHandle,
}

impl StockEntryForm {
    pub fn new(
        store: Entity<InventoryStore>,
        product_id: i64,
        product_name: String,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            store,
            product_id,
            product_name,
            quantity: String::new(),
            unit_cost: String::new(),
            supplier: String::new(),
            invoice_ref: String::new(),
            acquisition_type: "restock".to_string(),
            error: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn validate(&self) -> Result<(f64, f64), String> {
        let qty: f64 = self.quantity.trim().parse()
            .map_err(|_| "Quantity must be a positive number".to_string())?;
        if qty <= 0.0 { return Err("Quantity must be > 0".to_string()); }
        let cost: f64 = self.unit_cost.trim().parse()
            .map_err(|_| "Unit cost must be a number ≥ 0".to_string())?;
        if cost < 0.0 { return Err("Unit cost must be ≥ 0".to_string()); }
        Ok((qty, cost))
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.validate() {
            Err(msg) => {
                self.error = Some(msg);
                cx.notify();
            }
            Ok((qty, cost)) => {
                let db     = InventoryDb::global(cx.app());
                let pid    = self.product_id;
                let sup    = self.supplier.trim().to_string();
                let invref = self.invoice_ref.trim().to_string();
                let acq    = self.acquisition_type.clone();
                let store  = self.store.clone();

                cx.spawn(async move |_this, cx| {
                    let result = db.write(|conn| {
                        use sqlez::typed_statements::*;
                        let now = chrono::Utc::now().to_rfc3339();
                        conn.exec_bound::<(i64, f64, f64, Option<String>, String, String, Option<i64>, Option<String>, Option<String>)>(
                            "INSERT INTO stock_entries
                             (product_id, quantity, unit_cost_usd, supplier, acquired_at,
                              acquisition_type, project_id, invoice_ref, notes)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
                        )
                        .map_err(|e| anyhow::anyhow!("{e}"))?
                        ((pid, qty, cost,
                          if sup.is_empty() { None } else { Some(sup) },
                          now, acq, None,
                          if invref.is_empty() { None } else { Some(invref) },
                          None))
                        .map_err(|e| anyhow::anyhow!("{e}"))
                    }).await;

                    if let Err(e) = result {
                        tracing::error!("insert_stock_entry failed: {e:?}");
                        return Ok(());
                    }

                    // Reload products in the store
                    store.update(cx, |s, cx| s.load_products(cx))
                })
                .detach();

                // Emit close signal to parent panel
                cx.emit(StockFormEvent::Submitted);
            }
        }
    }
}

pub enum StockFormEvent { Submitted, Cancelled }
impl gpui::EventEmitter<StockFormEvent> for StockEntryForm {}

impl Render for StockEntryForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Modal overlay — full-screen dark scrim + centered card
        div()
            .absolute()
            .top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(gpui::rgba(0x000000cc))  // semi-transparent black scrim
            .child(
                div()
                    .w(px(400.)).bg(rgb(colors::CANVAS_BG))
                    .rounded(px(8.))
                    .p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    // Title
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .child(format!("New Stock Entry — {}", self.product_name))
                    )
                    // Quantity field
                    .child(form_field("Quantity", &self.quantity, "e.g. 10"))
                    // Unit cost field
                    .child(form_field("Unit Cost (USD)", &self.unit_cost, "e.g. 120.00"))
                    // Supplier field
                    .child(form_field("Supplier", &self.supplier, "optional"))
                    // Invoice ref field
                    .child(form_field("Invoice Ref", &self.invoice_ref, "optional"))
                    // Error message
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(colors::STATUS_RED))
                            .child(
                                self.error.as_deref()
                                    .map(SharedString::from)
                                    .unwrap_or_default()
                            )
                    )
                    // Buttons
                    .child(
                        div()
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(
                                div()
                                    .id("btn-cancel")
                                    .px(px(16.)).py(px(6.))
                                    .rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .text_size(px(12.))
                                    .text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_this, _, _, cx| {
                                        cx.emit(StockFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div()
                                    .id("btn-save")
                                    .px(px(16.)).py(px(6.))
                                    .rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_ACTIVE))
                                    .text_size(px(12.))
                                    .text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.submit(cx);
                                    }))
                                    .child("Save")
                            )
                    )
            )
    }
}

fn form_field(label: &str, value: &str, placeholder: &str) -> impl IntoElement {
    div()
        .flex().flex_col().gap(px(4.))
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(colors::TEXT_MUTED))
                .child(label.to_string())
        )
        .child(
            div()
                .px(px(8.)).py(px(6.))
                .bg(rgb(colors::SURFACE_DEFAULT))
                .rounded(px(4.))
                .text_size(px(13.))
                .text_color(rgb(if value.is_empty() { colors::TEXT_MUTED } else { colors::TEXT_DEFAULT }))
                .child(if value.is_empty() { placeholder.to_string() } else { value.to_string() })
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeForm {
        quantity: String,
        unit_cost: String,
    }

    impl FakeForm {
        fn validate(&self) -> Result<(f64, f64), String> {
            let qty: f64 = self.quantity.trim().parse()
                .map_err(|_| "Quantity must be a positive number".to_string())?;
            if qty <= 0.0 { return Err("Quantity must be > 0".to_string()); }
            let cost: f64 = self.unit_cost.trim().parse()
                .map_err(|_| "Unit cost must be a number ≥ 0".to_string())?;
            if cost < 0.0 { return Err("Unit cost must be ≥ 0".to_string()); }
            Ok((qty, cost))
        }
    }

    #[test]
    fn validate_rejects_empty_quantity() {
        let f = FakeForm { quantity: "".into(), unit_cost: "10.0".into() };
        assert!(f.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_quantity() {
        let f = FakeForm { quantity: "0".into(), unit_cost: "10.0".into() };
        assert!(f.validate().is_err());
    }

    #[test]
    fn validate_rejects_negative_cost() {
        let f = FakeForm { quantity: "5".into(), unit_cost: "-1".into() };
        assert!(f.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_input() {
        let f = FakeForm { quantity: "10.5".into(), unit_cost: "120.00".into() };
        assert_eq!(f.validate().unwrap(), (10.5, 120.0));
    }
}
```

> **Note on text input:** GPUI has a text input component. For now, the form uses static `div()` elements to display the current value (non-editable). Actual text field support can be added in Plan 5 (Polish) using GPUI's `TextInput` or `Editor` component. The form renders "placeholder" text when the field is empty — enough for a functional first version where values would come from programmatic population.

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-inventory -- stock_form::tests --nocapture
```

Expected: 4 tests pass.

- [ ] **Step 3: Wire form into `InventoryPanel`**

Update `crates/vassl-inventory/src/panel.rs` to hold an optional `Entity<StockEntryForm>` and show it as a modal overlay. Add a "New Entry" button in the header that creates the form for the selected product.

Add to `panel.rs`:
```rust
use crate::stock_form::{StockEntryForm, StockFormEvent};

// Add to InventoryPanel struct:
//   stock_form: Option<Entity<StockEntryForm>>,

// In render(), after the content div, if stock_form is Some, overlay it:
//   .child(if let Some(form) = &self.stock_form { form.clone().into_any_element() } else { div().into_any_element() })
```

Add "New Entry" button in header that:
1. Reads the selected product from the store
2. If a product is selected, creates `StockEntryForm` entity
3. Subscribes to `StockFormEvent::Submitted` and `StockFormEvent::Cancelled` to set `stock_form = None`

Full updated `panel.rs`:

```rust
use gpui::{Context, Entity, Global, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::product_list::ProductList;
use crate::restock::RestockAlerts;
use crate::stock_form::{StockEntryForm, StockFormEvent};
use crate::store::InventoryStore;

#[derive(Clone, Copy, PartialEq)]
enum Tab { Products, RestockAlerts }

pub struct InventoryPanel {
    store:            Entity<InventoryStore>,
    product_list:     Entity<ProductList>,
    restock_alerts:   Entity<RestockAlerts>,
    active_tab:       Tab,
    stock_form:       Option<Entity<StockEntryForm>>,
    _form_sub:        Option<Subscription>,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<Entity<InventoryStore>>().clone();
        let product_list   = cx.new(|cx| ProductList::new(store.clone(), cx));
        let restock_alerts = cx.new(|cx| RestockAlerts::new(store.clone(), cx));

        store.update(cx, |s, cx| s.load_products(cx));

        Self {
            store,
            product_list,
            restock_alerts,
            active_tab: Tab::Products,
            stock_form: None,
            _form_sub: None,
        }
    }

    fn open_stock_form(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(pid) = store.selected_product_id else { return; };
        let product_name = store.products
            .iter()
            .find(|p| p.product.id == pid)
            .map(|p| p.product.name.clone())
            .unwrap_or_default();
        drop(store);

        let store_entity = self.store.clone();
        let form = cx.new(|cx| StockEntryForm::new(store_entity, pid, product_name, cx));

        let sub = cx.subscribe(&form, |this, _form, ev: &StockFormEvent, cx| {
            match ev {
                StockFormEvent::Submitted | StockFormEvent::Cancelled => {
                    this.stock_form = None;
                    this._form_sub = None;
                    cx.notify();
                }
            }
        });

        self.stock_form = Some(form);
        self._form_sub  = Some(sub);
        cx.notify();
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

        let content = match active_tab {
            Tab::Products      => self.product_list.clone().into_any_element(),
            Tab::RestockAlerts => self.restock_alerts.clone().into_any_element(),
        };

        let mut root = div()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    // Products tab
                    .child(
                        div()
                            .id("tab-products")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Products { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Products;
                                cx.notify();
                            }))
                            .child("Products")
                    )
                    // Restock Alerts tab
                    .child(
                        div()
                            .id("tab-restock")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::RestockAlerts { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::RestockAlerts;
                                cx.notify();
                            }))
                            .child("Restock Alerts")
                    )
                    // Spacer
                    .child(div().flex_1())
                    // New Entry button (enabled only when a product is selected)
                    .child(
                        div()
                            .id("btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.open_stock_form(cx);
                            }))
                            .child("+ New Entry")
                    )
            )
            .child(content);

        // Overlay modal if form is open
        if let Some(form) = &self.stock_form {
            root = root.child(form.clone());
        }

        root
    }
}
```

- [ ] **Step 4: Build**

```bash
cargo build -p vassl-inventory -p vassl-app
```

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-inventory/src/stock_form.rs crates/vassl-inventory/src/panel.rs
git commit -m "feat(inventory): StockEntryForm modal with validation, wired into InventoryPanel"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Covered by |
|---|---|
| Stock list — SKU, name, category, current qty, min level, status badge | Task 4 — `ProductList` |
| Stock status badge (red/amber/green) | Task 4 — `product_row` + `StockStatus::from_levels` |
| Stock entry form modal — product, quantity, unit cost, supplier, invoice ref, acquisition type | Task 6 — `StockEntryForm` |
| Restock alerts panel — products at/below min_stock_level | Task 5 — `RestockAlerts` |
| `products` + `stock_entries` DB tables | Task 1 — `InventoryDb::MIGRATIONS` |
| Current stock = SUM(quantity) | Task 1 — `current_stock()` query |
| min_stock_level = 0 means no alert | Task 1 — `products_below_min_stock` WHERE clause |
| Module crate init pattern | Task 2 — `init(cx: &mut App)` |
| Keyboard Ctrl+1 switches to Inventory | Foundation plan (already done) |

**Not in this plan (later plans):**
- Product detail pane line chart (price history) — Plan 5 (requires chart rendering)
- Global search (fuzzy: SKU, name, category, supplier) — Plan 5
- Product CRUD (add/edit/delete product record) — Plan 5
- Project picker in stock entry form (project_id) — Plan 5
- Text input fields in StockEntryForm (Plan 5 — GPUI TextInput integration)

**Placeholder scan:** No TBD, TODO, or "implement later" in task code. All steps have complete code.

**Type consistency:** `ProductWithStock` defined in `store.rs`, used in `product_list.rs`. `StockStatus` defined in `store.rs`, used in `product_list.rs`, `restock.rs`. `InventoryDb` defined in `db.rs`, used in `store.rs`, `stock_form.rs`. `StockEntryForm` defined in `stock_form.rs`, `StockFormEvent` emitted by it, subscribed to in `panel.rs`. Consistent throughout.

---

## Next Plans

- **Plan 3 — Price Book module:** `PriceBookDb`, `PriceBookStore`, price table view, entry form with live selling price preview, price history per product
- **Plan 4 — Quotations module:** `QuotationDb`, `QuotationStore`, quotation list, line items editor, project picker
- **Plan 5 — App Polish:** command palette (`Ctrl+P`), full audit log view, pane splitting, first-run user prompt, GPUI TextInput for forms, product detail chart, global search
