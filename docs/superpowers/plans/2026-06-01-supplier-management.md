# Supplier Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Suppliers module — a new sidebar panel with a structured supplier directory (create/edit). Supplier records stand alone; the existing `stock_entries.supplier TEXT` field is untouched.

**Architecture:** New `vassl-suppliers` crate following the exact pattern of `vassl-pricebook`. `Supplier`/`NewSupplier` types live in `vassl-core`. The panel plugs into the existing `VasslRoot` + `Sidebar` scaffolding as a new `ActiveModule::Suppliers` entry. The form supports both create and edit via two constructors.

**Tech Stack:** Rust, GPUI, sqlez, vassl-core types, chrono for timestamps.

---

## File Map

| File | Change |
|---|---|
| `crates/vassl-core/src/supplier.rs` | **New** — `Supplier`, `NewSupplier` structs |
| `crates/vassl-core/src/lib.rs` | Export `pub mod supplier` + re-exports |
| `Cargo.toml` | Add `"crates/vassl-suppliers"` to workspace members |
| `crates/vassl-suppliers/Cargo.toml` | **New** — crate manifest |
| `crates/vassl-suppliers/src/lib.rs` | **New** — init(), re-exports |
| `crates/vassl-suppliers/src/db.rs` | **New** — `SupplierDb` + CRUD methods |
| `crates/vassl-suppliers/src/store.rs` | **New** — `SupplierStore`, `SupplierEvent`, `SupplierStoreHandle` |
| `crates/vassl-suppliers/src/supplier_form.rs` | **New** — `SupplierForm`, create + edit modes |
| `crates/vassl-suppliers/src/supplier_list.rs` | **New** — `SupplierList` row component |
| `crates/vassl-suppliers/src/panel.rs` | **New** — `SupplierPanel` |
| `crates/vassl-app/Cargo.toml` | Add `vassl-suppliers` dependency |
| `crates/vassl-app/src/sidebar.rs` | Add `ActiveModule::Suppliers` |
| `crates/vassl-app/src/root.rs` | Add `suppliers_panel`, render in `ActiveModule::Suppliers` arm |

---

## Task 1: `Supplier` and `NewSupplier` types in vassl-core

**Files:**
- Create: `crates/vassl-core/src/supplier.rs`
- Modify: `crates/vassl-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vassl-core/src/supplier.rs` with tests first:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    pub id:             i64,
    pub name:           String,
    pub contact_person: Option<String>,
    pub email:          Option<String>,
    pub phone:          Option<String>,
    pub notes:          Option<String>,
    pub created_at:     String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSupplier {
    pub name:           String,
    pub contact_person: Option<String>,
    pub email:          Option<String>,
    pub phone:          Option<String>,
    pub notes:          Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplier_optional_fields_are_none_by_default() {
        let s = Supplier {
            id:             1,
            name:           "Acme Ltd".to_string(),
            contact_person: None,
            email:          None,
            phone:          None,
            notes:          None,
            created_at:     "2026-01-01T00:00:00Z".to_string(),
        };
        assert!(s.contact_person.is_none());
        assert!(s.email.is_none());
        assert_eq!(s.name, "Acme Ltd");
    }

    #[test]
    fn new_supplier_with_all_fields() {
        let ns = NewSupplier {
            name:           "Sony Electronics".to_string(),
            contact_person: Some("Jane Doe".to_string()),
            email:          Some("jane@sony.com".to_string()),
            phone:          Some("+1 555 0100".to_string()),
            notes:          Some("Primary camera supplier".to_string()),
        };
        assert_eq!(ns.name, "Sony Electronics");
        assert_eq!(ns.contact_person.as_deref(), Some("Jane Doe"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-core supplier
```
Expected: compile error — module not exported yet.

- [ ] **Step 3: Export in `vassl-core/src/lib.rs`**

Replace the full content of `crates/vassl-core/src/lib.rs`:

```rust
pub mod price_entry;
pub mod product;
pub mod project;
pub mod quotation;
pub mod supplier;

pub use price_entry::{NewPriceEntry, PriceEntry, PriceEntryError, selling_price};
pub use product::{AcquisitionType, NewProduct, NewStockEntry, Product, StockEntry};
pub use project::{NewProject, Project, ProjectStatus};
pub use quotation::{NewQuotationItem, Quotation, QuotationItem, QuotationStatus};
pub use supplier::{NewSupplier, Supplier};
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-core supplier
```
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-core/src/supplier.rs crates/vassl-core/src/lib.rs
git commit -m "feat(core): add Supplier and NewSupplier types"
```

---

## Task 2: New `vassl-suppliers` crate — scaffold + DB

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/vassl-suppliers/Cargo.toml`
- Create: `crates/vassl-suppliers/src/lib.rs`
- Create: `crates/vassl-suppliers/src/db.rs`

- [ ] **Step 1: Write the failing DB tests**

Create `crates/vassl-suppliers/src/db.rs`:

```rust
use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::Supplier;
use vassl_db::SharedDomain;

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
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
                .context("execute last_insert_rowid")?
                .ok_or_else(|| anyhow::anyhow!("no rowid after insert"))
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
            .context("execute update_supplier")
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
```

- [ ] **Step 2: Create `Cargo.toml` for the new crate**

Create `crates/vassl-suppliers/Cargo.toml`:

```toml
[package]
name    = "vassl-suppliers"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui.workspace       = true
anyhow.workspace     = true
tracing.workspace    = true
chrono.workspace     = true
sqlez.workspace      = true
vassl-core           = { path = "../vassl-core" }
vassl-db             = { path = "../vassl-db" }
vassl-ui             = { path = "../vassl-ui" }

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
db    = { path = "../db", features = ["test-support"] }
```

- [ ] **Step 3: Create minimal `lib.rs`**

Create `crates/vassl-suppliers/src/lib.rs`:

```rust
pub mod db;
pub mod panel;
pub mod store;
pub mod supplier_form;
pub mod supplier_list;

use gpui::{App, Entity};

pub use db::SupplierDb;
pub use store::{SupplierStore, SupplierStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<SupplierStore> = cx.new(SupplierStore::new);
    cx.set_global(SupplierStoreHandle(store));
}
```

- [ ] **Step 4: Add `vassl-suppliers` to the workspace**

Edit the root `Cargo.toml` — add `"crates/vassl-suppliers"` to the `members` list:

```toml
[workspace]
members = [
    "crates/vassl-core",
    "crates/vassl-db",
    "crates/vassl-app",
    "crates/vassl-inventory",
    "crates/vassl-quotations",
    "crates/vassl-pricebook",
    "crates/vassl-suppliers",
    "crates/vassl-ui",
    # Vendored from Zed monorepo
    "crates/collections",
    "crates/util",
    "crates/paths",
    "crates/release_channel",
    "crates/zed_env_vars",
    "crates/sqlez",
    "crates/sqlez_macros",
    "crates/db",
]
```

- [ ] **Step 5: Create placeholder panel, store, supplier_form, supplier_list files so the crate compiles**

Create `crates/vassl-suppliers/src/store.rs`:

```rust
use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::Supplier;
use crate::db::SupplierDb;

pub struct SupplierStore {
    pub suppliers:            Vec<Supplier>,
    pub selected_supplier_id: Option<i64>,
    pub loading:              bool,
}

pub struct SupplierStoreHandle(pub Entity<SupplierStore>);
impl Global for SupplierStoreHandle {}

#[derive(Debug)]
pub enum SupplierEvent { SuppliersLoaded }
impl EventEmitter<SupplierEvent> for SupplierStore {}

impl SupplierStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { suppliers: Vec::new(), selected_supplier_id: None, loading: false }
    }

    pub fn load_suppliers(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = SupplierDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_suppliers() })
                .await;
            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match result {
                    Ok(rows) => {
                        store.suppliers = rows;
                        cx.emit(SupplierEvent::SuppliersLoaded);
                    }
                    Err(e) => tracing::error!("load_suppliers failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_supplier(&mut self, id: i64, cx: &mut Context<Self>) {
        self.selected_supplier_id = Some(id);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplier_event_loaded_variant() {
        let ev = SupplierEvent::SuppliersLoaded;
        assert!(matches!(ev, SupplierEvent::SuppliersLoaded));
    }
}
```

Create placeholder files (empty modules compile; full content added in later tasks):

Create `crates/vassl-suppliers/src/supplier_form.rs`:

```rust
// Implemented in Task 4
```

Create `crates/vassl-suppliers/src/supplier_list.rs`:

```rust
// Implemented in Task 5
```

Create `crates/vassl-suppliers/src/panel.rs`:

```rust
// Implemented in Task 5
```

- [ ] **Step 6: Run DB tests**

```
cargo test -p vassl-suppliers
```
Expected: 4 DB tests + 1 store test PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-suppliers/ Cargo.toml crates/vassl-core/
git commit -m "feat(suppliers): new crate scaffold with SupplierDb, SupplierStore, Supplier types"
```

---

## Task 3: `SupplierForm` — create and edit modes

**Files:**
- Replace: `crates/vassl-suppliers/src/supplier_form.rs`

- [ ] **Step 1: Write the failing tests**

Replace `crates/vassl-suppliers/src/supplier_form.rs` with tests only:

```rust
fn validate_supplier_name(name: &str) -> Result<String, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_name() {
        assert!(validate_supplier_name("").is_err());
    }

    #[test]
    fn rejects_whitespace_only_name() {
        assert!(validate_supplier_name("   ").is_err());
    }

    #[test]
    fn accepts_valid_name() {
        let result = validate_supplier_name("  Acme Ltd  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Acme Ltd");  // trimmed
    }
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-suppliers supplier_form
```
Expected: FAIL (panics at `todo!()`).

- [ ] **Step 3: Implement the full `SupplierForm`**

Replace `crates/vassl-suppliers/src/supplier_form.rs` with:

```rust
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::Supplier;
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::db::SupplierDb;
use crate::store::SupplierStore;

actions!(supplier_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum SupplierFormEvent { Submitted, Cancelled }
impl EventEmitter<SupplierFormEvent> for SupplierForm {}

pub struct SupplierForm {
    store:          Entity<SupplierStore>,
    editing_id:     Option<i64>,
    pub name:       Entity<TextInput>,
    contact_person: Entity<TextInput>,
    email:          Entity<TextInput>,
    phone:          Entity<TextInput>,
    notes:          Entity<TextInput>,
    error:          Option<String>,
    focus_handle:   FocusHandle,
}

fn validate_supplier_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Name is required.".to_string()); }
    Ok(name)
}

impl SupplierForm {
    pub fn new(store: Entity<SupplierStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            editing_id:     None,
            name:           cx.new(|cx| TextInput::with_placeholder("e.g. Sony Electronics", cx)),
            contact_person: cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            email:          cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            phone:          cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            notes:          cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            error:          None,
            focus_handle:   cx.focus_handle(),
        }
    }

    pub fn edit(store: Entity<SupplierStore>, supplier: &Supplier, cx: &mut Context<Self>) -> Self {
        let name_f    = cx.new(|cx| TextInput::with_placeholder("e.g. Sony Electronics", cx));
        let contact_f = cx.new(|cx| TextInput::with_placeholder("optional", cx));
        let email_f   = cx.new(|cx| TextInput::with_placeholder("optional", cx));
        let phone_f   = cx.new(|cx| TextInput::with_placeholder("optional", cx));
        let notes_f   = cx.new(|cx| TextInput::with_placeholder("optional", cx));

        name_f.update(cx, |t, cx| t.set_text(supplier.name.clone(), cx));
        if let Some(v) = &supplier.contact_person {
            contact_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }
        if let Some(v) = &supplier.email {
            email_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }
        if let Some(v) = &supplier.phone {
            phone_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }
        if let Some(v) = &supplier.notes {
            notes_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }

        Self {
            store,
            editing_id:     Some(supplier.id),
            name:           name_f,
            contact_person: contact_f,
            email:          email_f,
            phone:          phone_f,
            notes:          notes_f,
            error:          None,
            focus_handle:   cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name_raw   = self.name.read(cx).text().to_string();
        let contact    = self.contact_person.read(cx).text().trim().to_string();
        let email      = self.email.read(cx).text().trim().to_string();
        let phone      = self.phone.read(cx).text().trim().to_string();
        let notes      = self.notes.read(cx).text().trim().to_string();
        let contact_op = if contact.is_empty() { None } else { Some(contact) };
        let email_op   = if email.is_empty()   { None } else { Some(email) };
        let phone_op   = if phone.is_empty()   { None } else { Some(phone) };
        let notes_op   = if notes.is_empty()   { None } else { Some(notes) };

        match validate_supplier_name(&name_raw) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok(name) => {
                let db         = SupplierDb::global(&**cx);
                let store      = self.store.clone();
                let editing_id = self.editing_id;

                cx.spawn(async move |this, cx| {
                    let result = if let Some(id) = editing_id {
                        db.update_supplier(id, &name, contact_op.as_deref(), email_op.as_deref(), phone_op.as_deref(), notes_op.as_deref()).await
                            .map(|_| ())
                    } else {
                        db.insert_supplier(&name, contact_op.as_deref(), email_op.as_deref(), phone_op.as_deref(), notes_op.as_deref()).await
                            .map(|_| ())
                    };

                    match result {
                        Err(e) => {
                            let msg = if e.to_string().contains("UNIQUE") {
                                "A supplier with this name already exists.".to_string()
                            } else {
                                format!("Save failed: {e}")
                            };
                            let _ = this.update(cx, |form, cx| { form.error = Some(msg); cx.notify(); });
                        }
                        Ok(()) => {
                            let _ = store.update(cx, |s, cx| s.load_suppliers(cx));
                            let _ = this.update(cx, |_, cx| cx.emit(SupplierFormEvent::Submitted));
                        }
                    }
                    Ok(())
                }).detach();
            }
        }
    }
}

impl Focusable for SupplierForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for SupplierForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c         = cx.global::<ThemeHandle>().0.clone();
        let name_f    = self.name.read(cx).focus_handle.is_focused(window);
        let contact_f = self.contact_person.read(cx).focus_handle.is_focused(window);
        let email_f   = self.email.read(cx).focus_handle.is_focused(window);
        let phone_f   = self.phone.read(cx).focus_handle.is_focused(window);
        let notes_f   = self.notes.read(cx).focus_handle.is_focused(window);
        let title     = if self.editing_id.is_some() { "Edit Supplier" } else { "New Supplier" };
        let save_label= if self.editing_id.is_some() { "Save Changes" } else { "Save Supplier" };

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("SupplierForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(SupplierFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.contact_person.read(cx).focus_handle.clone(),
                    this.email.read(cx).focus_handle.clone(),
                    this.phone.read(cx).focus_handle.clone(),
                    this.notes.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.contact_person.read(cx).focus_handle.clone(),
                    this.email.read(cx).focus_handle.clone(),
                    this.phone.read(cx).focus_handle.clone(),
                    this.notes.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(540.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1().text_size(px(13.)).text_color(rgb(c.text_default)).child(title))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Name"))
                                    .child(div().flex_1().child(text_field("", self.name.clone(), name_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Contact Person"))
                                    .child(div().flex_1().child(text_field("", self.contact_person.clone(), contact_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Email"))
                                    .child(div().flex_1().child(text_field("", self.email.clone(), email_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Phone"))
                                    .child(div().flex_1().child(text_field("", self.phone.clone(), phone_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_start().py(px(10.))
                                    .child(div().w(px(160.)).pt(px(6.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Notes"))
                                    .child(div().flex_1().h(px(64.)).child(text_field("", self.notes.clone(), notes_f, cx)))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(px(11.)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1().border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("sup-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| cx.emit(SupplierFormEvent::Cancelled)))
                                .child("Cancel"))
                            .child(div().id("sup-btn-save").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| this.submit(cx)))
                                .child(save_label))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_name() {
        assert!(validate_supplier_name("").is_err());
    }

    #[test]
    fn rejects_whitespace_only_name() {
        assert!(validate_supplier_name("   ").is_err());
    }

    #[test]
    fn accepts_valid_name() {
        let result = validate_supplier_name("  Acme Ltd  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Acme Ltd");
    }
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-suppliers supplier_form
```
Expected: PASS (3 tests).

- [ ] **Step 5: Run full test suite**

```
cargo test -p vassl-suppliers
```
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-suppliers/src/supplier_form.rs
git commit -m "feat(suppliers): SupplierForm with create and edit modes"
```

---

## Task 4: `SupplierList` and `SupplierPanel`

**Files:**
- Replace: `crates/vassl-suppliers/src/supplier_list.rs`
- Replace: `crates/vassl-suppliers/src/panel.rs`

- [ ] **Step 1: Write the failing test**

Replace `crates/vassl-suppliers/src/supplier_list.rs` with just the test + struct stub:

```rust
use vassl_core::Supplier;

pub fn format_supplier_row(s: &Supplier) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_supplier(name: &str, email: Option<&str>, phone: Option<&str>) -> Supplier {
        Supplier {
            id: 1, name: name.to_string(),
            contact_person: None,
            email: email.map(String::from),
            phone: phone.map(String::from),
            notes: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn supplier_row_shows_name_and_email() {
        let s = make_supplier("Acme Ltd", Some("a@acme.com"), None);
        let row = format_supplier_row(&s);
        assert!(row.contains("Acme Ltd"));
        assert!(row.contains("a@acme.com"));
    }

    #[test]
    fn supplier_row_no_email_shows_name_only() {
        let s = make_supplier("Beta Corp", None, None);
        let row = format_supplier_row(&s);
        assert!(row.contains("Beta Corp"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-suppliers supplier_list
```
Expected: FAIL (panics at `todo!()`).

- [ ] **Step 3: Implement `SupplierList`**

Replace `crates/vassl-suppliers/src/supplier_list.rs`:

```rust
use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};
use vassl_core::Supplier;
use vassl_ui::{ThemeColors, ThemeHandle};

use crate::store::SupplierStore;

pub struct SupplierList {
    store: Entity<SupplierStore>,
}

impl SupplierList {
    pub fn new(store: Entity<SupplierStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

pub fn format_supplier_row(s: &Supplier) -> String {
    let extra = match (&s.email, &s.phone) {
        (Some(e), Some(p)) => format!("  {e}  {p}"),
        (Some(e), None)    => format!("  {e}"),
        (None, Some(p))    => format!("  {p}"),
        (None, None)       => String::new(),
    };
    format!("{}{extra}", s.name)
}

impl Render for SupplierList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c     = cx.global::<ThemeHandle>().0.clone();
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child("Loading…")
                .into_any_element();
        }

        if store.suppliers.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_default))
                .child("No suppliers — add one to get started.")
                .into_any_element();
        }

        let selected = store.selected_supplier_id;
        let rows: Vec<_> = store.suppliers.iter().map(|s| {
            supplier_row(s, selected == Some(s.id), self.store.clone(), &c)
        }).collect();

        div()
            .id("supplier-list-scroll")
            .flex_1().flex().flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn supplier_row(s: &Supplier, selected: bool, store: Entity<SupplierStore>, c: &ThemeColors) -> impl IntoElement {
    let id     = s.id;
    let row_bg = if selected { c.surface_active } else { c.canvas_bg };

    div()
        .id(format!("supplier-{id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(7.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_supplier(id, cx));
            },
        )
        .child(
            div()
                .flex_1()
                .text_size(px(13.))
                .text_color(rgb(c.text_default))
                .child(s.name.clone())
        )
        .child(
            div()
                .w(px(180.))
                .text_size(px(12.))
                .text_color(rgb(c.text_muted))
                .child(s.email.clone().unwrap_or_default())
        )
        .child(
            div()
                .w(px(130.))
                .text_size(px(12.))
                .text_color(rgb(c.text_muted))
                .child(s.phone.clone().unwrap_or_default())
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_supplier(name: &str, email: Option<&str>, phone: Option<&str>) -> Supplier {
        Supplier {
            id: 1, name: name.to_string(),
            contact_person: None,
            email: email.map(String::from),
            phone: phone.map(String::from),
            notes: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn supplier_row_shows_name_and_email() {
        let s = make_supplier("Acme Ltd", Some("a@acme.com"), None);
        let row = format_supplier_row(&s);
        assert!(row.contains("Acme Ltd"));
        assert!(row.contains("a@acme.com"));
    }

    #[test]
    fn supplier_row_no_email_shows_name_only() {
        let s = make_supplier("Beta Corp", None, None);
        let row = format_supplier_row(&s);
        assert!(row.contains("Beta Corp"));
    }
}
```

- [ ] **Step 4: Implement `SupplierPanel`**

Replace `crates/vassl-suppliers/src/panel.rs`:

```rust
use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};
use vassl_ui::ThemeHandle;

use crate::store::SupplierStore;
use crate::supplier_form::{SupplierForm, SupplierFormEvent};
use crate::supplier_list::SupplierList;
use crate::SupplierStoreHandle;

pub struct SupplierPanel {
    store:         Entity<SupplierStore>,
    supplier_list: Entity<SupplierList>,
    form:          Option<Entity<SupplierForm>>,
    _form_sub:     Option<Subscription>,
}

impl SupplierPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store         = cx.global::<SupplierStoreHandle>().0.clone();
        let supplier_list = cx.new(|cx| SupplierList::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_suppliers(cx));
        Self { store, supplier_list, form: None, _form_sub: None }
    }

    fn open_new_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let form  = cx.new(|cx| SupplierForm::new(self.store.clone(), cx));
        let first = form.read(cx).name.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        self.wire_form_sub(form, cx);
    }

    fn open_edit_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let supplier = {
            let store = self.store.read(cx);
            let Some(id) = store.selected_supplier_id else { return; };
            store.suppliers.iter().find(|s| s.id == id).cloned()
        };
        let Some(supplier) = supplier else { return; };
        let form  = cx.new(|cx| SupplierForm::edit(self.store.clone(), &supplier, cx));
        let first = form.read(cx).name.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        self.wire_form_sub(form, cx);
    }

    fn wire_form_sub(&mut self, form: gpui::Entity<SupplierForm>, cx: &mut Context<Self>) {
        let sub = cx.subscribe(&form, |this, _form, ev: &SupplierFormEvent, cx| {
            match ev {
                SupplierFormEvent::Submitted | SupplierFormEvent::Cancelled => {
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

impl Render for SupplierPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c             = cx.global::<ThemeHandle>().0.clone();
        let has_selection = self.store.read(cx).selected_supplier_id.is_some();

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("sup-btn-new")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_new_form(window, cx);
                            }))
                            .child("+ New Supplier")
                    )
                    .child({
                        let mut btn = div()
                            .id("sup-btn-edit")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .child("Edit");
                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_edit_form(window, cx);
                                }));
                        }
                        btn
                    })
            )
            .child(self.supplier_list.clone());

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        root
    }
}
```

- [ ] **Step 5: Run tests**

```
cargo test -p vassl-suppliers supplier_list
```
Expected: PASS (2 tests).

- [ ] **Step 6: Run full suite**

```
cargo test -p vassl-suppliers
```
Expected: all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-suppliers/src/supplier_list.rs crates/vassl-suppliers/src/panel.rs
git commit -m "feat(suppliers): SupplierList and SupplierPanel with create/edit flow"
```

---

## Task 5: Wire suppliers into the app

**Files:**
- Modify: `crates/vassl-app/Cargo.toml`
- Modify: `crates/vassl-app/src/sidebar.rs`
- Modify: `crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write the failing test**

Add a test that the Suppliers sidebar module is distinct from others:

```rust
// In crates/vassl-app/src/sidebar.rs tests block, add:
#[test]
fn suppliers_module_is_distinct() {
    assert_ne!(ActiveModule::Suppliers, ActiveModule::Inventory);
    assert_ne!(ActiveModule::Suppliers, ActiveModule::PriceBook);
    assert_ne!(ActiveModule::Suppliers, ActiveModule::Settings);
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-app suppliers_module
```
Expected: compile error — `ActiveModule::Suppliers` doesn't exist.

- [ ] **Step 3: Add `vassl-suppliers` to `vassl-app/Cargo.toml`**

Add one line to `[dependencies]` in `crates/vassl-app/Cargo.toml`:

```toml
vassl-suppliers = { path = "../vassl-suppliers" }
```

- [ ] **Step 4: Add `ActiveModule::Suppliers` to sidebar**

Replace the full content of `crates/vassl-app/src/sidebar.rs`:

```rust
use gpui::{
    Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px, rgb,
};
use vassl_ui::ThemeHandle;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveModule {
    Inventory,
    Quotations,
    PriceBook,
    Suppliers,
    Settings,
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
        let c = cx.global::<ThemeHandle>().0.clone();
        let active = self.active;

        let make_btn = |module: ActiveModule, label: &'static str, id: &'static str| {
            let is_active = active == module;
            let bg = if is_active { rgb(c.surface_active) } else { rgb(c.surface_default) };
            let fg = if is_active { rgb(c.text_default)   } else { rgb(c.text_muted) };
            div()
                .id(id)
                .w(px(36.)).h(px(36.)).m(px(6.))
                .rounded(px(6.))
                .bg(bg).text_color(fg)
                .flex().items_center().justify_center()
                .cursor_pointer()
                .child(label)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, _window, cx| {
                    this.active = module;
                    cx.notify();
                }))
        };

        div()
            .w(px(48.)).h_full()
            .bg(rgb(c.sidebar_bg))
            .flex().flex_col().justify_between()
            .child(
                div().flex().flex_col()
                    .child(make_btn(ActiveModule::Inventory,  "I",  "btn-inventory"))
                    .child(make_btn(ActiveModule::Quotations, "Q",  "btn-quotations"))
                    .child(make_btn(ActiveModule::PriceBook,  "P",  "btn-pricebook"))
                    .child(make_btn(ActiveModule::Suppliers,  "S",  "btn-suppliers")),
            )
            .child(make_btn(ActiveModule::Settings, "⚙", "btn-settings"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_module_is_inventory() {
        assert_eq!(ActiveModule::Inventory, ActiveModule::Inventory);
    }

    #[test]
    fn modules_are_distinct() {
        assert_ne!(ActiveModule::Inventory,  ActiveModule::Quotations);
        assert_ne!(ActiveModule::Quotations, ActiveModule::PriceBook);
        assert_ne!(ActiveModule::Inventory,  ActiveModule::PriceBook);
    }

    #[test]
    fn settings_module_is_distinct() {
        assert_ne!(ActiveModule::Settings, ActiveModule::Inventory);
        assert_ne!(ActiveModule::Settings, ActiveModule::Quotations);
        assert_ne!(ActiveModule::Settings, ActiveModule::PriceBook);
    }

    #[test]
    fn suppliers_module_is_distinct() {
        assert_ne!(ActiveModule::Suppliers, ActiveModule::Inventory);
        assert_ne!(ActiveModule::Suppliers, ActiveModule::PriceBook);
        assert_ne!(ActiveModule::Suppliers, ActiveModule::Settings);
    }
}
```

- [ ] **Step 5: Wire `SupplierPanel` into `root.rs`**

Add to the imports at the top of `crates/vassl-app/src/root.rs`:

```rust
use vassl_suppliers::panel::SupplierPanel;
```

Add `suppliers_panel: Entity<SupplierPanel>` to the `VasslRoot` struct (alongside `pricebook_panel`).

In `VasslRoot::new`, initialize it:

```rust
let suppliers_panel = cx.new(SupplierPanel::new);
```

Add it to the `Self { ... }` initializer.

Add the `ActiveModule::Suppliers` arm to the `match active` block in `render()`:

```rust
ActiveModule::Suppliers  => content.child(self.suppliers_panel.clone()),
```

Add `OpenSuppliers` action handling (or simply rely on sidebar click — no keyboard shortcut needed for v1).

Also call `vassl_suppliers::init(cx)` in main — see Step 6.

- [ ] **Step 6: Update `main.rs` — init + keybindings + imports**

In `crates/vassl-app/src/main.rs`:

**Add import at the top** (alongside the other form action imports):

```rust
use vassl_suppliers::supplier_form::{EscapeForm as SupplierEscapeForm, TabField as SupplierTab, BackTabField as SupplierBackTab};
```

**Add `vassl_suppliers::init(cx)` after `vassl_pricebook::init(cx)`:**

```rust
vassl_inventory::init(cx);
vassl_quotations::init(cx);
vassl_pricebook::init(cx);
vassl_suppliers::init(cx);   // ← add this line
```

**Add keybindings** after the PriceEntryForm bindings block:

```rust
// SupplierForm escape + tab
KeyBinding::new("escape",    SupplierEscapeForm, Some("SupplierForm")),
KeyBinding::new("tab",       SupplierTab,        Some("SupplierForm")),
KeyBinding::new("shift-tab", SupplierBackTab,    Some("SupplierForm")),
```

- [ ] **Step 7: Run workspace tests**

```
cargo test --workspace
```
Expected: all tests PASS (previous 30+ tests + new supplier tests).

- [ ] **Step 8: Verify build**

```
cargo build --workspace
```
Expected: compiles cleanly.

- [ ] **Step 9: Commit**

```bash
git add crates/vassl-app/ crates/vassl-suppliers/
git commit -m "feat(app): wire SupplierPanel into sidebar and root"
```

---

## Done

All 5 tasks complete. Run the full test suite:

```
cargo test --workspace
```

Expected: all tests passing. The sidebar now has an "S" button that opens the Suppliers panel with create/edit supplier support.
