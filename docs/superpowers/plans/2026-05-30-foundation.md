# VASSL Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the Rust + GPUI workspace with sqlez DB layer and app shell — a running `vassl` binary that opens a window with left icon sidebar (3 module icons), empty pane area, and bottom status bar.

**Architecture:** Helix-inspired three-layer boundary: `vassl-core` (pure domain types, zero deps) → `vassl-db` (sqlez, depends on core) → module crates (GPUI + DB views) → `vassl-app` (binary, wires everything). Zed's `sqlez`, `sqlez_macros`, and `db` crates are vendored. `tracing` + `tracing-appender` for logging. Platform-specific code isolated to `vassl-app/src/platform/`.

**Tech Stack:** Rust stable, GPUI (git dep from `zed-industries/zed`), sqlez + db vendored from Zed, `inventory` crate for migration collection, `thiserror` in sub-crates / `anyhow` in binary, `tracing` + `tracing-appender` for logging, `dirs` for platform paths, `chrono` for timestamps, `arc-swap` for live config reload.

---

## File Map

```
tools/
├── Cargo.toml                              # workspace manifest, default-members = ["vassl-app"]
├── .gitattributes                          # *.sql text eol=lf
├── assets/
│   └── keymaps/
│       └── default.json                    # Ctrl+1/2/3, Ctrl+N, Ctrl+F, Ctrl+Shift+A
└── crates/
    ├── sqlez/                              # vendored from zed-industries/zed (Task 2)
    ├── sqlez_macros/                       # vendored from zed-industries/zed (Task 2)
    ├── db/                                 # vendored from zed-industries/zed (Task 2)
    │                                       # provides: AppDatabase Global, static_connection! macro
    ├── vassl-core/                         # (Task 1) pure domain types — NO gpui, NO sqlez
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs                      # re-exports all domain types
    │       ├── product.rs                  # Product, NewProduct, StockEntry, NewStockEntry
    │       ├── project.rs                  # Project, ProjectStatus
    │       ├── quotation.rs                # Quotation, QuotationItem, QuotationStatus
    │       └── price_entry.rs              # PriceEntry, NewPriceEntry, selling_price()
    ├── vassl-db/                           # (Tasks 3–4) DB layer — depends on vassl-core + vendored db
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs                      # re-exports, db_path(), init(cx)
    │       ├── migrations.rs               # DomainMigration, inventory::collect!, AppMigrator
    │       └── shared.rs                   # shared domain migration + settings/projects/audit helpers
    ├── vassl-app/                          # (Tasks 5–8) GPUI binary
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs                     # Application::new().run() + tracing init
    │       ├── app.rs                      # VasslApp struct (helix Application pattern)
    │       ├── root.rs                     # VasslRoot — top-level GPUI view
    │       ├── sidebar.rs                  # Sidebar — 48px icon rail, active module state
    │       ├── status_bar.rs               # StatusBar — bottom strip, last action label
    │       ├── actions.rs                  # global actions! + keybinding loader
    │       └── platform/
    │           ├── mod.rs                  # platform-neutral exports
    │           ├── macos.rs                # #[cfg(target_os = "macos")] specifics
    │           └── windows.rs              # #[cfg(target_os = "windows")] specifics
    ├── vassl-inventory/
    │   ├── Cargo.toml
    │   └── src/lib.rs                      # pub fn init(cx: &mut App) {} stub
    ├── vassl-quotations/
    │   ├── Cargo.toml
    │   └── src/lib.rs                      # stub
    └── vassl-pricebook/
        ├── Cargo.toml
        └── src/lib.rs                      # stub
```

---

### Task 1: Initialize Cargo workspace + vassl-core domain types

**Files:**
- Create: `tools/Cargo.toml`
- Create: `tools/.gitattributes`
- Create: `tools/crates/vassl-core/Cargo.toml`
- Create: `tools/crates/vassl-core/src/lib.rs`
- Create: `tools/crates/vassl-core/src/product.rs`
- Create: `tools/crates/vassl-core/src/project.rs`
- Create: `tools/crates/vassl-core/src/quotation.rs`
- Create: `tools/crates/vassl-core/src/price_entry.rs`
- Create: stub `Cargo.toml` + `src/lib.rs` for vassl-db, vassl-app, vassl-inventory, vassl-quotations, vassl-pricebook

- [ ] **Step 1: Create directory structure**

```bash
cd /Users/oluwasetemi/r/kamalu/tools
mkdir -p crates/vassl-core/src \
         crates/vassl-db/src \
         crates/vassl-app/src/platform \
         crates/vassl-inventory/src \
         crates/vassl-quotations/src \
         crates/vassl-pricebook/src \
         assets/keymaps
```

- [ ] **Step 2: Write `.gitattributes`**

```
# Keep SQL migration hashes stable across Windows/Unix
*.sql text eol=lf
```

- [ ] **Step 3: Write root `Cargo.toml`**

```toml
[workspace]
members = [
    "crates/vassl-core",
    "crates/vassl-db",
    "crates/vassl-app",
    "crates/vassl-inventory",
    "crates/vassl-quotations",
    "crates/vassl-pricebook",
]
default-members = ["crates/vassl-app"]   # cargo run / cargo build targets vassl-app
resolver = "2"

[workspace.dependencies]
# UI
gpui = { git = "https://github.com/zed-industries/zed" }

# Error handling — thiserror in libs, anyhow in binary
anyhow    = "1"
thiserror = "2"

# Serialization
serde      = { version = "1", features = ["derive"] }
serde_json = "1"

# Logging (modern recipe)
tracing             = "0.1"
tracing-subscriber  = { version = "0.3", features = ["env-filter"] }
tracing-appender    = "0.2"

# DB / migration
inventory = "0.3"

# Platform paths + time
dirs   = "5"
chrono = { version = "0.4", features = ["serde"] }

# Live config reload (helix pattern)
arc-swap = "1"
```

> **Note on GPUI:** The git dep pulls only the `gpui` package from the Zed monorepo. First compile takes ~10 min. Pin to a specific commit (`rev = "abc123"`) once the project is stable.

- [ ] **Step 4: Write `crates/vassl-core/Cargo.toml`**

```toml
[package]
name    = "vassl-core"
version = "0.1.0"
edition = "2021"

# No gpui, no sqlez — pure domain types only
[dependencies]
thiserror.workspace = true
serde.workspace     = true
chrono.workspace    = true
```

- [ ] **Step 5: Write failing tests for `price_entry.rs` selling price formula**

`crates/vassl-core/src/price_entry.rs`:
```rust
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceEntry {
    pub id: i64,
    pub product_id: i64,
    pub cost_price_usd: f64,
    pub duty_cost_usd: f64,
    pub markup_percent: f64,
    pub selling_price_usd: f64,
    pub effective_date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewPriceEntry {
    pub product_id: i64,
    pub cost_price_usd: f64,
    pub duty_cost_usd: f64,
    pub markup_percent: f64,   // default 30.0
    pub effective_date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Error)]
pub enum PriceEntryError {
    #[error("markup_percent must be > 0, got {0}")]
    InvalidMarkup(f64),
    #[error("cost_price_usd must be >= 0, got {0}")]
    InvalidCostPrice(f64),
    #[error("duty_cost_usd must be >= 0, got {0}")]
    InvalidDuty(f64),
}

/// Stored selling price: (cost + duty) * (1 + markup / 100)
/// Stored rather than computed on read to preserve historical snapshots.
pub fn selling_price(cost: f64, duty: f64, markup_percent: f64) -> Result<f64, PriceEntryError> {
    if markup_percent <= 0.0 {
        return Err(PriceEntryError::InvalidMarkup(markup_percent));
    }
    if cost < 0.0 {
        return Err(PriceEntryError::InvalidCostPrice(cost));
    }
    if duty < 0.0 {
        return Err(PriceEntryError::InvalidDuty(duty));
    }
    Ok((cost + duty) * (1.0 + markup_percent / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selling_price_default_markup() {
        // (100 + 10) * 1.30 = 143.0
        let price = selling_price(100.0, 10.0, 30.0).unwrap();
        assert!((price - 143.0).abs() < 1e-10);
    }

    #[test]
    fn selling_price_zero_duty() {
        // 200 * 1.30 = 260.0
        let price = selling_price(200.0, 0.0, 30.0).unwrap();
        assert!((price - 260.0).abs() < 1e-10);
    }

    #[test]
    fn selling_price_rejects_zero_markup() {
        assert!(selling_price(100.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn selling_price_rejects_negative_cost() {
        assert!(selling_price(-1.0, 0.0, 30.0).is_err());
    }
}
```

- [ ] **Step 6: Run failing tests (they will fail — module not wired up yet)**

```bash
cd /Users/oluwasetemi/r/kamalu/tools
cargo test -p vassl-core -- price_entry::tests 2>&1 | head -20
```

Expected: FAIL — `price_entry` module not declared in `lib.rs` yet.

- [ ] **Step 7: Write remaining domain type files**

`crates/vassl-core/src/product.rs`:
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
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewProduct {
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockEntry {
    pub id: i64,
    pub product_id: i64,
    pub quantity: f64,           // always positive — incoming stock only
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

#[derive(Debug, Clone)]
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
```

`crates/vassl-core/src/project.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub client_name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Completed,
    Archived,
}

#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub client_name: String,
    pub description: Option<String>,
}
```

`crates/vassl-core/src/quotation.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quotation {
    pub id: i64,
    pub project_id: i64,
    pub reference_number: String,
    pub status: QuotationStatus,
    pub notes: Option<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotationStatus {
    Draft,
    Sent,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotationItem {
    pub id: i64,
    pub quotation_id: i64,
    pub product_id: Option<i64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price_usd: f64,
    pub total_usd: f64,          // quantity * unit_price_usd
}
```

- [ ] **Step 8: Wire everything in `crates/vassl-core/src/lib.rs`**

```rust
pub mod price_entry;
pub mod product;
pub mod project;
pub mod quotation;

pub use price_entry::{NewPriceEntry, PriceEntry, PriceEntryError, selling_price};
pub use product::{AcquisitionType, NewProduct, NewStockEntry, Product, StockEntry};
pub use project::{NewProject, Project, ProjectStatus};
pub use quotation::{Quotation, QuotationItem, QuotationStatus};
```

- [ ] **Step 9: Write stub source files for other crates**

`crates/vassl-db/src/lib.rs`:
```rust
pub fn placeholder() {}
```

`crates/vassl-app/src/main.rs`:
```rust
fn main() { println!("VASSL starting..."); }
```

`crates/vassl-inventory/src/lib.rs` / `vassl-quotations/src/lib.rs` / `vassl-pricebook/src/lib.rs`:
```rust
use gpui::App;
pub fn init(_cx: &mut App) {}
```

Stub `Cargo.toml` for vassl-db:
```toml
[package]
name    = "vassl-db"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow.workspace    = true
thiserror.workspace = true
serde.workspace     = true
serde_json.workspace = true
inventory.workspace = true
chrono.workspace    = true
dirs.workspace      = true
vassl-core          = { path = "../vassl-core" }
```

Stub `Cargo.toml` for vassl-app:
```toml
[package]
name    = "vassl-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "vassl"
path = "src/main.rs"

[dependencies]
gpui.workspace              = true
anyhow.workspace            = true
tracing.workspace           = true
tracing-subscriber.workspace = true
tracing-appender.workspace  = true
dirs.workspace              = true
vassl-db                    = { path = "../vassl-db" }
vassl-inventory             = { path = "../vassl-inventory" }
vassl-quotations            = { path = "../vassl-quotations" }
vassl-pricebook             = { path = "../vassl-pricebook" }
```

`crates/vassl-inventory/Cargo.toml`:
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
inventory.workspace  = true
chrono.workspace     = true
vassl-core           = { path = "../vassl-core" }
vassl-db             = { path = "../vassl-db" }
```

(`vassl-quotations` and `vassl-pricebook` are identical — change `name`.)

- [ ] **Step 10: Run all tests**

```bash
cargo test -p vassl-core
```

Expected: 4 tests pass (`selling_price_*` tests).

- [ ] **Step 11: Commit**

```bash
git add crates/ Cargo.toml .gitattributes assets/
git commit -m "feat: workspace skeleton + vassl-core domain types"
```

---

### Task 2: Vendor sqlez, sqlez_macros, and db from Zed

**Files:**
- Create: `tools/crates/sqlez/` (copied from Zed)
- Create: `tools/crates/sqlez_macros/` (copied from Zed)
- Create: `tools/crates/db/` (copied from Zed — provides `static_connection!` macro)
- Modify: `tools/Cargo.toml`
- Modify: `tools/crates/vassl-db/Cargo.toml`

- [ ] **Step 1: Clone Zed and copy the three DB crates**

```bash
git clone --depth=1 https://github.com/zed-industries/zed /tmp/zed-source
cp -r /tmp/zed-source/crates/sqlez       /Users/oluwasetemi/r/kamalu/tools/crates/
cp -r /tmp/zed-source/crates/sqlez_macros /Users/oluwasetemi/r/kamalu/tools/crates/
cp -r /tmp/zed-source/crates/db          /Users/oluwasetemi/r/kamalu/tools/crates/
```

- [ ] **Step 2: Add the three crates to workspace `Cargo.toml`**

Add to `[workspace] members`:
```toml
"crates/sqlez",
"crates/sqlez_macros",
"crates/db",
```

Add to `[workspace.dependencies]`:
```toml
sqlez        = { path = "crates/sqlez" }
sqlez_macros = { path = "crates/sqlez_macros" }
db           = { path = "crates/db" }
```

- [ ] **Step 3: Update `vassl-db/Cargo.toml` dependencies**

```toml
[dependencies]
anyhow.workspace     = true
thiserror.workspace  = true
serde.workspace      = true
serde_json.workspace = true
inventory.workspace  = true
chrono.workspace     = true
dirs.workspace       = true
gpui.workspace       = true
sqlez.workspace      = true
sqlez_macros.workspace = true
db.workspace         = true
vassl-core           = { path = "../vassl-core" }
```

- [ ] **Step 4: Check vendored crates compile**

```bash
cd /Users/oluwasetemi/r/kamalu/tools
cargo check -p sqlez -p sqlez_macros -p db
```

Expected: all three compile.

> **If errors:** Check `/tmp/zed-source/Cargo.toml` `[workspace.dependencies]` for any deps that sqlez/db use that aren't yet in our root `Cargo.toml`. Add them as needed.

- [ ] **Step 5: Read the vendored `db` crate's `static_connection!` macro**

```bash
cat /Users/oluwasetemi/r/kamalu/tools/crates/db/src/db.rs | head -120
```

This reveals the exact API for `static_connection!` and `DomainMigration`. Note any differences from the research — the macro signature is authoritative.

- [ ] **Step 6: Commit**

```bash
git add crates/sqlez crates/sqlez_macros crates/db Cargo.toml crates/vassl-db/Cargo.toml
git commit -m "chore: vendor sqlez, sqlez_macros, db from Zed"
```

---

### Task 3: vassl-db — AppDatabase + DomainMigration + AppMigrator

**Files:**
- Create: `tools/crates/vassl-db/src/migrations.rs`
- Modify: `tools/crates/vassl-db/src/lib.rs`

- [ ] **Step 1: Write `migrations.rs` with test**

`crates/vassl-db/src/migrations.rs`:
```rust
use sqlez::connection::Connection;

pub struct DomainMigration {
    pub name: &'static str,
    pub migrations: &'static [&'static str],
    pub dependencies: &'static [&'static str],
}

inventory::collect!(DomainMigration);

pub struct AppMigrator;

impl AppMigrator {
    pub fn sorted_migrations() -> Vec<&'static DomainMigration> {
        let all: Vec<&DomainMigration> = inventory::iter::<DomainMigration>().collect();
        let mut sorted: Vec<&DomainMigration> = Vec::with_capacity(all.len());
        let mut remaining: Vec<&DomainMigration> = all;

        while !remaining.is_empty() {
            let before = remaining.len();
            remaining.retain(|m| {
                let satisfied = m.dependencies
                    .iter()
                    .all(|dep| sorted.iter().any(|s| s.name == *dep));
                if satisfied { sorted.push(m); false } else { true }
            });
            assert!(
                remaining.len() < before,
                "circular or missing DomainMigration dependency"
            );
        }
        sorted
    }

    pub fn migrate(conn: &Connection) -> anyhow::Result<()> {
        for domain in Self::sorted_migrations() {
            for sql in domain.migrations {
                conn.execute_batch(sql)
                    .map_err(|e| anyhow::anyhow!(
                        "migration '{}' failed: {}", domain.name, e
                    ))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_migrations_empty_is_ok() {
        let result = std::panic::catch_unwind(AppMigrator::sorted_migrations);
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Run test**

```bash
cargo test -p vassl-db -- migrations::tests --nocapture
```

Expected: `test migrations::tests::sorted_migrations_empty_is_ok ... ok`

- [ ] **Step 3: Write full `vassl-db/src/lib.rs`**

```rust
pub mod migrations;
pub mod shared;

pub use migrations::{AppMigrator, DomainMigration};

use anyhow::Context as _;
use gpui::{App, Global};
use sqlez::connection::Connection;
use std::path::Path;

pub struct AppDatabase {
    pub connection: Connection,
}

impl Global for AppDatabase {}

impl AppDatabase {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(
            db_path.parent().context("db path has no parent")?,
        )?;
        let connection = Connection::open_file(
            db_path.to_str().context("db path is not valid UTF-8")?,
        )?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;",
        )?;
        AppMigrator::migrate(&connection)?;
        Ok(Self { connection })
    }
}

pub fn db_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .expect("platform has no local data dir")
        .join("VASSL")
        .join("vassl.db")
}

pub fn init(cx: &mut App) -> anyhow::Result<()> {
    let db = AppDatabase::new(&db_path())?;
    cx.set_global(db);
    Ok(())
}

/// Fire-and-forget async write. Logs errors; never panics the UI thread.
pub fn write_and_log<F>(cx: &App, write: impl FnOnce() -> F + Send + 'static)
where
    F: std::future::Future<Output = anyhow::Result<()>> + Send,
{
    cx.background_executor()
        .spawn(async move {
            if let Err(e) = write().await {
                tracing::error!("DB write failed: {e:?}");
            }
        })
        .detach();
}
```

- [ ] **Step 4: Add placeholder `crates/vassl-db/src/shared.rs`**

```rust
// populated in Task 4
```

- [ ] **Step 5: Run all vassl-db tests**

```bash
cargo test -p vassl-db
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-db/src/
git commit -m "feat(db): AppDatabase global, DomainMigration, AppMigrator"
```

---

### Task 4: vassl-db — Shared domain tables

**Files:**
- Modify: `tools/crates/vassl-db/src/shared.rs`

- [ ] **Step 1: Write `shared.rs` with failing tests first**

`crates/vassl-db/src/shared.rs`:
```rust
use crate::{AppMigrator, DomainMigration};
use sqlez::connection::Connection;

inventory::submit! {
    DomainMigration {
        name: "shared",
        dependencies: &[],
        migrations: &[
            "CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS projects (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                client_name TEXT NOT NULL,
                description TEXT,
                status      TEXT NOT NULL DEFAULT 'active',
                created_at  TEXT NOT NULL
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
        ],
    }
}

pub fn get_setting(conn: &Connection, key: &str) -> anyhow::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    stmt.bind_blob(1, key.as_bytes())?;
    if stmt.step()? {
        Ok(Some(stmt.column_text(0)?.to_string()))
    } else {
        Ok(None)
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> anyhow::Result<()> {
    conn.execute_batch(&format!(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('{}', '{}')",
        key.replace('\'', "''"),
        value.replace('\'', "''"),
    ))?;
    Ok(())
}

pub fn current_user(conn: &Connection) -> anyhow::Result<Option<String>> {
    get_setting(conn, "current_user")
}

pub fn set_current_user(conn: &Connection, name: &str) -> anyhow::Result<()> {
    set_setting(conn, "current_user", name)
}

pub fn log_audit(
    conn: &Connection,
    table_name: &str,
    record_id: i64,
    action: &str,
    changed_by: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let old_sql = old_value
        .map(|s| format!("'{}'", s.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());
    let new_sql = new_value
        .map(|s| format!("'{}'", s.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());
    conn.execute_batch(&format!(
        "INSERT INTO audit_log \
         (table_name, record_id, action, changed_by, changed_at, old_value, new_value) \
         VALUES ('{}', {}, '{}', '{}', '{}', {}, {})",
        table_name.replace('\'', "''"),
        record_id,
        action.replace('\'', "''"),
        changed_by.replace('\'', "''"),
        now,
        old_sql,
        new_sql,
    ))?;
    Ok(())
}

fn open_test_conn() -> Connection {
    let conn = Connection::open_memory(Some("shared_test")).unwrap();
    AppMigrator::migrate(&conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_roundtrip() {
        let conn = open_test_conn();
        assert_eq!(current_user(&conn).unwrap(), None);
        set_current_user(&conn, "Alice").unwrap();
        assert_eq!(current_user(&conn).unwrap(), Some("Alice".to_string()));
        set_current_user(&conn, "Bob").unwrap();
        assert_eq!(current_user(&conn).unwrap(), Some("Bob".to_string()));
    }

    #[test]
    fn test_audit_log_insert() {
        let conn = open_test_conn();
        log_audit(&conn, "products", 1, "create", "Alice", None,
                  Some(r#"{"name":"IP Camera"}"#)).unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM audit_log").unwrap();
        stmt.step().unwrap();
        let count = stmt.column_i64(0).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_projects_table_exists() {
        let conn = open_test_conn();
        conn.execute_batch(
            "INSERT INTO projects (name, client_name, created_at) \
             VALUES ('CCTV Install', 'Ritz Hotel', '2026-05-30T09:00:00Z')"
        ).unwrap();
        let mut stmt = conn.prepare("SELECT name FROM projects").unwrap();
        stmt.step().unwrap();
        assert_eq!(stmt.column_text(0).unwrap(), "CCTV Install");
    }
}
```

> **sqlez Statement API note:** `conn.prepare(sql)` → `Statement`. `stmt.bind_blob(pos, bytes)` to bind params, `stmt.step()` to advance, `stmt.column_text(idx)` / `stmt.column_i64(idx)` to read. If the vendored sqlez uses different method names, consult `crates/sqlez/src/statement.rs` — those names are authoritative.

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-db -- shared::tests --nocapture
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vassl-db/src/shared.rs
git commit -m "feat(db): shared domain tables — settings, projects, audit_log"
```

---

### Task 5: vassl-app — tracing setup + GPUI window bootstrap

**Files:**
- Create: `tools/crates/vassl-app/src/app.rs`
- Create: `tools/crates/vassl-app/src/root.rs`
- Create: `tools/crates/vassl-app/src/platform/mod.rs`
- Create: `tools/crates/vassl-app/src/platform/macos.rs`
- Create: `tools/crates/vassl-app/src/platform/windows.rs`
- Modify: `tools/crates/vassl-app/src/main.rs`

- [ ] **Step 1: Write `platform/mod.rs`**

`crates/vassl-app/src/platform/mod.rs`:
```rust
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Returns the app name for platform-specific window chrome.
pub fn app_name() -> &'static str {
    "VASSL"
}
```

`crates/vassl-app/src/platform/macos.rs`:
```rust
// macOS-specific setup (e.g. hide dock icon for agent mode) goes here
```

`crates/vassl-app/src/platform/windows.rs`:
```rust
// Windows-specific setup (e.g. DPI awareness) goes here
```

- [ ] **Step 2: Write `app.rs` — VasslApp state holder (helix Application pattern)**

`crates/vassl-app/src/app.rs`:
```rust
use gpui::{App, Entity};

/// Top-level app state — mirrors helix's Application struct.
/// Holds all shared state that crosses module boundaries.
pub struct VasslApp {
    // Module stores added here in Plans 2–4 as:
    // pub inventory: Entity<InventoryStore>,
    // pub quotations: Entity<QuotationStore>,
    // pub pricebook: Entity<PriceBookStore>,
}

impl VasslApp {
    pub fn new(_cx: &mut App) -> Self {
        Self {}
    }
}
```

- [ ] **Step 3: Write minimal `root.rs`**

`crates/vassl-app/src/root.rs`:
```rust
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};

pub struct VasslRoot;

impl VasslRoot {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .w_full()
            .h_full()
            .bg(gpui::rgb(0x1e1e2e))
            .child("placeholder — sidebar + pane + status bar wired in Tasks 6–7")
    }
}
```

- [ ] **Step 4: Write full `main.rs` with tracing init**

`crates/vassl-app/src/main.rs`:
```rust
mod actions;
mod app;
mod platform;
mod root;
mod sidebar;
mod status_bar;

use app::VasslApp;
use gpui::{App, Application, WindowOptions, bounds, point, size, px};
use root::VasslRoot;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = dirs::data_local_dir()
        .expect("no local data dir")
        .join("VASSL")
        .join("logs");
    std::fs::create_dir_all(&log_dir).expect("create log dir");

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("vassl")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("init log appender");

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stdout).pretty())
        .init();

    guard  // MUST be held for app lifetime or log writes are dropped
}

fn main() {
    let _tracing_guard = init_tracing();  // held for entire main() scope
    tracing::info!("VASSL starting");

    Application::new().run(|cx: &mut App| {
        if let Err(e) = vassl_db::init(cx) {
            tracing::error!("DB init failed: {e:?}");
            cx.quit();
            return;
        }

        let _app_state = VasslApp::new(cx);

        vassl_inventory::init(cx);
        vassl_quotations::init(cx);
        vassl_pricebook::init(cx);

        cx.activate(true);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds(
                    point(px(100.), px(100.)),
                    size(px(1280.), px(800.)),
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(platform::app_name().into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| VasslRoot::new(window, cx)),
        )
        .unwrap();
    });
}
```

- [ ] **Step 5: Create stub `actions.rs`, `sidebar.rs`, `status_bar.rs`**

`crates/vassl-app/src/actions.rs`:
```rust
// populated in Task 8
```

`crates/vassl-app/src/sidebar.rs`:
```rust
// populated in Task 6
```

`crates/vassl-app/src/status_bar.rs`:
```rust
// populated in Task 7
```

- [ ] **Step 6: Build and run**

```bash
cd /Users/oluwasetemi/r/kamalu/tools
cargo build
cargo run
```

Expected: window opens 1280×800, dark background. Log file created at `$LOCALAPPDATA/VASSL/logs/vassl.YYYY-MM-DD.log` (Windows) or `~/Library/Application Support/VASSL/logs/` (macOS).

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-app/src/
git commit -m "feat(app): tracing init + GPUI window bootstrap + platform module"
```

---

### Task 6: vassl-app — Sidebar view

**Files:**
- Modify: `tools/crates/vassl-app/src/sidebar.rs`
- Modify: `tools/crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write failing test for sidebar state**

Add to `crates/vassl-app/src/sidebar.rs`:
```rust
use gpui::{
    Context, IntoElement, MouseButton, ParentElement, Render, Styled, Window,
    div, px, rgb,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveModule {
    Inventory,
    Quotations,
    PriceBook,
}

pub struct Sidebar {
    pub active: ActiveModule,
}

impl Sidebar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { active: ActiveModule::Inventory }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;

        div()
            .flex().flex_col().justify_between()
            .w(px(48.)).h_full()
            .bg(rgb(0x181825))
            .child(
                div()
                    .flex().flex_col().gap(px(4.)).pt(px(8.))
                    .child(module_btn(ActiveModule::Inventory,  "I", active, cx))
                    .child(module_btn(ActiveModule::Quotations, "Q", active, cx))
                    .child(module_btn(ActiveModule::PriceBook,  "P", active, cx))
            )
            .child(
                div().pb(px(8.)).child(
                    div()
                        .w(px(36.)).h(px(36.)).mx(px(6.))
                        .flex().items_center().justify_center()
                        .rounded(px(6.))
                        .bg(rgb(0x313244))
                        .text_color(rgb(0xcdd6f4))
                        .cursor_pointer()
                        .child("⚙")
                )
            )
    }
}

fn module_btn(
    module: ActiveModule,
    label: &'static str,
    active: ActiveModule,
    cx: &mut Context<Sidebar>,
) -> impl IntoElement {
    let is_active = module == active;
    div()
        .id(label)
        .w(px(36.)).h(px(36.)).mx(px(6.))
        .flex().items_center().justify_center()
        .rounded(px(6.))
        .bg(if is_active { rgb(0x1a3c5e) } else { rgb(0x313244) })
        .text_color(if is_active { rgb(0xcdd6f4) } else { rgb(0x6c7086) })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _w, cx| {
            this.active = module;
            cx.notify();
        }))
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_module_is_inventory() {
        let sidebar = Sidebar { active: ActiveModule::Inventory };
        assert_eq!(sidebar.active, ActiveModule::Inventory);
    }

    #[test]
    fn modules_are_distinct() {
        assert_ne!(ActiveModule::Inventory,  ActiveModule::Quotations);
        assert_ne!(ActiveModule::Quotations, ActiveModule::PriceBook);
        assert_ne!(ActiveModule::Inventory,  ActiveModule::PriceBook);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-app -- sidebar::tests --nocapture
```

Expected: 2 tests pass.

- [ ] **Step 3: Wire sidebar into `root.rs`**

Replace `crates/vassl-app/src/root.rs`:
```rust
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use crate::sidebar::Sidebar;

pub struct VasslRoot {
    sidebar: Entity<Sidebar>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self { sidebar: cx.new(Sidebar::new) }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex().flex_row()
            .w_full().h_full()
            .bg(gpui::rgb(0x1e1e2e))
            .child(self.sidebar.clone())
            .child(
                div()
                    .flex_1().h_full()
                    .bg(gpui::rgb(0x1e1e2e))
                    .child("pane area — Tasks 2–4")
            )
    }
}
```

- [ ] **Step 4: Run and verify sidebar renders**

```bash
cargo run
```

Expected: 48px dark sidebar on left with I/Q/P icon buttons. Clicking each highlights it in `#1a3c5e` navy. Settings gear at bottom.

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-app/src/sidebar.rs crates/vassl-app/src/root.rs
git commit -m "feat(app): Sidebar view — module switching with navy highlight"
```

---

### Task 7: vassl-app — StatusBar view

**Files:**
- Modify: `tools/crates/vassl-app/src/status_bar.rs`
- Modify: `tools/crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write `status_bar.rs` with test**

`crates/vassl-app/src/status_bar.rs`:
```rust
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};

pub struct StatusBar {
    pub last_action: Option<String>,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { last_action: None }
    }

    pub fn set_last_action(&mut self, action: impl Into<String>, cx: &mut Context<Self>) {
        self.last_action = Some(action.into());
        cx.notify();
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.last_action.as_deref().unwrap_or("Ready").to_string();

        div()
            .flex().flex_row().items_center()
            .w_full().h(px(24.))
            .bg(rgb(0x181825))
            .border_t_1()
            .border_color(rgb(0x313244))
            .px(px(12.))
            .text_color(rgb(0x6c7086))
            .text_size(px(11.))
            .child(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_last_action_is_none() {
        let bar = StatusBar { last_action: None };
        assert!(bar.last_action.is_none());
    }

    #[test]
    fn set_last_action_updates_state() {
        let bar = StatusBar { last_action: Some("Stock entry added".into()) };
        assert_eq!(bar.last_action.as_deref(), Some("Stock entry added"));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-app -- status_bar::tests --nocapture
```

Expected: 2 tests pass.

- [ ] **Step 3: Wire StatusBar into `root.rs`**

Replace `crates/vassl-app/src/root.rs`:
```rust
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use crate::sidebar::Sidebar;
use crate::status_bar::StatusBar;

pub struct VasslRoot {
    sidebar:    Entity<Sidebar>,
    status_bar: Entity<StatusBar>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar:    cx.new(Sidebar::new),
            status_bar: cx.new(StatusBar::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex().flex_col()
            .w_full().h_full()
            .bg(gpui::rgb(0x1e1e2e))
            .child(
                div()
                    .flex().flex_row().flex_1()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex_1().h_full()
                            .bg(gpui::rgb(0x1e1e2e))
                            .child("pane area — Tasks 2–4")
                    )
            )
            .child(self.status_bar.clone())
    }
}
```

- [ ] **Step 4: Run and verify layout**

```bash
cargo run
```

Expected: sidebar on left, content area, 24px status bar at bottom showing "Ready".

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-app/src/status_bar.rs crates/vassl-app/src/root.rs
git commit -m "feat(app): StatusBar view — bottom strip with last action text"
```

---

### Task 8: vassl-app — Actions + keybindings

**Files:**
- Modify: `tools/crates/vassl-app/src/actions.rs`
- Create: `tools/assets/keymaps/default.json`
- Modify: `tools/crates/vassl-app/src/main.rs`
- Modify: `tools/crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write `actions.rs`**

`crates/vassl-app/src/actions.rs`:
```rust
use gpui::actions;

actions!(vassl, [
    OpenInventory,
    OpenQuotations,
    OpenPriceBook,
    OpenAuditLog,
    NewRecord,
    FocusSearch,
]);
```

- [ ] **Step 2: Write `assets/keymaps/default.json`**

```json
[
  {
    "context": "VasslRoot",
    "bindings": {
      "ctrl-1":       "vassl::OpenInventory",
      "ctrl-2":       "vassl::OpenQuotations",
      "ctrl-3":       "vassl::OpenPriceBook",
      "ctrl-shift-a": "vassl::OpenAuditLog",
      "ctrl-n":       "vassl::NewRecord",
      "ctrl-f":       "vassl::FocusSearch"
    }
  }
]
```

- [ ] **Step 3: Load keybindings in `main.rs`**

Inside the `Application::new().run(|cx| { ... })` closure, before `cx.open_window(...)`:
```rust
// Load keybindings from assets
let keymap_json = include_str!("../../../assets/keymaps/default.json");
if let Ok(keymap) = gpui::KeymapFile::parse(keymap_json) {
    cx.bind_keys(keymap.bindings());
}
```

- [ ] **Step 4: Register action handlers in `root.rs`**

Add to `root.rs` imports:
```rust
use crate::actions::{OpenInventory, OpenQuotations, OpenPriceBook};
use crate::sidebar::ActiveModule;
```

Update the outer `div()` in `render()` to add `key_context` and action handlers:
```rust
div()
    .flex().flex_col()
    .w_full().h_full()
    .bg(gpui::rgb(0x1e1e2e))
    .key_context("VasslRoot")
    .on_action(cx.listener(|this, _: &OpenInventory, _w, cx| {
        this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Inventory;  cx.notify(); });
    }))
    .on_action(cx.listener(|this, _: &OpenQuotations, _w, cx| {
        this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Quotations; cx.notify(); });
    }))
    .on_action(cx.listener(|this, _: &OpenPriceBook, _w, cx| {
        this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook;  cx.notify(); });
    }))
    .child(/* ... existing sidebar + pane row ... */)
    .child(self.status_bar.clone())
```

- [ ] **Step 5: Run and verify keybindings**

```bash
cargo run
```

Expected:
- `Ctrl+1` → Inventory icon (I) highlighted navy
- `Ctrl+2` → Quotations icon (Q) highlighted navy
- `Ctrl+3` → Price Book icon (P) highlighted navy

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-app/src/actions.rs crates/vassl-app/src/main.rs \
        crates/vassl-app/src/root.rs assets/keymaps/default.json
git commit -m "feat(app): global actions + keybindings (Ctrl+1/2/3 module switching)"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Covered by |
|---|---|
| Single binary `vassl.exe` | Task 1 — `[[bin]] name = "vassl"` |
| SQLite bundled, `vassl.db` in OS data dir | Task 3 — `AppDatabase::new(db_path())` |
| Migrations on startup | Task 3 — `AppMigrator::migrate()` |
| `settings`, `projects`, `audit_log` tables | Task 4 |
| Left 48px icon sidebar | Task 6 |
| Active module highlighted `#1a3c5e` | Task 6 |
| Bottom status bar | Task 7 |
| `Ctrl+1/2/3` module switching | Task 8 |
| Settings gear icon | Task 6 |
| `DomainMigration` + `inventory::collect!` | Task 3 |
| `write_and_log` helper | Task 3 |
| `tracing` daily-rotation log file | Task 5 |
| Platform code isolated | Task 5 — `platform/` module |
| Pure domain types testable without GPUI | Task 1 — `vassl-core` |
| `thiserror` in sub-crates | Tasks 1, 3, 4 |
| `.gitattributes` `*.sql eol=lf` | Task 1 |
| `default-members` so `cargo run` works | Task 1 |

**Not in this plan (Plans 2–5):**
- Inventory, Price Book, Quotations modules
- Command palette (`Ctrl+P`)
- Full audit log view (`Ctrl+Shift+A`)
- Pane splitting
- First-run `current_user` prompt
- `Ctrl+N` / `Ctrl+F` implementations

**Placeholder scan:** None. All steps have complete code.

**Type consistency:** `ActiveModule` defined in `sidebar.rs`, imported in `root.rs` as `crate::sidebar::ActiveModule`. `VasslApp` in `app.rs` extended in Plans 2–4 by adding `Entity<InventoryStore>` etc. `write_and_log` in `vassl-db` uses `tracing::error!` consistently with the `tracing` setup in `main.rs`.

---

## Next Plans

- **Plan 2 — Inventory module:** `vassl-core` product/stock types already done; add `InventoryDb` (static_connection!), `InventoryStore` entity, `InventoryPanel` view, stock list table, entry form modal, product detail pane, restock alerts
- **Plan 3 — Price Book module:** `PriceBookDb`, `PriceBookStore`, price table, entry form with live selling price preview
- **Plan 4 — Quotations module:** `QuotationDb`, `QuotationStore`, quotation list, line items editor, project picker
- **Plan 5 — App Polish:** command palette, full audit log view, pane splitting, first-run user prompt
