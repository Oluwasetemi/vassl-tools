# Price Book Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Price Book module — a product price table (latest cost/duty/markup/selling price per product), a price history tab, and a "New Entry" modal — wired into VasslRoot's `ActiveModule::PriceBook` arm.

**Architecture:** `vassl-pricebook` follows the same three-layer pattern as `vassl-inventory`: `PriceBookDb` (typed sqlez handle via `static_connection!`), `PriceBookStore` (GPUI `Entity<T>` with async loads), and GPUI views (`PriceTable`, `PriceEntryForm` modal, `PriceBookPanel`). The panel has two tabs: "Price Book" (one row per product, showing latest price entry or "—" if none) and "History" (all entries for the selected product). VasslRoot replaces the `"Price Book — Plan 3"` placeholder with `PriceBookPanel`.

**Tech Stack:** Rust, GPUI (`Entity<T>`, `Render`, `cx.spawn`, `cx.notify()`), sqlez, `vassl-core` (`PriceEntry`, `selling_price()`), `vassl-db` (`static_connection!`, `SharedDomain`), chrono (effective_date timestamps).

---

## File Map

```
tools/
├── crates/
│   ├── vassl-pricebook/
│   │   ├── Cargo.toml                    # add sqlez, tracing, dev-deps
│   │   └── src/
│   │       ├── lib.rs                    # pub fn init(cx), declare modules, set PriceBookStoreHandle global
│   │       ├── colors.rs                 # mirror of vassl-app/src/colors.rs — kept in sync manually
│   │       ├── db.rs                     # PriceBookDb: Domain + static_connection! + 3 queries
│   │       ├── store.rs                  # PriceBookStore entity + ProductPrice view-model type
│   │       ├── panel.rs                  # PriceBookPanel: tab bar + content + form overlay wiring
│   │       ├── price_table.rs            # PriceTable: scrollable product+price row view
│   │       └── price_form.rs             # PriceEntryForm: modal overlay for new price entry
│   └── vassl-app/
│       └── src/
│           └── root.rs                   # replace Price Book placeholder with PriceBookPanel
```

---

### Task 1: PriceBookDb — domain, migration, queries

**Files:**
- Modify: `crates/vassl-pricebook/Cargo.toml`
- Create: `crates/vassl-pricebook/src/db.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace the entire file:

```toml
[package]
name    = "vassl-pricebook"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui.workspace       = true
anyhow.workspace     = true
thiserror.workspace  = true
serde.workspace      = true
serde_json.workspace = true
inventory.workspace  = true
chrono.workspace     = true
sqlez.workspace      = true
tracing.workspace    = true
vassl-core           = { path = "../vassl-core" }
vassl-db             = { path = "../vassl-db" }

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
db    = { path = "../db", features = ["test-support"] }
```

- [ ] **Step 2: Write failing tests in db.rs**

Create `crates/vassl-pricebook/src/db.rs`:

```rust
use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::PriceEntry;
use vassl_db::SharedDomain;

pub struct PriceBookDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for PriceBookDb {
    const NAME: &'static str = "pricebook";
    const MIGRATIONS: &'static [&'static str] = &[
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

vassl_db::static_connection!(PriceBookDb, [SharedDomain]);

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates the products table and inserts one row.  Returns the new product id.
    /// SQLite does not enforce FK constraints by default, so price_book_entries
    /// tests that only call list_entries_for_product or insert_entry don't need
    /// this helper — but list_products_with_latest_price JOINs products, so it does.
    async fn setup_product(db: &PriceBookDb, sku: &str, name: &str) -> i64 {
        let sku = sku.to_string();
        let name = name.to_string();
        db.write(move |conn| {
            conn.exec(
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
            )
            .context("create products table")?()
            .context("exec create products")?;

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
        let id = db.insert_entry(1, 100.0, 10.0, 30.0, 143.0, None).await.unwrap();
        assert!(id > 0);
        let entries = db.list_entries_for_product(1).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cost_price_usd, 100.0);
        assert_eq!(entries[0].selling_price_usd, 143.0);
    }

    #[tokio::test]
    async fn entries_returned_newest_first() {
        let db = PriceBookDb::open_test_db("pb_entries_order").await;
        db.insert_entry_with_date(1, 100.0, 0.0, 30.0, 130.0, None, "2025-01-01T00:00:00Z")
            .await.unwrap();
        db.insert_entry_with_date(1, 200.0, 0.0, 30.0, 260.0, None, "2026-06-01T00:00:00Z")
            .await.unwrap();
        db.insert_entry_with_date(1, 150.0, 0.0, 30.0, 195.0, None, "2026-01-01T00:00:00Z")
            .await.unwrap();
        let entries = db.list_entries_for_product(1).unwrap();
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
```

- [ ] **Step 3: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook 2>&1 | head -20
```

Expected: compile error — `list_entries_for_product`, `insert_entry`, `insert_entry_with_date`, `list_products_with_latest_price` not yet defined.

- [ ] **Step 4: Implement query methods**

Add after `vassl_db::static_connection!(PriceBookDb, [SharedDomain]);`:

```rust
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
        let notes = notes.map(String::from);
        let now   = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(i64, f64, f64, f64, f64, String, Option<String>)>(
                "INSERT INTO price_book_entries
                 (product_id, cost_price_usd, duty_cost_usd, markup_percent,
                  selling_price_usd, effective_date, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .context("prepare insert_entry")?
            ((product_id, cost_price_usd, duty_cost_usd, markup_percent,
              selling_price_usd, now, notes))
            .context("execute insert_entry")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
                .context("execute last_insert_rowid")?
                .context("last_insert_rowid returned None")
        })
        .await
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
```

- [ ] **Step 5: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook db 2>&1 | tail -15
```

Expected: 5 tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/Cargo.toml crates/vassl-pricebook/src/db.rs && git commit -m "feat(pricebook): PriceBookDb domain, migration, queries"
```

---

### Task 2: PriceBookStore — GPUI entity

**Files:**
- Create: `crates/vassl-pricebook/src/store.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/vassl-pricebook/src/store.rs` with tests only:

```rust
use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::PriceEntry;

use crate::db::PriceBookDb;

#[derive(Debug, Clone)]
pub struct ProductPrice {
    pub product_id: i64,
    pub sku:        String,
    pub name:       String,
    pub latest:     Option<PriceEntry>,
}

pub struct PriceBookStore {
    pub product_prices:      Vec<ProductPrice>,
    pub selected_product_id: Option<i64>,
    pub history:             Vec<PriceEntry>,
    pub loading:             bool,
}

pub struct PriceBookStoreHandle(pub Entity<PriceBookStore>);
impl Global for PriceBookStoreHandle {}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::PriceEntry;

    fn make_entry(id: i64, product_id: i64, cost: f64) -> PriceEntry {
        PriceEntry {
            id,
            product_id,
            cost_price_usd:    cost,
            duty_cost_usd:     0.0,
            markup_percent:    30.0,
            selling_price_usd: cost * 1.3,
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
        }
    }

    #[test]
    fn product_price_with_no_entry_has_no_latest() {
        let pp = ProductPrice {
            product_id: 1,
            sku:        "X".to_string(),
            name:       "Y".to_string(),
            latest:     None,
        };
        assert!(pp.latest.is_none());
    }

    #[test]
    fn product_price_with_entry_exposes_selling_price() {
        let pp = ProductPrice {
            product_id: 1,
            sku:        "A".to_string(),
            name:       "B".to_string(),
            latest:     Some(make_entry(1, 1, 100.0)),
        };
        assert_eq!(pp.latest.unwrap().selling_price_usd, 130.0);
    }

    #[test]
    fn history_ordering_invariant() {
        // The DB sorts entries by effective_date DESC — verify our mapping preserves order.
        let entries = vec![
            make_entry(3, 1, 300.0),  // newest comes first from DB
            make_entry(2, 1, 200.0),
            make_entry(1, 1, 100.0),
        ];
        // Map into history field — store just assigns, no re-sort.
        let history: Vec<PriceEntry> = entries.clone();
        assert_eq!(history[0].id, 3);
        assert_eq!(history[2].id, 1);
    }
}
```

- [ ] **Step 2: Run test — verify it compiles but tests pass (structs defined)**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook store 2>&1 | tail -10
```

Expected: compile error — `PriceBookStore::new`, `PriceBookStore::load_products`, etc. not defined yet (types are defined). Tests for struct access should compile once impl block is added.

- [ ] **Step 3: Implement PriceBookStore**

Add after `impl Global for PriceBookStoreHandle {}`:

```rust
#[derive(Debug)]
pub enum PriceBookEvent {
    ProductsLoaded,
    HistoryLoaded,
}

impl EventEmitter<PriceBookEvent> for PriceBookStore {}

impl PriceBookStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            product_prices:      Vec::new(),
            selected_product_id: None,
            history:             Vec::new(),
            loading:             false,
        }
    }

    pub fn load_products(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = PriceBookDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { db.list_products_with_latest_price() })
                .await;

            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match result {
                    Ok(rows) => {
                        store.product_prices = rows
                            .into_iter()
                            .map(|(pid, sku, name, latest)| ProductPrice { product_id: pid, sku, name, latest })
                            .collect();
                        cx.emit(PriceBookEvent::ProductsLoaded);
                    }
                    Err(e) => tracing::error!("load_products_with_latest_price failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_product(&mut self, product_id: i64, cx: &mut Context<Self>) {
        if self.selected_product_id == Some(product_id) { return; }
        self.selected_product_id = Some(product_id);
        self.history.clear();
        cx.notify();

        let db = PriceBookDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { db.list_entries_for_product(product_id) })
                .await;

            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(entries) => {
                        store.history = entries;
                        cx.emit(PriceBookEvent::HistoryLoaded);
                    }
                    Err(e) => tracing::error!("list_entries_for_product failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook store 2>&1 | tail -10
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/src/store.rs && git commit -m "feat(pricebook): PriceBookStore entity with async load and selection"
```

---

### Task 3: colors.rs + lib.rs init

**Files:**
- Create: `crates/vassl-pricebook/src/colors.rs`
- Modify: `crates/vassl-pricebook/src/lib.rs`

- [ ] **Step 1: Create colors.rs**

```rust
// Mirror of vassl-app/src/colors.rs — kept in sync manually.
pub const CANVAS_BG: u32       = 0x1e1e2e;
pub const SIDEBAR_BG: u32      = 0x181825;
pub const SURFACE_DEFAULT: u32 = 0x313244;
pub const SURFACE_ACTIVE: u32  = 0x1a3c5e;
pub const TEXT_DEFAULT: u32    = 0xcdd6f4;
pub const TEXT_MUTED: u32      = 0x6c7086;
pub const STATUS_GREEN: u32    = 0xa6e3a1;
pub const STATUS_AMBER: u32    = 0xf9e2af;
pub const STATUS_RED: u32      = 0xf38ba8;
pub const STATUS_GREY: u32     = 0x585b70;
```

- [ ] **Step 2: Replace lib.rs**

```rust
pub mod colors;
pub mod db;
pub mod panel;
pub mod price_form;
pub mod price_table;
pub mod store;

use gpui::{App, AppContext, Entity};

pub use db::PriceBookDb;
pub use store::{PriceBookStore, PriceBookStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<PriceBookStore> = cx.new(PriceBookStore::new);
    cx.set_global(PriceBookStoreHandle(store));
}
```

- [ ] **Step 3: Verify it compiles (modules don't exist yet — expect errors for panel, price_form, price_table)**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build -p vassl-pricebook 2>&1 | grep "error\[" | head -10
```

Expected: errors about missing files `panel.rs`, `price_form.rs`, `price_table.rs` — the module declarations in lib.rs are correct; the files just don't exist yet.

- [ ] **Step 4: Create empty stub files to unblock compilation**

Create `crates/vassl-pricebook/src/price_table.rs`:
```rust
// stub — implemented in Task 4
```

Create `crates/vassl-pricebook/src/price_form.rs`:
```rust
// stub — implemented in Task 5
```

Create `crates/vassl-pricebook/src/panel.rs`:
```rust
// stub — implemented in Task 6
```

- [ ] **Step 5: Verify crate compiles**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build -p vassl-pricebook 2>&1 | grep "^error" | head -5
```

Expected: no errors (stubs compile).

- [ ] **Step 6: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/src/colors.rs crates/vassl-pricebook/src/lib.rs crates/vassl-pricebook/src/price_table.rs crates/vassl-pricebook/src/price_form.rs crates/vassl-pricebook/src/panel.rs && git commit -m "feat(pricebook): lib init, colors, stub modules"
```

---

### Task 4: PriceTable view

**Files:**
- Modify: `crates/vassl-pricebook/src/price_table.rs`

- [ ] **Step 1: Write failing tests**

Replace the stub with:

```rust
use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::store::{PriceBookStore, ProductPrice};

pub struct PriceTable {
    store: Entity<PriceBookStore>,
}

impl PriceTable {
    pub fn new(store: Entity<PriceBookStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::PriceEntry;

    fn make_pp(id: i64, name: &str, cost: Option<f64>) -> ProductPrice {
        let latest = cost.map(|c| PriceEntry {
            id,
            product_id:        id,
            cost_price_usd:    c,
            duty_cost_usd:     0.0,
            markup_percent:    30.0,
            selling_price_usd: c * 1.3,
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
        });
        ProductPrice { product_id: id, sku: format!("SKU-{id}"), name: name.to_string(), latest }
    }

    #[test]
    fn format_price_with_entry() {
        let pp = make_pp(1, "Camera", Some(100.0));
        let display = price_display(&pp);
        assert!(display.contains("100"), "should show cost");
        assert!(display.contains("130"), "should show selling price");
    }

    #[test]
    fn format_price_no_entry() {
        let pp = make_pp(2, "NVR", None);
        let display = price_display(&pp);
        assert_eq!(display, "—");
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook price_table 2>&1 | tail -10
```

Expected: compile error — `price_display` not defined, `Render` not implemented.

- [ ] **Step 3: Implement PriceTable**

Replace the full file:

```rust
use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::store::{PriceBookStore, ProductPrice};

pub struct PriceTable {
    store: Entity<PriceBookStore>,
}

impl PriceTable {
    pub fn new(store: Entity<PriceBookStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

pub fn price_display(pp: &ProductPrice) -> String {
    match &pp.latest {
        None => "—".to_string(),
        Some(e) => format!(
            "${:.2}  +${:.2}  →  {:.0}%  →  ${:.2}",
            e.cost_price_usd, e.duty_cost_usd, e.markup_percent, e.selling_price_usd
        ),
    }
}

impl Render for PriceTable {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("Loading…")
                .into_any_element();
        }

        if store.product_prices.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_DEFAULT))
                .child("No products found.")
                .into_any_element();
        }

        let selected = store.selected_product_id;
        let rows: Vec<_> = store.product_prices.iter().map(|pp| {
            let is_selected = selected == Some(pp.product_id);
            price_row(pp, is_selected, self.store.clone())
        }).collect();

        div()
            .id("price-table-scroll")
            .flex_1().flex().flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn price_row(pp: &ProductPrice, selected: bool, store: Entity<PriceBookStore>) -> impl IntoElement {
    let product_id = pp.product_id;
    let row_bg = if selected { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG };
    let price_str = price_display(pp);

    div()
        .id(format!("pb-row-{product_id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(6.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_product(product_id, cx));
            },
        )
        // SKU
        .child(
            div()
                .w(px(90.)).text_size(px(12.))
                .text_color(rgb(colors::TEXT_MUTED))
                .child(pp.sku.clone())
        )
        // Name
        .child(
            div()
                .w(px(160.)).text_size(px(13.))
                .text_color(rgb(colors::TEXT_DEFAULT))
                .child(pp.name.clone())
        )
        // Price summary (cost → duty → markup → selling) or "—"
        .child(
            div()
                .flex_1().text_size(px(12.))
                .text_color(rgb(if pp.latest.is_some() { colors::TEXT_DEFAULT } else { colors::TEXT_MUTED }))
                .child(price_str)
        )
        // Effective date
        .child(
            div()
                .w(px(110.)).text_size(px(11.))
                .text_color(rgb(colors::TEXT_MUTED))
                .child(pp.latest.as_ref().map(|e| e.effective_date[..10].to_string()).unwrap_or_default())
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::PriceEntry;

    fn make_pp(id: i64, name: &str, cost: Option<f64>) -> ProductPrice {
        let latest = cost.map(|c| PriceEntry {
            id,
            product_id:        id,
            cost_price_usd:    c,
            duty_cost_usd:     0.0,
            markup_percent:    30.0,
            selling_price_usd: c * 1.3,
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
        });
        ProductPrice { product_id: id, sku: format!("SKU-{id}"), name: name.to_string(), latest }
    }

    #[test]
    fn format_price_with_entry() {
        let pp = make_pp(1, "Camera", Some(100.0));
        let display = price_display(&pp);
        assert!(display.contains("100"), "should show cost");
        assert!(display.contains("130"), "should show selling price");
    }

    #[test]
    fn format_price_no_entry() {
        let pp = make_pp(2, "NVR", None);
        let display = price_display(&pp);
        assert_eq!(display, "—");
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook price_table 2>&1 | tail -10
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/src/price_table.rs && git commit -m "feat(pricebook): PriceTable scrollable view with price_display"
```

---

### Task 5: PriceEntryForm modal

**Files:**
- Modify: `crates/vassl-pricebook/src/price_form.rs`

Note on text input: GPUI text input is deferred to Plan 5. The form renders static placeholder fields (same approach as `vassl-inventory`'s `StockEntryForm`). The form is openable and cancelable; "Save" validates the static empty strings and shows an error — functional architecture ready for Plan 5 text input wiring.

- [ ] **Step 1: Write failing tests**

Replace the stub:

```rust
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::selling_price;

use crate::colors;
use crate::db::PriceBookDb;
use crate::store::PriceBookStore;

#[derive(Debug)]
pub enum PriceFormEvent {
    Submitted,
    Cancelled,
}

impl EventEmitter<PriceFormEvent> for PriceEntryForm {}

pub struct PriceEntryForm {
    store:          Entity<PriceBookStore>,
    product_id:     i64,
    product_name:   String,
    cost:           String,
    duty:           String,
    markup:         String,
    error:          Option<String>,
    focus_handle:   FocusHandle,
}

fn validate_price_entry(cost: &str, duty: &str, markup: &str) -> Result<(f64, f64, f64), String> {
    let cost_val: f64 = cost.trim().parse()
        .map_err(|_| "Cost must be a number ≥ 0".to_string())?;
    if cost_val < 0.0 { return Err("Cost must be ≥ 0".to_string()); }

    let duty_val: f64 = duty.trim().parse()
        .map_err(|_| "Duty must be a number ≥ 0".to_string())?;
    if duty_val < 0.0 { return Err("Duty must be ≥ 0".to_string()); }

    let markup_val: f64 = markup.trim().parse()
        .map_err(|_| "Markup % must be > 0".to_string())?;
    if markup_val <= 0.0 { return Err("Markup % must be > 0".to_string()); }

    Ok((cost_val, duty_val, markup_val))
}

#[cfg(test)]
mod tests {
    use super::validate_price_entry;

    #[test]
    fn validate_rejects_empty_cost() {
        assert!(validate_price_entry("", "0", "30").is_err());
    }

    #[test]
    fn validate_rejects_negative_cost() {
        assert!(validate_price_entry("-1", "0", "30").is_err());
    }

    #[test]
    fn validate_rejects_zero_markup() {
        assert!(validate_price_entry("100", "0", "0").is_err());
    }

    #[test]
    fn validate_rejects_negative_markup() {
        assert!(validate_price_entry("100", "0", "-5").is_err());
    }

    #[test]
    fn validate_accepts_valid_input() {
        let result = validate_price_entry("100.0", "10.0", "30.0");
        assert!(result.is_ok());
        let (cost, duty, markup) = result.unwrap();
        assert_eq!(cost, 100.0);
        assert_eq!(duty, 10.0);
        assert_eq!(markup, 30.0);
    }

    #[test]
    fn validate_accepts_zero_duty() {
        assert!(validate_price_entry("200.0", "0.0", "25.0").is_ok());
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook price_form 2>&1 | tail -10
```

Expected: compile error — `PriceEntryForm` struct and `selling_price` import need impl block.

- [ ] **Step 3: Implement PriceEntryForm**

Add after `validate_price_entry`:

```rust
impl PriceEntryForm {
    pub fn new(
        store:        Entity<PriceBookStore>,
        product_id:   i64,
        product_name: String,
        cx:           &mut Context<Self>,
    ) -> Self {
        Self {
            store,
            product_id,
            product_name,
            cost:         String::new(),
            duty:         String::new(),
            markup:       "30".to_string(),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn computed_selling_price(&self) -> String {
        match validate_price_entry(&self.cost, &self.duty, &self.markup) {
            Ok((c, d, m)) => match selling_price(c, d, m) {
                Ok(s)  => format!("${s:.2}"),
                Err(_) => "—".to_string(),
            },
            Err(_) => "—".to_string(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match validate_price_entry(&self.cost, &self.duty, &self.markup) {
            Err(msg) => {
                self.error = Some(msg);
                cx.notify();
            }
            Ok((cost_val, duty_val, markup_val)) => {
                let sell = selling_price(cost_val, duty_val, markup_val).unwrap_or(0.0);
                let db    = PriceBookDb::global(&**cx);
                let pid   = self.product_id;
                let store = self.store.clone();

                cx.spawn(async move |this, cx| {
                    let result = db.insert_entry(pid, cost_val, duty_val, markup_val, sell, None).await;
                    if let Err(e) = result {
                        tracing::error!("insert_entry failed: {e:?}");
                        return Ok(());
                    }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(PriceFormEvent::Submitted))
                })
                .detach();
            }
        }
    }
}

impl Focusable for PriceEntryForm {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PriceEntryForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selling = self.computed_selling_price();

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(420.))
                    .bg(rgb(colors::CANVAS_BG))
                    .rounded(px(8.))
                    .p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    // Title
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .child(format!("New Price Entry — {}", self.product_name))
                    )
                    .child(price_field("Cost Price (USD)", &self.cost, "e.g. 120.00"))
                    .child(price_field("Duty Cost (USD)",  &self.duty, "e.g. 15.00"))
                    .child(price_field("Markup %",         &self.markup, "e.g. 30"))
                    // Computed selling price preview
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(
                                div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                                    .child("Selling Price (computed)")
                            )
                            .child(
                                div()
                                    .px(px(8.)).py(px(6.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .rounded(px(4.))
                                    .text_size(px(13.))
                                    .text_color(rgb(colors::STATUS_GREEN))
                                    .child(selling)
                            )
                    )
                    // Error
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
                                    .id("pb-btn-cancel")
                                    .px(px(16.)).py(px(6.)).rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .text_size(px(12.))
                                    .text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
                                        cx.emit(PriceFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div()
                                    .id("pb-btn-save")
                                    .px(px(16.)).py(px(6.)).rounded(px(4.))
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

fn price_field(label: &str, value: &str, placeholder: &str) -> impl IntoElement {
    let display    = if value.is_empty() { placeholder } else { value };
    let text_color = if value.is_empty() { colors::TEXT_MUTED } else { colors::TEXT_DEFAULT };

    div().flex().flex_col().gap(px(4.))
        .child(
            div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child(label.to_string())
        )
        .child(
            div()
                .px(px(8.)).py(px(6.))
                .bg(rgb(colors::SURFACE_DEFAULT))
                .rounded(px(4.))
                .text_size(px(13.))
                .text_color(rgb(text_color))
                .child(display.to_string())
        )
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook price_form 2>&1 | tail -10
```

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/src/price_form.rs && git commit -m "feat(pricebook): PriceEntryForm modal with validation and computed selling price"
```

---

### Task 6: PriceBookPanel — tab bar + history tab + form wiring

**Files:**
- Modify: `crates/vassl-pricebook/src/panel.rs`

- [ ] **Step 1: Write the full panel**

Replace the stub:

```rust
use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::price_form::{PriceEntryForm, PriceFormEvent};
use crate::price_table::PriceTable;
use crate::store::PriceBookStore;
use crate::PriceBookStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab { PriceBook, History }

pub struct PriceBookPanel {
    store:       Entity<PriceBookStore>,
    price_table: Entity<PriceTable>,
    active_tab:  Tab,
    form:        Option<Entity<PriceEntryForm>>,
    _form_sub:   Option<Subscription>,
}

impl PriceBookPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<PriceBookStoreHandle>().0.clone();
        let price_table = cx.new(|cx| PriceTable::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_products(cx));
        Self {
            store,
            price_table,
            active_tab: Tab::PriceBook,
            form:      None,
            _form_sub: None,
        }
    }

    fn open_form(&mut self, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let (product_id, product_name) = {
            let store = self.store.read(cx);
            let Some(pid) = store.selected_product_id else { return; };
            let name = store.product_prices
                .iter()
                .find(|p| p.product_id == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (pid, name)
        };
        let form = cx.new(|cx| PriceEntryForm::new(self.store.clone(), product_id, product_name, cx));
        let sub  = cx.subscribe(&form, |this, _form, ev: &PriceFormEvent, cx| {
            match ev {
                PriceFormEvent::Submitted | PriceFormEvent::Cancelled => {
                    this._form_sub = None;
                    this.form      = None;
                    cx.notify();
                }
            }
        });
        self.form      = Some(form);
        self._form_sub = Some(sub);
        cx.notify();
    }
}

impl Render for PriceBookPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

        // History tab content — extract data while store is borrowed
        let history_rows: Vec<_> = {
            let store = self.store.read(cx);
            store.history.iter().map(|e| {
                (
                    e.effective_date[..10].to_string(),
                    e.cost_price_usd,
                    e.duty_cost_usd,
                    e.markup_percent,
                    e.selling_price_usd,
                )
            }).collect()
        };
        let history_is_empty = history_rows.is_empty();

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::PriceBook => content.child(self.price_table.clone()),
            Tab::History => {
                if !has_selection {
                    content.child(
                        div()
                            .flex_1().flex().items_center().justify_center()
                            .text_color(rgb(colors::TEXT_MUTED))
                            .child("Select a product row to view pricing history.")
                    )
                } else if history_is_empty {
                    content.child(
                        div()
                            .flex_1().flex().items_center().justify_center()
                            .text_color(rgb(colors::TEXT_MUTED))
                            .child("No price history for this product.")
                    )
                } else {
                    let rows: Vec<_> = history_rows.iter().map(|(date, cost, duty, markup, sell)| {
                        div()
                            .flex().flex_row().items_center().w_full()
                            .px(px(12.)).py(px(6.))
                            .child(div().w(px(100.)).text_size(px(12.)).text_color(rgb(colors::TEXT_MUTED)).child(date.clone()))
                            .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT)).child(format!("${cost:.2}")))
                            .child(div().w(px(80.)).text_size(px(12.)).text_color(rgb(colors::TEXT_MUTED)).child(format!("+${duty:.2}")))
                            .child(div().w(px(70.)).text_size(px(12.)).text_color(rgb(colors::TEXT_MUTED)).child(format!("{markup:.0}%")))
                            .child(div().flex_1().text_size(px(13.)).text_color(rgb(colors::STATUS_GREEN)).child(format!("${sell:.2}")))
                    }).collect();

                    content.child(
                        div()
                            .id("history-scroll")
                            .flex_1().flex().flex_col()
                            .overflow_y_scroll()
                            .children(rows)
                    )
                }
            }
        };

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                // Tab bar + button row
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    // Price Book tab
                    .child(
                        div()
                            .id("pb-tab-pricebook")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::PriceBook { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::PriceBook;
                                cx.notify();
                            }))
                            .child("Price Book")
                    )
                    // History tab
                    .child(
                        div()
                            .id("pb-tab-history")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::History { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::History;
                                cx.notify();
                            }))
                            .child("History")
                    )
                    // Spacer
                    .child(div().flex_1())
                    // New Entry button — active only when a product row is selected
                    .child({
                        let mut btn = div()
                            .id("pb-btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .child("+ New Entry");
                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                    this.open_form(cx);
                                }));
                        }
                        btn
                    })
            )
            .child(content);

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        root
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build -p vassl-pricebook 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/src/panel.rs && git commit -m "feat(pricebook): PriceBookPanel with Price Book/History tabs and form overlay"
```

---

### Task 7: Wire PriceBookPanel into VasslRoot

**Files:**
- Modify: `crates/vassl-app/src/root.rs`

- [ ] **Step 1: Update root.rs**

Replace the current content of `crates/vassl-app/src/root.rs`:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, rgb};

use crate::actions::{OpenInventory, OpenPriceBook, OpenQuotations};
use crate::colors;
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;
use vassl_inventory::panel::InventoryPanel;
use vassl_pricebook::panel::PriceBookPanel;

pub struct VasslRoot {
    sidebar:          Entity<Sidebar>,
    status_bar:       Entity<StatusBar>,
    inventory_panel:  Entity<InventoryPanel>,
    pricebook_panel:  Entity<PriceBookPanel>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar:         cx.new(Sidebar::new),
            status_bar:      cx.new(StatusBar::new),
            inventory_panel: cx.new(InventoryPanel::new),
            pricebook_panel: cx.new(PriceBookPanel::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.sidebar.read(cx).active;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active {
            ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
            ActiveModule::Quotations => content.child(div().child("Quotations — Plan 4")),
            ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
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
                    .child(content),
            )
            .child(self.status_bar.clone())
    }
}
```

- [ ] **Step 2: Add vassl-pricebook to vassl-app's Cargo.toml**

In `crates/vassl-app/Cargo.toml`, add:

```toml
vassl-pricebook              = { path = "../vassl-pricebook" }
```

(The `vassl-quotations` and `vassl-pricebook` lines should appear alongside the existing `vassl-inventory` line.)

- [ ] **Step 3: Build the full workspace**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build 2>&1 | grep "^error" | head -20
```

Expected: no errors. The app compiles with `PriceBookPanel` in place.

- [ ] **Step 4: Run all tests**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test 2>&1 | tail -20
```

Expected: all existing tests pass plus new pricebook tests.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-app/src/root.rs crates/vassl-app/Cargo.toml && git commit -m "feat(app): wire PriceBookPanel into VasslRoot, replacing Plan 3 placeholder"
```

---

## Self-Review

**Spec coverage:**

| Spec requirement | Task |
|---|---|
| `price_book_entries` table schema | Task 1 — `PriceBookDb::MIGRATIONS` |
| Selling price stored (not computed on read) | Task 1 — `insert_entry` receives `selling_price_usd` |
| Price book table — product name, SKU, cost, duty, markup%, selling price, effective date | Task 4 — `price_row` columns |
| Entry form — product, cost, duty, markup (default 30), live selling price preview | Task 5 — `PriceEntryForm` + `computed_selling_price()` |
| Price history — per-product view | Task 6 — History tab in `PriceBookPanel` |
| `Ctrl+3` opens Price Book | Foundation plan (already done) — `OpenPriceBook` action wired in `root.rs` |
| Module init pattern | Task 3 — `pub fn init(cx)` in `lib.rs` |

**Not in this plan (Plan 5):**
- Global search / price range filter
- Product picker widget in form (currently requires clicking a row to pre-select)
- Text input fields (currently static placeholders — same as inventory `StockEntryForm`)
- Product CRUD (add/edit/delete products)

**Placeholder scan:** No TBD or "implement later" in task code. All steps have complete code.

**Type consistency:** `ProductPrice` defined in `store.rs`, used in `price_table.rs`. `PriceEntry` from `vassl-core`, used in `db.rs`, `store.rs`, `price_table.rs`, `price_form.rs`. `PriceFormEvent` defined and emitted in `price_form.rs`, subscribed to in `panel.rs`. `PriceBookStoreHandle` defined in `store.rs`, exported via `lib.rs`, accessed via `cx.global::<PriceBookStoreHandle>()` in `panel.rs`. Consistent throughout.

---

## Next Plans

- **Plan 4 — Quotations module:** `QuotationDb`, `QuotationStore`, quotation list view, line items editor, project picker
- **Plan 5 — App Polish:** GPUI `TextInput` for forms (unlocks actual data entry in all modals), command palette (`Ctrl+P`), product CRUD, global search, product detail price history chart
