# VASSL Internal Tools — Design Spec

**Date:** 2026-05-30  
**Company:** Video Access Security Solutions Limited  
**Stack:** Rust + GPUI (Zed framework)  
**Platform:** Windows (primary, VM deployment) + macOS (development/testing)  

---

## 1. Overview

A single desktop application (`vassl`) containing three internal business modules:

1. **Inventory** — track stock levels, acquisitions, restock alerts, price history
2. **Quotations** — manage internal project quotes for auditing
3. **Price Book** — maintain cost, duty, markup, and selling prices per product

Users: CEO and designated VASSL staff. Single-user at a time (SQLite, no concurrent writes).  
Currency: USD throughout.  
No AI, no OCR — all data entry is manual.

---

## 2. Application Architecture

### Binary

One executable: `vassl` / `vassl.exe`. Cross-compiled for:
- `x86_64-pc-windows-msvc` (primary deployment)
- `aarch64-apple-darwin` + `x86_64-apple-darwin` (development/testing)

### Crate Layout

```
vassl/
├── crates/
│   ├── vassl-app/        # GPUI workspace, window, sidebar, pane system, settings
│   ├── vassl-db/         # sqlez wrapper: AppDatabase global, static_connection! macro, open/migrate on startup
│   ├── vassl-inventory/  # Inventory views + business logic + owns its DomainMigration
│   ├── vassl-quotations/ # Quotations views + business logic + owns its DomainMigration
│   └── vassl-pricebook/  # Price book views + business logic + owns its DomainMigration
└── Cargo.toml            # workspace manifest
```

### Window Layout

```
┌─────────────────────────────────────────────────────┐
│  [≡] VASSL          [search...]           [⚙]       │  ← title bar
├──────┬──────────────────────────────────────────────┤
│  I   │                                              │
│  Q   │          Active Module Pane Group            │
│  P   │          (splittable — split right/down)     │
│      │                                              │
├──────┴──────────────────────────────────────────────┤
│  Audit log strip / status bar                       │
└─────────────────────────────────────────────────────┘
I = Inventory  |  Q = Quotations  |  P = Price Book
```

### Data Storage

- Single `vassl.db` SQLite file
- Location: `%APPDATA%\vassl\vassl.db` (Windows), `~/Library/Application Support/vassl/vassl.db` (macOS)
- Database layer uses **sqlez** (Zed's in-house SQLite wrapper) — copied directly from `crates/sqlez/` and `crates/db/` in the Zed repo
- **Migration strategy — `DomainMigration` + `inventory::collect!` pattern (mirror of Zed's `db` crate):**
  - `vassl-db` defines `struct DomainMigration { name, migrations: &[&str], dependencies: &[&str] }` and calls `inventory::collect!(DomainMigration)`
  - Each module crate (`vassl-inventory`, `vassl-quotations`, `vassl-pricebook`) declares its own migrations via `inventory::submit!` — no central migration file to update when adding modules
  - On startup, `AppDatabase::new()` opens one `ThreadSafeConnection`, topologically sorts all collected `DomainMigration`s by dependency, and applies them in order
  - Each module also calls `static_connection!(InventoryDb, ["vassl-inventory"])` to get a typed DB handle scoped to its domain
- Pane layout and user preferences stored in `vassl.db` via the shared `settings` domain

---

## 3. Data Model

### Shared

```sql
settings (
  key   TEXT PRIMARY KEY,   -- e.g. "current_user", "theme", "pane_layout"
  value TEXT NOT NULL
)
-- current_user is set on first launch (a simple name field, no passwords)
-- All audit_log.changed_by and quotations.created_by values read from settings("current_user")

projects (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  client_name   TEXT NOT NULL,
  description   TEXT,
  status        TEXT NOT NULL DEFAULT 'active',  -- active | completed | archived
  created_at    TEXT NOT NULL
)

audit_log (
  id            INTEGER PRIMARY KEY,
  table_name    TEXT NOT NULL,
  record_id     INTEGER NOT NULL,
  action        TEXT NOT NULL,   -- create | update | delete
  changed_by    TEXT NOT NULL,
  changed_at    TEXT NOT NULL,
  old_value     TEXT,            -- JSON blob
  new_value     TEXT             -- JSON blob
)
```

### Inventory

```sql
products (
  id              INTEGER PRIMARY KEY,
  sku             TEXT UNIQUE NOT NULL,
  name            TEXT NOT NULL,
  category        TEXT,
  unit            TEXT NOT NULL,   -- e.g. "pcs", "meters", "box"
  min_stock_level REAL NOT NULL DEFAULT 0,
  notes           TEXT,
  created_at      TEXT NOT NULL
)

stock_entries (
  id               INTEGER PRIMARY KEY,
  product_id       INTEGER NOT NULL REFERENCES products(id),
  quantity         REAL NOT NULL,
  unit_cost_usd    REAL NOT NULL,
  supplier         TEXT,
  acquired_at      TEXT NOT NULL,
  acquisition_type TEXT NOT NULL,  -- project | restock
  project_id       INTEGER REFERENCES projects(id),
  invoice_ref      TEXT,
  notes            TEXT
)
```

Current stock quantity = `SUM(quantity)` across all `stock_entries` for a product.  
`quantity` is always positive — `stock_entries` records incoming stock only (no consumption/disposal tracking in v1).  
Price history = `stock_entries` ordered by `acquired_at`.

### Quotations

```sql
quotations (
  id               INTEGER PRIMARY KEY,
  project_id       INTEGER NOT NULL REFERENCES projects(id),
  reference_number TEXT UNIQUE NOT NULL,
  status           TEXT NOT NULL DEFAULT 'draft',  -- draft | sent | accepted | rejected
  notes            TEXT,
  created_by       TEXT NOT NULL,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
)

quotation_items (
  id             INTEGER PRIMARY KEY,
  quotation_id   INTEGER NOT NULL REFERENCES quotations(id),
  product_id     INTEGER REFERENCES products(id),  -- nullable for non-inventory items
  description    TEXT NOT NULL,
  quantity       REAL NOT NULL,
  unit_price_usd REAL NOT NULL,
  total_usd      REAL NOT NULL   -- quantity * unit_price_usd
)
```

### Price Book

```sql
price_book_entries (
  id               INTEGER PRIMARY KEY,
  product_id       INTEGER NOT NULL REFERENCES products(id),
  cost_price_usd   REAL NOT NULL,
  duty_cost_usd    REAL NOT NULL DEFAULT 0,
  markup_percent   REAL NOT NULL DEFAULT 30,
  selling_price_usd REAL NOT NULL,  -- stored: (cost + duty) * (1 + markup/100)
  effective_date   TEXT NOT NULL,
  notes            TEXT
)
```

Selling price is stored (not computed on read) to preserve historical snapshots when markup changes.

---

## 4. Module Features

### 4.1 Inventory

- **Stock list** — table: SKU, name, category, current qty, min level, unit cost, stock status badge (red = below min, amber = within 20% of min, green = healthy)
- **Stock entry form** — modal: product picker, quantity, unit cost, supplier, invoice ref, acquisition type, project picker (if project)
- **Product detail pane** — splittable alongside list: unit cost history line chart, full acquisition history table
- **Restock alerts panel** — filterable list of products at or below `min_stock_level`
- **Global search** — fuzzy: SKU, name, category, supplier, project name

### 4.2 Quotations

- **Quotation list** — table: reference number, project + client, status badge, date, total value (USD)
- **Quotation detail** — line items editor (add/remove/edit rows), project picker, status transitions, notes field
- **Project sidebar tree** — groups quotes by project for quick navigation
- **Global search** — reference number, project name, client name, item description

### 4.3 Price Book

- **Price book table** — product name, SKU, cost price, duty, markup %, selling price, effective date
- **Entry form** — modal: product picker, cost price, duty cost, markup % (default 30, editable), live-computed selling price preview
- **Price history** — per-product view showing selling price over time (effective dates)
- **Global search** — product name, SKU, price range filter

### 4.4 Audit Log

- **Status bar strip** — last action summary always visible (e.g., "Stock entry added — 5× IP Camera, $120.00, 2 mins ago")
- **Full audit log** (`Ctrl+Shift+A`) — filterable by module/table, action type, date range; shows old/new values side by side

---

## 5. UI & Interaction Design

### Theme

- Dark by default; light mode toggle stored in settings
- Accent color: `#1a3c5e` (deep navy) — VASSL brand, used for active sidebar items, focus rings, status badges

### Sidebar

- Fixed ~48px icon rail on the left
- Three module icons: Inventory (box), Quotations (document), Price Book (tag)
- Active module highlighted with accent color
- Bottom: Settings gear icon

### Pane Splitting

- Right-click any pane → "Split Right" / "Split Down"
- Each split holds an independent view of the active module
- Layout persisted in `vassl.db` between sessions

### Command Palette

`Ctrl+P` — fuzzy search scoped to active module:
- "New Stock Entry", "New Quotation", "New Price Book Entry"
- "Go to Product: [name]", "Filter by Project: [name]"

### Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+1` | Switch to Inventory |
| `Ctrl+2` | Switch to Quotations |
| `Ctrl+3` | Switch to Price Book |
| `Ctrl+P` | Command palette |
| `Ctrl+Shift+A` | Open full audit log |
| `Ctrl+N` | New record (context-aware) |
| `Ctrl+F` | Focus search bar |
| `Escape` | Close modal / clear search |

### Forms

Modal overlays (not separate OS windows) for all create/edit operations. Consistent layout across all three modules: title, fields, cancel/save buttons.

---

## 6. Error Handling & Edge Cases

- SQLite write failures shown as inline error toast (non-blocking)
- Deleting a product blocked if referenced by stock entries, quotation items, or price book entries — show a blocking modal listing dependents
- Minimum stock level of 0 means no alert (effectively disabled for that product)
- Markup percent must be > 0; cost price must be ≥ 0; duty cost must be ≥ 0
- Reference numbers for quotations auto-generated as `VASSL-YYYY-NNNN` but editable

---

## 7. Database Layer (sqlez)

### Source
Copy `crates/sqlez/` and `crates/db/` verbatim from the Zed repo into `crates/vassl-db/`. Rename the crate to `vassl-db`. The key files are: `connection.rs`, `thread_safe_connection.rs`, `migrations.rs`, `domain.rs`, `statement.rs`, `typed_statements.rs`, `bindable.rs`.

### AppDatabase global (in `vassl-db/src/lib.rs`)
```rust
pub struct AppDatabase { connection: ThreadSafeConnection }
impl Global for AppDatabase {}

impl AppDatabase {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let connection = ThreadSafeConnection::new(db_path, true)?;
        connection.with_conn(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;"
            )
        })?;
        AppMigrator::migrate(&connection)?;  // topological sort + apply all DomainMigrations
        Ok(Self { connection })
    }
}
```

### DomainMigration registration pattern
```rust
// In vassl-db/src/lib.rs — the collector
pub struct DomainMigration {
    pub name: &'static str,
    pub migrations: &'static [&'static str],
    pub dependencies: &'static [&'static str],
}
inventory::collect!(DomainMigration);
```

```rust
// In vassl-inventory/src/db.rs — each module submits its own migrations
inventory::submit! {
    DomainMigration {
        name: "inventory",
        dependencies: &["shared"],
        migrations: &[
            sql!("CREATE TABLE IF NOT EXISTS products (
                id              INTEGER PRIMARY KEY,
                sku             TEXT UNIQUE NOT NULL,
                name            TEXT NOT NULL,
                category        TEXT,
                unit            TEXT NOT NULL,
                min_stock_level REAL NOT NULL DEFAULT 0,
                notes           TEXT,
                created_at      TEXT NOT NULL
            )"),
            sql!("CREATE TABLE IF NOT EXISTS stock_entries (...)"),
        ],
    }
}
```

### Typed DB handle per module
```rust
// In vassl-inventory/src/db.rs
static_connection!(InventoryDb, ["inventory"]);

impl InventoryDb {
    pub fn global(cx: &App) -> Self {
        cx.global::<AppDatabase>().for_domain()
    }

    pub fn list_products(&self) -> anyhow::Result<Vec<Product>> {
        self.select(sql!("SELECT id, sku, name, category, unit, min_stock_level, notes, created_at
                          FROM products ORDER BY name"))
    }

    pub fn insert_stock_entry(&self, entry: &NewStockEntry) -> anyhow::Result<()> {
        self.exec_bound(sql!("INSERT INTO stock_entries (...) VALUES (...)"))?(entry)
    }
}
```

### Async DB call pattern (from a GPUI view)
```rust
cx.spawn(async move |this, cx| {
    let products = cx.background_spawn(async move {
        InventoryDb::global(&*cx).list_products()
    }).await?;
    this.update(cx, |panel, cx| {
        panel.products = products;
        cx.notify();   // required — triggers re-render
    })
}).detach();
```

### Write + audit log helper
```rust
pub fn write_and_log<F>(cx: &App, write: impl FnOnce() -> F + Send + 'static)
where F: std::future::Future<Output = anyhow::Result<()>> + Send,
{
    cx.background_spawn(async move { write().await.log_err() }).detach()
}
```

Every mutation (insert/update/delete) that goes through `write_and_log` must also call `AuditDb::log(table, record_id, action, old_json, new_json)` inside the same write closure.

---

## 8. Build & Release

- `cargo build --release --target x86_64-pc-windows-msvc` for VM deployment
- Single binary, no installer required — copy `vassl.exe` to client machine
- Database created automatically on first run
- macOS: universal binary built via `cargo-lipo` or separate targets merged with `lipo`
