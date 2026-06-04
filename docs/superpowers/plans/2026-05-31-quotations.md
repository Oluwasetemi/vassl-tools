# Quotations Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Quotations module — a quotation list, per-quotation line items view, status transitions (Draft → Sent → Accepted/Rejected), and a "New Quotation" modal that auto-generates a reference number and lets the user pick a project.

**Architecture:** `vassl-quotations` follows the same three-layer pattern: `QuotationDb` (sqlez `Domain` + `static_connection!`), `QuotationStore` (GPUI entity), views (`QuotationList`, `QuotationDetail`, `QuotationForm` modal, `QuotationPanel`). The panel has two tabs: "Quotations" (all quotations) and "Items" (line items for selected quotation). Status transitions are button-driven (no text input needed). Line item creation with text input is deferred to Plan 5.

**Pre-requisite — seed test projects:** The `projects` table lives in SharedDomain. Before testing the Quotations module, run this once to create two test projects:

```bash
sqlite3 ~/Library/Application\ Support/VASSL/0-global/db.sqlite "
INSERT INTO projects (name, client_name, status, created_at) VALUES
  ('CCTV Upgrade – City Hall',   'City Council',     'active', datetime('now')),
  ('Warehouse Security Install', 'SupplyCo Ltd',     'active', datetime('now'));
"
```

**Tech Stack:** Rust, GPUI, sqlez, `vassl-core` (`Quotation`, `QuotationItem`, `QuotationStatus`, `Project`, `ProjectStatus`), `vassl-db` (`static_connection!`, `SharedDomain`), chrono (reference number generation, timestamps).

---

## File Map

```
tools/
├── crates/
│   ├── vassl-quotations/
│   │   ├── Cargo.toml                      # add sqlez, tracing, dev-deps
│   │   └── src/
│   │       ├── lib.rs                      # pub fn init(cx), module declarations
│   │       ├── colors.rs                   # mirror of vassl-app colors
│   │       ├── db.rs                       # QuotationDb: Domain + migrations + queries
│   │       ├── store.rs                    # QuotationStore entity + QuotationRow view-model
│   │       ├── panel.rs                    # QuotationPanel: tabs + form wiring
│   │       ├── quotation_list.rs           # QuotationList: scrollable quotation rows
│   │       ├── quotation_detail.rs         # QuotationDetail: line items + status buttons
│   │       └── quotation_form.rs           # QuotationForm: new-quotation modal with project picker
│   └── vassl-app/
│       └── src/
│           └── root.rs                     # replace Quotations placeholder with QuotationPanel
```

---

### Task 1: QuotationDb — domain, migrations, queries

**Files:**
- Modify: `crates/vassl-quotations/Cargo.toml`
- Create: `crates/vassl-quotations/src/db.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace the entire file:

```toml
[package]
name    = "vassl-quotations"
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

- [ ] **Step 2: Write failing tests**

Create `crates/vassl-quotations/src/db.rs`:

```rust
use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::{Project, ProjectStatus, QuotationItem, QuotationStatus};
use vassl_db::SharedDomain;

pub struct QuotationDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for QuotationDb {
    const NAME: &'static str = "quotations";
    const MIGRATIONS: &'static [&'static str] = &[
        "CREATE TABLE IF NOT EXISTS quotations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id       INTEGER NOT NULL REFERENCES projects(id),
            reference_number TEXT UNIQUE NOT NULL,
            status           TEXT NOT NULL DEFAULT 'draft',
            notes            TEXT,
            created_by       TEXT NOT NULL,
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS quotation_items (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            quotation_id   INTEGER NOT NULL REFERENCES quotations(id),
            product_id     INTEGER REFERENCES products(id),
            description    TEXT NOT NULL,
            quantity       REAL NOT NULL,
            unit_price_usd REAL NOT NULL,
            total_usd      REAL NOT NULL
        )",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

vassl_db::static_connection!(QuotationDb, [SharedDomain]);

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_project(db: &QuotationDb, name: &str, client: &str) -> i64 {
        let name   = name.to_string();
        let client = client.to_string();
        db.write(move |conn| {
            conn.exec(
                "CREATE TABLE IF NOT EXISTS projects (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    name        TEXT NOT NULL,
                    client_name TEXT NOT NULL,
                    description TEXT,
                    status      TEXT NOT NULL DEFAULT 'active',
                    created_at  TEXT NOT NULL
                )",
            ).context("create projects table")?()?;
            conn.exec_bound::<(String, String)>(
                "INSERT INTO projects (name, client_name, created_at)
                 VALUES (?1, ?2, datetime('now'))",
            ).context("prepare insert")?((name, client)).context("exec insert")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()?
                .context("rowid None")
        }).await.unwrap()
    }

    #[tokio::test]
    async fn next_reference_number_starts_at_0001() {
        let db  = QuotationDb::open_test_db("quot_ref_first").await;
        let ref_num = db.next_reference_number().unwrap();
        let year = chrono::Utc::now().format("%Y").to_string();
        assert_eq!(ref_num, format!("VASSL-{year}-0001"));
    }

    #[tokio::test]
    async fn next_reference_number_increments() {
        let db  = QuotationDb::open_test_db("quot_ref_incr").await;
        let pid = setup_project(&db, "P1", "C1").await;
        db.insert_quotation(pid, "VASSL-2026-0001", "tester").await.unwrap();
        let ref_num = db.next_reference_number().unwrap();
        assert_eq!(ref_num, "VASSL-2026-0002");
    }

    #[tokio::test]
    async fn insert_and_list_quotation() {
        let db  = QuotationDb::open_test_db("quot_insert_list").await;
        let pid = setup_project(&db, "Project A", "Client A").await;
        let id  = db.insert_quotation(pid, "VASSL-2026-0001", "alice").await.unwrap();
        assert!(id > 0);
        let rows = db.list_quotations_with_project().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reference_number, "VASSL-2026-0001");
        assert_eq!(rows[0].status, QuotationStatus::Draft);
        assert_eq!(rows[0].project_name, "Project A");
    }

    #[tokio::test]
    async fn update_status_changes_quotation_status() {
        let db  = QuotationDb::open_test_db("quot_status_update").await;
        let pid = setup_project(&db, "P2", "C2").await;
        let id  = db.insert_quotation(pid, "VASSL-2026-0001", "alice").await.unwrap();
        db.update_status(id, QuotationStatus::Sent).await.unwrap();
        let rows = db.list_quotations_with_project().unwrap();
        assert_eq!(rows[0].status, QuotationStatus::Sent);
    }

    #[tokio::test]
    async fn list_items_empty_for_new_quotation() {
        let db  = QuotationDb::open_test_db("quot_items_empty").await;
        let pid = setup_project(&db, "P3", "C3").await;
        let qid = db.insert_quotation(pid, "VASSL-2026-0001", "bob").await.unwrap();
        let items = db.list_items_for_quotation(qid).unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn list_projects_returns_seeded_projects() {
        let db = QuotationDb::open_test_db("quot_list_projects").await;
        let _ = setup_project(&db, "Alpha", "AlphaCo").await;
        let _ = setup_project(&db, "Beta",  "BetaCo").await;
        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 2);
    }
}
```

- [ ] **Step 3: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations 2>&1 | head -15
```

Expected: compile error — `next_reference_number`, `insert_quotation`, `list_quotations_with_project`, `update_status`, `list_items_for_quotation`, `list_projects` not defined. `QuotationRow` not defined.

- [ ] **Step 4: Add QuotationRow view-model and implement all query methods**

Add after `vassl_db::static_connection!(QuotationDb, [SharedDomain]);`:

```rust
#[derive(Debug, Clone)]
pub struct QuotationRow {
    pub id:               i64,
    pub reference_number: String,
    pub status:           QuotationStatus,
    pub project_id:       i64,
    pub project_name:     String,
    pub client_name:      String,
    pub total_usd:        f64,
    pub created_at:       String,
    pub notes:            Option<String>,
}

fn status_from_str(s: &str) -> QuotationStatus {
    match s {
        "sent"     => QuotationStatus::Sent,
        "accepted" => QuotationStatus::Accepted,
        "rejected" => QuotationStatus::Rejected,
        _          => QuotationStatus::Draft,
    }
}

fn status_to_str(s: &QuotationStatus) -> &'static str {
    match s {
        QuotationStatus::Draft    => "draft",
        QuotationStatus::Sent     => "sent",
        QuotationStatus::Accepted => "accepted",
        QuotationStatus::Rejected => "rejected",
    }
}

impl QuotationDb {
    pub fn next_reference_number(&self) -> anyhow::Result<String> {
        let year    = chrono::Utc::now().format("%Y").to_string();
        let pattern = format!("VASSL-{year}-%");
        let count: i64 = self
            .select_row_bound::<String, Option<i64>>(
                "SELECT COUNT(*) FROM quotations WHERE reference_number LIKE ?1",
            )
            .context("prepare count")?
            (pattern)
            .context("execute count")?
            .flatten()
            .unwrap_or(0);
        Ok(format!("VASSL-{year}-{:04}", count + 1))
    }

    pub fn list_quotations_with_project(&self) -> anyhow::Result<Vec<QuotationRow>> {
        type Row = (i64, String, String, String, Option<String>, String, String, i64, String, String, f64);
        self.select::<Row>(
            "SELECT q.id, q.reference_number, q.status, q.created_at, q.notes,
                    q.created_by, q.updated_at, q.project_id,
                    p.name, p.client_name,
                    COALESCE(SUM(i.total_usd), 0.0) AS total_usd
             FROM quotations q
             JOIN projects p ON p.id = q.project_id
             LEFT JOIN quotation_items i ON i.quotation_id = q.id
             GROUP BY q.id
             ORDER BY q.created_at DESC",
        )
        .context("prepare list_quotations_with_project")?()
        .context("execute list_quotations_with_project")
        .map(|rows| {
            rows.into_iter().map(|(id, ref_num, status_str, created_at, notes,
                                   _created_by, _updated_at, project_id,
                                   project_name, client_name, total_usd)| {
                QuotationRow {
                    id,
                    reference_number: ref_num,
                    status: status_from_str(&status_str),
                    project_id,
                    project_name,
                    client_name,
                    total_usd,
                    created_at,
                    notes,
                }
            }).collect()
        })
    }

    pub fn list_items_for_quotation(&self, quotation_id: i64) -> anyhow::Result<Vec<QuotationItem>> {
        self.select_bound::<i64, (i64, i64, Option<i64>, String, f64, f64, f64)>(
            "SELECT id, quotation_id, product_id, description, quantity, unit_price_usd, total_usd
             FROM quotation_items WHERE quotation_id = ?1
             ORDER BY id ASC",
        )
        .context("prepare list_items_for_quotation")?
        (quotation_id)
        .context("execute list_items_for_quotation")
        .map(|rows| {
            rows.into_iter().map(|(id, qid, product_id, description, quantity, unit_price_usd, total_usd)| {
                QuotationItem { id, quotation_id: qid, product_id, description, quantity, unit_price_usd, total_usd }
            }).collect()
        })
    }

    pub fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        self.select::<(i64, String, String, Option<String>, String, String)>(
            "SELECT id, name, client_name, description, status, created_at
             FROM projects ORDER BY name",
        )
        .context("prepare list_projects")?()
        .context("execute list_projects")
        .map(|rows| {
            rows.into_iter().map(|(id, name, client_name, description, status_str, created_at)| {
                let status = match status_str.as_str() {
                    "completed" => ProjectStatus::Completed,
                    "archived"  => ProjectStatus::Archived,
                    _           => ProjectStatus::Active,
                };
                Project { id, name, client_name, description, status, created_at }
            }).collect()
        })
    }

    pub async fn insert_quotation(
        &self,
        project_id:       i64,
        reference_number: impl Into<String>,
        created_by:       impl Into<String>,
    ) -> anyhow::Result<i64> {
        let ref_num    = reference_number.into();
        let created_by = created_by.into();
        let now        = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(i64, String, String, String, String)>(
                "INSERT INTO quotations
                 (project_id, reference_number, status, created_by, created_at, updated_at)
                 VALUES (?1, ?2, 'draft', ?3, ?4, ?5)",
            )
            .context("prepare insert_quotation")?
            ((project_id, ref_num, created_by, now.clone(), now))
            .context("execute insert_quotation")?;

            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
                .context("execute rowid")?
                .context("rowid was None")
        })
        .await
    }

    pub async fn update_status(&self, id: i64, status: QuotationStatus) -> anyhow::Result<()> {
        let status_str = status_to_str(&status).to_string();
        let now        = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(String, String, i64)>(
                "UPDATE quotations SET status = ?1, updated_at = ?2 WHERE id = ?3",
            )
            .context("prepare update_status")?
            ((status_str, now, id))
            .context("execute update_status")
        })
        .await
    }
}
```

- [ ] **Step 5: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations db 2>&1 | tail -15
```

Expected: 6 tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/Cargo.toml crates/vassl-quotations/src/db.rs && git commit -m "feat(quotations): QuotationDb domain, migrations, queries"
```

---

### Task 2: QuotationStore — GPUI entity

**Files:**
- Create: `crates/vassl-quotations/src/store.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/vassl-quotations/src/store.rs`:

```rust
use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::{Project, QuotationItem, QuotationStatus};

use crate::db::{QuotationDb, QuotationRow};

pub struct QuotationStore {
    pub quotations: Vec<QuotationRow>,
    pub selected_id: Option<i64>,
    pub line_items:  Vec<QuotationItem>,
    pub projects:    Vec<Project>,
    pub loading:     bool,
}

pub struct QuotationStoreHandle(pub Entity<QuotationStore>);
impl Global for QuotationStoreHandle {}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;
    use crate::db::QuotationRow;

    fn make_row(id: i64, status: QuotationStatus) -> QuotationRow {
        QuotationRow {
            id,
            reference_number: format!("VASSL-2026-{id:04}"),
            status,
            project_id:   1,
            project_name: "Test Project".to_string(),
            client_name:  "Test Client".to_string(),
            total_usd:    0.0,
            created_at:   "2026-01-01T00:00:00Z".to_string(),
            notes:        None,
        }
    }

    #[test]
    fn quotation_store_starts_empty() {
        let store = QuotationStore {
            quotations: vec![],
            selected_id: None,
            line_items: vec![],
            projects: vec![],
            loading: false,
        };
        assert!(store.quotations.is_empty());
        assert!(store.selected_id.is_none());
    }

    #[test]
    fn selected_quotation_lookup() {
        let rows = vec![
            make_row(1, QuotationStatus::Draft),
            make_row(2, QuotationStatus::Sent),
        ];
        let found = rows.iter().find(|r| r.id == 2);
        assert!(found.is_some());
        assert_eq!(found.unwrap().status, QuotationStatus::Sent);
    }

    #[test]
    fn status_badge_color_mapping() {
        assert_eq!(status_badge_color(QuotationStatus::Draft),    crate::colors::STATUS_GREY);
        assert_eq!(status_badge_color(QuotationStatus::Sent),     crate::colors::STATUS_AMBER);
        assert_eq!(status_badge_color(QuotationStatus::Accepted), crate::colors::STATUS_GREEN);
        assert_eq!(status_badge_color(QuotationStatus::Rejected), crate::colors::STATUS_RED);
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations store 2>&1 | head -10
```

Expected: compile errors — `status_badge_color` not defined, `crate::colors` not yet declared.

- [ ] **Step 3: Implement QuotationStore**

Add after `impl Global for QuotationStoreHandle {}`:

```rust
pub fn status_badge_color(status: QuotationStatus) -> u32 {
    match status {
        QuotationStatus::Draft    => crate::colors::STATUS_GREY,
        QuotationStatus::Sent     => crate::colors::STATUS_AMBER,
        QuotationStatus::Accepted => crate::colors::STATUS_GREEN,
        QuotationStatus::Rejected => crate::colors::STATUS_RED,
    }
}

#[derive(Debug)]
pub enum QuotationEvent {
    QuotationsLoaded,
    ItemsLoaded,
}

impl EventEmitter<QuotationEvent> for QuotationStore {}

impl QuotationStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            quotations:  Vec::new(),
            selected_id: None,
            line_items:  Vec::new(),
            projects:    Vec::new(),
            loading:     false,
        }
    }

    pub fn load_quotations(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let (quot_result, proj_result) = (
                cx.background_executor().spawn(async move { db.list_quotations_with_project() }).await,
                cx.background_executor().spawn(async move { db.list_projects() }).await,
            );

            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match quot_result {
                    Ok(rows) => { store.quotations = rows; }
                    Err(e)   => tracing::error!("list_quotations_with_project failed: {e:?}"),
                }
                match proj_result {
                    Ok(projects) => { store.projects = projects; }
                    Err(e)       => tracing::error!("list_projects failed: {e:?}"),
                }
                cx.emit(QuotationEvent::QuotationsLoaded);
                cx.notify();
            });
        }).detach();
    }

    pub fn select_quotation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.selected_id == Some(id) { return; }
        self.selected_id = Some(id);
        self.line_items.clear();
        cx.notify();

        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_items_for_quotation(id) })
                .await;
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(items) => { store.line_items = items; cx.emit(QuotationEvent::ItemsLoaded); }
                    Err(e)    => tracing::error!("list_items_for_quotation failed: {e:?}"),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn transition_status(&mut self, id: i64, new_status: QuotationStatus, cx: &mut Context<Self>) {
        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.update_status(id, new_status).await;
            if let Err(e) = result {
                tracing::error!("update_status failed: {e:?}");
                return Ok(());
            }
            let _ = this.update(cx, |store, cx| store.load_quotations(cx));
            Ok(())
        }).detach();
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations store 2>&1 | tail -10
```

Expected: 3 tests pass (after colors module is stubbed in next task).

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/src/store.rs && git commit -m "feat(quotations): QuotationStore entity with async loads and status transitions"
```

---

### Task 3: colors.rs + lib.rs init + stubs

**Files:**
- Create: `crates/vassl-quotations/src/colors.rs`
- Modify: `crates/vassl-quotations/src/lib.rs`
- Create stub files for remaining modules

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
pub mod quotation_detail;
pub mod quotation_form;
pub mod quotation_list;
pub mod store;

use gpui::{App, AppContext, Entity};

pub use db::QuotationDb;
pub use store::{QuotationStore, QuotationStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<QuotationStore> = cx.new(QuotationStore::new);
    cx.set_global(QuotationStoreHandle(store));
}
```

- [ ] **Step 3: Create stub files**

Create `crates/vassl-quotations/src/quotation_list.rs`:
```rust
// stub — implemented in Task 4
```

Create `crates/vassl-quotations/src/quotation_detail.rs`:
```rust
// stub — implemented in Task 5
```

Create `crates/vassl-quotations/src/quotation_form.rs`:
```rust
// stub — implemented in Task 6
```

Create `crates/vassl-quotations/src/panel.rs`:
```rust
// stub — implemented in Task 7
```

- [ ] **Step 4: Run all pricebook tests to confirm colors is satisfied**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations 2>&1 | tail -10
```

Expected: all 9 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/src/colors.rs crates/vassl-quotations/src/lib.rs crates/vassl-quotations/src/quotation_list.rs crates/vassl-quotations/src/quotation_detail.rs crates/vassl-quotations/src/quotation_form.rs crates/vassl-quotations/src/panel.rs && git commit -m "feat(quotations): lib init, colors, stub modules"
```

---

### Task 4: QuotationList view

**Files:**
- Modify: `crates/vassl-quotations/src/quotation_list.rs`

- [ ] **Step 1: Write failing tests**

Replace the stub:

```rust
use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::db::QuotationRow;
use crate::store::{QuotationStore, status_badge_color};

pub struct QuotationList {
    store: Entity<QuotationStore>,
}

impl QuotationList {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;
    use crate::db::QuotationRow;

    fn make_row(id: i64, ref_num: &str, total: f64) -> QuotationRow {
        QuotationRow {
            id,
            reference_number: ref_num.to_string(),
            status:      QuotationStatus::Draft,
            project_id:  1,
            project_name: "Test Project".to_string(),
            client_name: "Client A".to_string(),
            total_usd:   total,
            created_at:  "2026-01-01T00:00:00Z".to_string(),
            notes:       None,
        }
    }

    #[test]
    fn format_total_two_decimal_places() {
        let row = make_row(1, "VASSL-2026-0001", 1234.5);
        let formatted = format_total(row.total_usd);
        assert_eq!(formatted, "$1234.50");
    }

    #[test]
    fn format_date_trims_to_10_chars() {
        let row = make_row(1, "VASSL-2026-0001", 0.0);
        let date = &row.created_at[..10];
        assert_eq!(date.len(), 10);
        assert_eq!(date, "2026-01-01");
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations quotation_list 2>&1 | tail -10
```

Expected: compile error — `format_total` not defined, `Render` not implemented.

- [ ] **Step 3: Implement QuotationList**

Replace the file:

```rust
use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::db::QuotationRow;
use crate::store::{QuotationStore, status_badge_color};

pub struct QuotationList {
    store: Entity<QuotationStore>,
}

impl QuotationList {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

pub fn format_total(usd: f64) -> String {
    format!("${usd:.2}")
}

impl Render for QuotationList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("Loading…")
                .into_any_element();
        }

        if store.quotations.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_DEFAULT))
                .child("No quotations yet — click \"+ New Quotation\" to create one.")
                .into_any_element();
        }

        let selected = store.selected_id;
        let rows: Vec<_> = store.quotations.iter().map(|q| {
            quotation_row(q, selected == Some(q.id), self.store.clone())
        }).collect();

        div()
            .id("quotation-list-scroll")
            .flex_1().flex().flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn quotation_row(q: &QuotationRow, selected: bool, store: Entity<QuotationStore>) -> impl IntoElement {
    let id        = q.id;
    let row_bg    = if selected { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG };
    let badge_col = status_badge_color(q.status.clone());
    let date_str  = q.created_at.get(..10).unwrap_or("").to_string();

    div()
        .id(format!("quot-row-{id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(6.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_quotation(id, cx));
            },
        )
        // Status badge dot
        .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(badge_col)).mr(px(8.)))
        // Reference number
        .child(
            div().w(px(130.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                .child(q.reference_number.clone())
        )
        // Project + client
        .child(
            div().flex_1().text_size(px(12.)).text_color(rgb(colors::TEXT_MUTED))
                .child(format!("{} / {}", q.project_name, q.client_name))
        )
        // Total
        .child(
            div().w(px(90.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                .child(format_total(q.total_usd))
        )
        // Date
        .child(
            div().w(px(90.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                .child(date_str)
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;
    use crate::db::QuotationRow;

    fn make_row(id: i64, ref_num: &str, total: f64) -> QuotationRow {
        QuotationRow {
            id,
            reference_number: ref_num.to_string(),
            status:       QuotationStatus::Draft,
            project_id:   1,
            project_name: "Test Project".to_string(),
            client_name:  "Client A".to_string(),
            total_usd:    total,
            created_at:   "2026-01-01T00:00:00Z".to_string(),
            notes:        None,
        }
    }

    #[test]
    fn format_total_two_decimal_places() {
        let row = make_row(1, "VASSL-2026-0001", 1234.5);
        let formatted = format_total(row.total_usd);
        assert_eq!(formatted, "$1234.50");
    }

    #[test]
    fn format_date_trims_to_10_chars() {
        let row = make_row(1, "VASSL-2026-0001", 0.0);
        let date = &row.created_at[..10];
        assert_eq!(date.len(), 10);
        assert_eq!(date, "2026-01-01");
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations quotation_list 2>&1 | tail -10
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/src/quotation_list.rs && git commit -m "feat(quotations): QuotationList view with status badges"
```

---

### Task 5: QuotationDetail — line items + status transition buttons

**Files:**
- Modify: `crates/vassl-quotations/src/quotation_detail.rs`

- [ ] **Step 1: Write failing tests**

Replace the stub:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rgb};

use crate::colors;
use crate::store::{QuotationStore, status_badge_color};

pub struct QuotationDetail {
    store: Entity<QuotationStore>,
}

impl QuotationDetail {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;

    #[test]
    fn next_status_transitions_are_correct() {
        assert_eq!(next_transitions(QuotationStatus::Draft),    vec![QuotationStatus::Sent]);
        assert_eq!(next_transitions(QuotationStatus::Sent),     vec![QuotationStatus::Accepted, QuotationStatus::Rejected]);
        assert!(next_transitions(QuotationStatus::Accepted).is_empty());
        assert!(next_transitions(QuotationStatus::Rejected).is_empty());
    }

    #[test]
    fn transition_label_is_human_readable() {
        assert_eq!(transition_label(&QuotationStatus::Sent),     "Mark as Sent");
        assert_eq!(transition_label(&QuotationStatus::Accepted), "Mark as Accepted");
        assert_eq!(transition_label(&QuotationStatus::Rejected), "Mark as Rejected");
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations quotation_detail 2>&1 | tail -10
```

Expected: compile error — `next_transitions`, `transition_label` not defined.

- [ ] **Step 3: Implement QuotationDetail**

Replace the file:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rgb};
use vassl_core::QuotationStatus;

use crate::colors;
use crate::store::{QuotationStore, status_badge_color};

pub struct QuotationDetail {
    store: Entity<QuotationStore>,
}

impl QuotationDetail {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

pub fn next_transitions(status: QuotationStatus) -> Vec<QuotationStatus> {
    match status {
        QuotationStatus::Draft    => vec![QuotationStatus::Sent],
        QuotationStatus::Sent     => vec![QuotationStatus::Accepted, QuotationStatus::Rejected],
        QuotationStatus::Accepted => vec![],
        QuotationStatus::Rejected => vec![],
    }
}

pub fn transition_label(status: &QuotationStatus) -> &'static str {
    match status {
        QuotationStatus::Draft    => "Mark as Draft",
        QuotationStatus::Sent     => "Mark as Sent",
        QuotationStatus::Accepted => "Mark as Accepted",
        QuotationStatus::Rejected => "Mark as Rejected",
    }
}

impl Render for QuotationDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (selected_id, current_status, items, total) = {
            let store = self.store.read(cx);
            let sid   = store.selected_id;
            let status = store.quotations.iter()
                .find(|q| Some(q.id) == sid)
                .map(|q| q.status.clone());
            let total: f64 = store.line_items.iter().map(|i| i.total_usd).sum();
            (sid, status, store.line_items.clone(), total)
        };

        if selected_id.is_none() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("Select a quotation to view its line items.")
                .into_any_element();
        }

        let mut root = div().flex_1().flex().flex_col();

        // Status transition buttons
        if let Some(ref status) = current_status {
            let transitions = next_transitions(status.clone());
            if !transitions.is_empty() {
                let id = selected_id.unwrap();
                let btn_row = div()
                    .flex().flex_row().gap(px(8.))
                    .px(px(12.)).py(px(8.))
                    .children(transitions.into_iter().map(|next_status| {
                        let store = self.store.clone();
                        let ns = next_status.clone();
                        div()
                            .id(format!("status-btn-{}", transition_label(&next_status)))
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(status_badge_color(next_status.clone())))
                            .text_size(px(12.)).text_color(rgb(colors::CANVAS_BG))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left,
                                move |_, _, cx: &mut gpui::App| {
                                    store.update(cx, |s, cx| s.transition_status(id, ns.clone(), cx));
                                })
                            .child(transition_label(&next_status).to_string())
                    }));
                root = root.child(btn_row);
            }
        }

        // Line items header
        root = root.child(
            div()
                .flex().flex_row().items_center()
                .px(px(12.)).py(px(4.))
                .bg(rgb(colors::SURFACE_DEFAULT))
                .child(div().flex_1().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Description"))
                .child(div().w(px(70.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Qty"))
                .child(div().w(px(90.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Unit Price"))
                .child(div().w(px(90.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Total"))
        );

        if items.is_empty() {
            root = root.child(
                div()
                    .flex_1().flex().items_center().justify_center()
                    .text_color(rgb(colors::TEXT_MUTED))
                    .child("No line items. (Add items in Plan 5 once text input is available.)")
            );
        } else {
            let item_rows = div()
                .id("items-scroll").flex_1().flex().flex_col().overflow_y_scroll()
                .children(items.iter().map(|item| {
                    div()
                        .flex().flex_row().items_center().w_full()
                        .px(px(12.)).py(px(6.))
                        .child(div().flex_1().text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT)).child(item.description.clone()))
                        .child(div().w(px(70.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT)).child(format!("{:.2}", item.quantity)))
                        .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT)).child(format!("${:.2}", item.unit_price_usd)))
                        .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(colors::STATUS_GREEN)).child(format!("${:.2}", item.total_usd)))
                }));
            root = root.child(item_rows);
        }

        // Total footer
        root.child(
            div()
                .flex().flex_row().justify_end()
                .px(px(12.)).py(px(8.))
                .bg(rgb(colors::SURFACE_DEFAULT))
                .child(
                    div().text_size(px(13.)).text_color(rgb(colors::STATUS_GREEN))
                        .child(format!("Total: ${total:.2}"))
                )
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;

    #[test]
    fn next_status_transitions_are_correct() {
        assert_eq!(next_transitions(QuotationStatus::Draft),    vec![QuotationStatus::Sent]);
        assert_eq!(next_transitions(QuotationStatus::Sent),     vec![QuotationStatus::Accepted, QuotationStatus::Rejected]);
        assert!(next_transitions(QuotationStatus::Accepted).is_empty());
        assert!(next_transitions(QuotationStatus::Rejected).is_empty());
    }

    #[test]
    fn transition_label_is_human_readable() {
        assert_eq!(transition_label(&QuotationStatus::Sent),     "Mark as Sent");
        assert_eq!(transition_label(&QuotationStatus::Accepted), "Mark as Accepted");
        assert_eq!(transition_label(&QuotationStatus::Rejected), "Mark as Rejected");
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations quotation_detail 2>&1 | tail -10
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/src/quotation_detail.rs && git commit -m "feat(quotations): QuotationDetail with line items and status transition buttons"
```

---

### Task 6: QuotationForm — new quotation modal with inline project picker

**Files:**
- Modify: `crates/vassl-quotations/src/quotation_form.rs`

- [ ] **Step 1: Write failing tests**

Replace the stub:

```rust
use gpui::{App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           MouseButton, MouseDownEvent, Render, Window, div, prelude::*, px, rgb, rgba};
use vassl_core::{Project, QuotationStatus};

use crate::colors;
use crate::db::QuotationDb;
use crate::store::QuotationStore;

#[derive(Debug)]
pub enum QuotationFormEvent {
    Submitted,
    Cancelled,
}

impl EventEmitter<QuotationFormEvent> for QuotationForm {}

pub struct QuotationForm {
    store:              Entity<QuotationStore>,
    reference_number:   String,
    projects:           Vec<Project>,
    selected_project:   Option<i64>,
    error:              Option<String>,
    focus_handle:       FocusHandle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_number_format() {
        let ref_num = "VASSL-2026-0001";
        assert!(ref_num.starts_with("VASSL-"));
        assert_eq!(ref_num.len(), 14);
    }

    #[test]
    fn form_requires_project_selection() {
        let error = validate_form(None);
        assert!(error.is_some());
        assert!(error.unwrap().contains("project"));
    }

    #[test]
    fn form_valid_with_project_selected() {
        let error = validate_form(Some(1));
        assert!(error.is_none());
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations quotation_form 2>&1 | tail -10
```

Expected: compile error — `validate_form` not defined.

- [ ] **Step 3: Implement QuotationForm**

Replace the file:

```rust
use gpui::{App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           MouseButton, MouseDownEvent, Render, Window, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::Project;

use crate::colors;
use crate::db::QuotationDb;
use crate::store::QuotationStore;

#[derive(Debug)]
pub enum QuotationFormEvent {
    Submitted,
    Cancelled,
}

impl EventEmitter<QuotationFormEvent> for QuotationForm {}

pub struct QuotationForm {
    store:            Entity<QuotationStore>,
    reference_number: String,
    projects:         Vec<Project>,
    selected_project: Option<i64>,
    error:            Option<String>,
    focus_handle:     FocusHandle,
}

pub fn validate_form(selected_project: Option<i64>) -> Option<String> {
    if selected_project.is_none() {
        Some("Please select a project.".to_string())
    } else {
        None
    }
}

impl QuotationForm {
    pub fn new(
        store:            Entity<QuotationStore>,
        reference_number: String,
        projects:         Vec<Project>,
        cx:               &mut Context<Self>,
    ) -> Self {
        Self {
            store,
            reference_number,
            projects,
            selected_project: None,
            error:            None,
            focus_handle:     cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match validate_form(self.selected_project) {
            Some(msg) => {
                self.error = Some(msg);
                cx.notify();
            }
            None => {
                let pid    = self.selected_project.unwrap();
                let ref_num = self.reference_number.clone();
                let store  = self.store.clone();
                let db     = QuotationDb::global(&**cx);

                cx.spawn(async move |this, cx| {
                    let result = db.insert_quotation(pid, ref_num, "user").await;
                    if let Err(e) = result {
                        tracing::error!("insert_quotation failed: {e:?}");
                        return Ok(());
                    }
                    let _ = store.update(cx, |s, cx| s.load_quotations(cx));
                    this.update(cx, |_, cx| cx.emit(QuotationFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for QuotationForm {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for QuotationForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(460.))
                    .bg(rgb(colors::CANVAS_BG))
                    .rounded(px(8.))
                    .p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    // Title
                    .child(
                        div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .child("New Quotation")
                    )
                    // Reference number (read-only)
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Reference Number"))
                            .child(
                                div().px(px(8.)).py(px(6.))
                                    .bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                    .text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                                    .child(self.reference_number.clone())
                            )
                    )
                    // Project picker (scrollable list)
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Select Project"))
                            .child(
                                div()
                                    .id("project-picker")
                                    .h(px(120.)).overflow_y_scroll()
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .rounded(px(4.))
                                    .children(self.projects.iter().map(|p| {
                                        let pid      = p.id;
                                        let selected = self.selected_project == Some(pid);
                                        let bg       = if selected { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT };
                                        let store_entity = self.store.clone();
                                        div()
                                            .id(format!("pick-project-{pid}"))
                                            .flex().flex_row().items_center()
                                            .px(px(8.)).py(px(5.))
                                            .bg(rgb(bg))
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                                    this.selected_project = Some(pid);
                                                    this.error = None;
                                                    cx.notify();
                                                })
                                            )
                                            .child(
                                                div().flex_1().text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                                    .child(p.name.clone())
                                            )
                                            .child(
                                                div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                                                    .child(p.client_name.clone())
                                            )
                                    }))
                            )
                    )
                    // Error
                    .child(
                        div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                            .child(self.error.as_deref().map(SharedString::from).unwrap_or_default())
                    )
                    // Buttons
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(
                                div()
                                    .id("quot-btn-cancel")
                                    .px(px(16.)).py(px(6.)).rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
                                        cx.emit(QuotationFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div()
                                    .id("quot-btn-create")
                                    .px(px(16.)).py(px(6.)).rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_ACTIVE))
                                    .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.submit(cx);
                                    }))
                                    .child("Create")
                            )
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_number_format() {
        let ref_num = "VASSL-2026-0001";
        assert!(ref_num.starts_with("VASSL-"));
        assert_eq!(ref_num.len(), 14);
    }

    #[test]
    fn form_requires_project_selection() {
        let error = validate_form(None);
        assert!(error.is_some());
        assert!(error.unwrap().contains("project"));
    }

    #[test]
    fn form_valid_with_project_selected() {
        let error = validate_form(Some(1));
        assert!(error.is_none());
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations quotation_form 2>&1 | tail -10
```

Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/src/quotation_form.rs && git commit -m "feat(quotations): QuotationForm with inline project picker and auto-generated reference"
```

---

### Task 7: QuotationPanel — tab bar + form wiring

**Files:**
- Modify: `crates/vassl-quotations/src/panel.rs`

- [ ] **Step 1: Write the full panel**

Replace the stub:

```rust
use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::quotation_detail::QuotationDetail;
use crate::quotation_form::{QuotationForm, QuotationFormEvent};
use crate::quotation_list::QuotationList;
use crate::store::QuotationStore;
use crate::QuotationStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab { Quotations, Items }

pub struct QuotationPanel {
    store:      Entity<QuotationStore>,
    quot_list:  Entity<QuotationList>,
    quot_detail: Entity<QuotationDetail>,
    active_tab: Tab,
    form:       Option<Entity<QuotationForm>>,
    _form_sub:  Option<Subscription>,
}

impl QuotationPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store       = cx.global::<QuotationStoreHandle>().0.clone();
        let quot_list   = cx.new(|cx| QuotationList::new(store.clone(), cx));
        let quot_detail = cx.new(|cx| QuotationDetail::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_quotations(cx));
        Self {
            store,
            quot_list,
            quot_detail,
            active_tab: Tab::Quotations,
            form:       None,
            _form_sub:  None,
        }
    }

    fn open_form(&mut self, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let (ref_num, projects) = {
            let store    = self.store.read(cx);
            let db       = crate::db::QuotationDb::global(&**cx);
            let ref_num  = db.next_reference_number().unwrap_or_else(|_| "VASSL-ERR-0000".to_string());
            (ref_num, store.projects.clone())
        };
        let form = cx.new(|cx| QuotationForm::new(self.store.clone(), ref_num, projects, cx));
        let sub  = cx.subscribe(&form, |this, _form, ev: &QuotationFormEvent, cx| {
            match ev {
                QuotationFormEvent::Submitted | QuotationFormEvent::Cancelled => {
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

impl Render for QuotationPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Quotations => content.child(self.quot_list.clone()),
            Tab::Items      => content.child(self.quot_detail.clone()),
        };

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    .child(
                        div()
                            .id("quot-tab-quotations")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Quotations { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Quotations;
                                cx.notify();
                            }))
                            .child("Quotations")
                    )
                    .child(
                        div()
                            .id("quot-tab-items")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Items { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Items;
                                cx.notify();
                            }))
                            .child("Items")
                    )
                    .child(div().flex_1())
                    // New Quotation button — always enabled (form has inline project picker)
                    .child(
                        div()
                            .id("quot-btn-new")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(colors::SURFACE_ACTIVE))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.open_form(cx);
                            }))
                            .child("+ New Quotation")
                    )
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
cd /Users/oluwasetemi/r/kamalu/tools && cargo build -p vassl-quotations 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/src/panel.rs && git commit -m "feat(quotations): QuotationPanel with Quotations/Items tabs and new-quotation form"
```

---

### Task 8: Wire QuotationPanel into VasslRoot

**Files:**
- Modify: `crates/vassl-app/src/root.rs`
- Modify: `crates/vassl-app/Cargo.toml`

- [ ] **Step 1: Add vassl-quotations to vassl-app Cargo.toml**

In `crates/vassl-app/Cargo.toml`, the dependencies block should include:

```toml
vassl-db                     = { path = "../vassl-db" }
vassl-inventory              = { path = "../vassl-inventory" }
vassl-quotations             = { path = "../vassl-quotations" }
vassl-pricebook              = { path = "../vassl-pricebook" }
```

- [ ] **Step 2: Update root.rs**

Replace `crates/vassl-app/src/root.rs`:

```rust
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, rgb};

use crate::actions::{OpenInventory, OpenPriceBook, OpenQuotations};
use crate::colors;
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;
use vassl_inventory::panel::InventoryPanel;
use vassl_pricebook::panel::PriceBookPanel;
use vassl_quotations::panel::QuotationPanel;

pub struct VasslRoot {
    sidebar:          Entity<Sidebar>,
    status_bar:       Entity<StatusBar>,
    inventory_panel:  Entity<InventoryPanel>,
    pricebook_panel:  Entity<PriceBookPanel>,
    quotation_panel:  Entity<QuotationPanel>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar:          cx.new(Sidebar::new),
            status_bar:       cx.new(StatusBar::new),
            inventory_panel:  cx.new(InventoryPanel::new),
            pricebook_panel:  cx.new(PriceBookPanel::new),
            quotation_panel:  cx.new(QuotationPanel::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.sidebar.read(cx).active;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active {
            ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
            ActiveModule::Quotations => content.child(self.quotation_panel.clone()),
            ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
        };

        div()
            .key_context("VasslRoot")
            .on_action(cx.listener(|this, _: &OpenInventory, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Inventory; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenQuotations, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Quotations; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenPriceBook, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); });
            }))
            // TODO(Plan 5): add on_action handlers for OpenAuditLog, NewRecord, FocusSearch
            .flex().flex_col().w_full().h_full()
            .bg(rgb(colors::CANVAS_BG))
            .child(
                div().flex().flex_row().flex_1()
                    .child(self.sidebar.clone())
                    .child(content),
            )
            .child(self.status_bar.clone())
    }
}
```

- [ ] **Step 3: Build full workspace**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build 2>&1 | grep "^error" | head -20
```

Expected: no errors.

- [ ] **Step 4: Run all tests**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-app/src/root.rs crates/vassl-app/Cargo.toml && git commit -m "feat(app): wire QuotationPanel into VasslRoot, replacing Plan 4 placeholder"
```

---

## Self-Review

**Spec coverage:**

| Spec requirement | Task |
|---|---|
| `quotations` + `quotation_items` DB tables | Task 1 — migrations |
| Reference number format `VASSL-YYYY-NNNN` | Task 1 — `next_reference_number()` |
| Status transitions: draft → sent → accepted/rejected | Task 5 — `next_transitions()` + `transition_status()` |
| Quotation list: ref number, project+client, status badge, date, total | Task 4 — `quotation_row` |
| Quotation detail: line items | Task 5 — `QuotationDetail` |
| New quotation modal with project picker | Task 6 — `QuotationForm` inline picker |
| `Ctrl+2` switches to Quotations | Foundation plan — `OpenQuotations` action (already done) |
| Module init pattern | Task 3 — `pub fn init(cx)` |

**Not in this plan (Plan 5):**
- Line item creation / editing (requires text input)
- Notes field on quotation (requires text input)
- Project creation from UI (requires text input)
- Global search by reference number, project name, client
- "Project sidebar tree" grouping

**Placeholder scan:** No TBD or incomplete stubs in task code.

**Type consistency:** `QuotationRow` defined in `db.rs`, used in `store.rs`, `quotation_list.rs`. `QuotationStatus` from `vassl-core`, used in `db.rs`, `store.rs`, `quotation_list.rs`, `quotation_detail.rs`. `QuotationFormEvent` defined and emitted in `quotation_form.rs`, subscribed in `panel.rs`. `QuotationStoreHandle` defined in `store.rs`, exported via `lib.rs`, accessed via `cx.global::<QuotationStoreHandle>()` in `panel.rs`. Consistent throughout.

---

## Next Plans

- **Plan 5 — App Polish:** GPUI `TextInput` component for all forms, product CRUD, line item editor, first-run user prompt, full audit log view, command palette (`Ctrl+P`)
