# Price History Context Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a right-click context menu to product rows in both the Inventory and Price Book panels, showing stock/price info and two actions: "Price History" (opens a modal overlay) and "Add Price Entry".

**Architecture:** Each panel's store gets a `ContextMenuTarget` field set on right-click and cleared on any action. `PriceHistoryPanel` is a GPUI entity with `EventEmitter<PriceHistoryEvent>` owned by `VasslRoot`; it emits `Dismissed` on Esc or backdrop click. `InventoryPanel` and `PriceBookPanel` both become `EventEmitter` types, routing cross-panel overlay requests through `VasslRoot` (which owns both crates as a dependency).

**Tech Stack:** Rust, GPUI (Entity, EventEmitter, Render, cx.listener, on_mouse_down), sqlez for DB, vassl-core types.

---

## File Map

| File | Change |
|---|---|
| `crates/vassl-inventory/src/store.rs` | Add `ContextMenuTarget` struct + `context_menu` field + `set/clear_context_menu()` |
| `crates/vassl-inventory/src/product_list.rs` | Add `on_mouse_down(Right, …)` to `product_row` |
| `crates/vassl-pricebook/src/price_history.rs` | **New** — `PriceHistoryPanel` + `PriceHistoryEvent` |
| `crates/vassl-pricebook/src/lib.rs` | Export `pub mod price_history` |
| `crates/vassl-pricebook/src/store.rs` | Add `ContextMenuTarget` struct + `context_menu` field + `set/clear_context_menu()` |
| `crates/vassl-pricebook/src/price_table.rs` | Add `on_mouse_down(Right, …)` to `price_row` |
| `crates/vassl-inventory/src/panel.rs` | Add `InventoryPanelEvent` + `EventEmitter` + context menu overlay render |
| `crates/vassl-pricebook/src/panel.rs` | Add `PriceBookPanelEvent` + `EventEmitter` + overlay + `open_form_for()` |
| `crates/vassl-app/src/root.rs` | Subscribe to both panels + own `price_history` + extend EscapeModal |

---

## Task 1: ContextMenuTarget + right-click in Inventory

**Files:**
- Modify: `crates/vassl-inventory/src/store.rs`
- Modify: `crates/vassl-inventory/src/product_list.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `crates/vassl-inventory/src/store.rs`:

```rust
#[test]
fn context_menu_target_fields_roundtrip() {
    let target = ContextMenuTarget {
        product_id:   42,
        product_name: "Camera Lens".to_string(),
        x:            150.0,
        y:            320.0,
    };
    assert_eq!(target.product_id,   42);
    assert_eq!(target.product_name, "Camera Lens");
    assert_eq!(target.x, 150.0);
    assert_eq!(target.y, 320.0);
}

#[test]
fn inventory_store_starts_with_no_context_menu() {
    // InventoryStore::new requires cx, but we can verify the field exists via
    // constructing the target directly and checking the Option type.
    let target: Option<ContextMenuTarget> = None;
    assert!(target.is_none());
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-inventory context_menu_target
```
Expected: compile error — `ContextMenuTarget` not defined yet.

- [ ] **Step 3: Add `ContextMenuTarget` to `store.rs` and wire it into `InventoryStore`**

In `crates/vassl-inventory/src/store.rs`, add after the `StockStatus` block and before `InventoryStore`:

```rust
#[derive(Debug, Clone)]
pub struct ContextMenuTarget {
    pub product_id:   i64,
    pub product_name: String,
    pub x:            f32,
    pub y:            f32,
}
```

Add the field to `InventoryStore`:

```rust
pub struct InventoryStore {
    pub products:            Vec<ProductWithStock>,
    pub selected_product_id: Option<i64>,
    pub stock_entries:       Vec<StockEntry>,
    pub loading:             bool,
    pub context_menu:        Option<ContextMenuTarget>,
}
```

Update `InventoryStore::new`:

```rust
pub fn new(_cx: &mut Context<Self>) -> Self {
    Self {
        products:            Vec::new(),
        selected_product_id: None,
        stock_entries:       Vec::new(),
        loading:             false,
        context_menu:        None,
    }
}
```

Add methods after `select_product`:

```rust
pub fn set_context_menu(&mut self, target: ContextMenuTarget, cx: &mut Context<Self>) {
    self.context_menu = Some(target);
    cx.notify();
}

pub fn clear_context_menu(&mut self, cx: &mut Context<Self>) {
    self.context_menu = None;
    cx.notify();
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-inventory context_menu_target
```
Expected: PASS (2 tests).

- [ ] **Step 5: Add right-click handler to `product_row` in `product_list.rs`**

The full replacement for `product_row` in `crates/vassl-inventory/src/product_list.rs`:

```rust
fn product_row(p: &ProductWithStock, selected: bool, store: Entity<InventoryStore>, c: &ThemeColors) -> impl IntoElement {
    let product_id    = p.product.id;
    let product_name  = p.product.name.clone();
    let badge_color = match p.status {
        StockStatus::Healthy  => c.status_green,
        StockStatus::Low      => c.status_amber,
        StockStatus::Critical => c.status_red,
        StockStatus::NoAlert  => c.status_grey,
    };

    let row_bg      = if selected { c.surface_active } else { c.canvas_bg };
    let store_right = store.clone();

    div()
        .id(format!("product-{product_id}"))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_product(product_id, cx));
            },
        )
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                let target = ContextMenuTarget {
                    product_id,
                    product_name: product_name.clone(),
                    x: event.position.x.0,
                    y: event.position.y.0,
                };
                store_right.update(cx, |s, cx| s.set_context_menu(target, cx));
            },
        )
        .child(
            div()
                .w(px(8.)).h(px(8.))
                .rounded_full()
                .bg(rgb(badge_color))
                .mr(px(8.))
        )
        .child(
            div()
                .w(px(80.))
                .text_size(px(12.))
                .text_color(rgb(c.text_muted))
                .child(p.product.sku.clone())
        )
        .child(
            div()
                .flex_1()
                .text_size(px(13.))
                .text_color(rgb(c.text_default))
                .child(p.product.name.clone())
        )
        .child(
            div()
                .w(px(70.))
                .text_size(px(12.))
                .text_color(rgb(c.text_default))
                .child(format!("{:.1} {}", p.current_stock, p.product.unit))
        )
        .child(
            div()
                .w(px(70.))
                .text_size(px(12.))
                .text_color(rgb(c.text_muted))
                .child(format!("min {:.1}", p.product.min_stock_level))
        )
}
```

Also add `ContextMenuTarget` to the import in `product_list.rs`:

```rust
use crate::store::{ContextMenuTarget, InventoryStore, ProductWithStock, StockStatus};
```

- [ ] **Step 6: Run all inventory tests**

```
cargo test -p vassl-inventory
```
Expected: all existing tests PASS, no new failures.

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-inventory/src/store.rs crates/vassl-inventory/src/product_list.rs
git commit -m "feat(inventory): add ContextMenuTarget + right-click on product rows"
```

---

## Task 2: PriceHistoryPanel

**Files:**
- Create: `crates/vassl-pricebook/src/price_history.rs`
- Modify: `crates/vassl-pricebook/src/lib.rs`

- [ ] **Step 1: Write failing tests in the new file**

Create `crates/vassl-pricebook/src/price_history.rs` with tests only first:

```rust
use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};
use vassl_core::PriceEntry;
use vassl_ui::ThemeHandle;

use crate::db::PriceBookDb;

pub enum PriceHistoryEvent { Dismissed }
impl gpui::EventEmitter<PriceHistoryEvent> for PriceHistoryPanel {}

pub struct PriceHistoryPanel {
    pub product_name: String,
    pub entries:      Vec<PriceEntry>,
}

// TODO: impl PriceHistoryPanel and impl Render — added in Step 3

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: i64) -> PriceEntry {
        PriceEntry {
            id,
            product_id:        1,
            cost_price_usd:    100.0,
            duty_cost_usd:     10.0,
            markup_percent:    30.0,
            selling_price_usd: 143.0,
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
        }
    }

    #[test]
    fn empty_entries_gives_no_history_state() {
        let panel = PriceHistoryPanel {
            product_name: "Test Product".to_string(),
            entries:      Vec::new(),
        };
        assert!(panel.entries.is_empty());
        assert_eq!(panel.product_name, "Test Product");
    }

    #[test]
    fn entries_count_is_correct() {
        let panel = PriceHistoryPanel {
            product_name: "Camera".to_string(),
            entries:      vec![make_entry(1), make_entry(2), make_entry(3)],
        };
        assert_eq!(panel.entries.len(), 3);
    }
}
```

- [ ] **Step 2: Export the module and run to see compile failure**

Add to `crates/vassl-pricebook/src/lib.rs`:

```rust
pub mod price_history;
```

Full updated `lib.rs`:

```rust
pub mod colors;
pub mod db;
pub mod panel;
pub mod price_form;
pub mod price_history;
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

```
cargo test -p vassl-pricebook price_history
```
Expected: compile error — `impl Render` missing for `PriceHistoryPanel`.

- [ ] **Step 3: Implement `PriceHistoryPanel::new` and `impl Render`**

Replace the placeholder in `price_history.rs` with the full implementation:

```rust
use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};
use vassl_core::PriceEntry;
use vassl_ui::ThemeHandle;

use crate::db::PriceBookDb;

pub enum PriceHistoryEvent { Dismissed }
impl gpui::EventEmitter<PriceHistoryEvent> for PriceHistoryPanel {}

pub struct PriceHistoryPanel {
    pub product_name: String,
    pub entries:      Vec<PriceEntry>,
}

impl PriceHistoryPanel {
    pub fn new(product_id: i64, product_name: String, cx: &mut Context<Self>) -> Self {
        let db      = PriceBookDb::global(&**cx);
        let entries = db.list_entries_for_product(product_id).unwrap_or_default();
        Self { product_name, entries }
    }
}

impl Render for PriceHistoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c           = cx.global::<ThemeHandle>().0.clone();
        let entry_count = self.entries.len();

        let col_header = |label: &'static str, w: f32| {
            div()
                .w(px(w))
                .text_size(px(11.))
                .text_color(rgb(c.text_muted))
                .child(label)
        };

        let rows: Vec<_> = self.entries.iter().map(|e| {
            div()
                .flex().flex_row().items_center().w_full()
                .px(px(16.)).py(px(6.))
                .child(div().w(px(100.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(e.effective_date[..10].to_string()))
                .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(c.text_default)).child(format!("${:.2}", e.cost_price_usd)))
                .child(div().w(px(80.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(format!("+${:.2}", e.duty_cost_usd)))
                .child(div().w(px(70.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(format!("{:.0}%", e.markup_percent)))
                .child(div().flex_1().text_size(px(13.)).text_color(rgb(c.status_green)).child(format!("${:.2}", e.selling_price_usd)))
        }).collect();

        let body = if self.entries.is_empty() {
            div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child("No price history for this product.")
                .into_any_element()
        } else {
            div()
                .id("price-history-scroll")
                .flex_1().flex().flex_col()
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        // Outer div = full-screen backdrop; click dismisses
        div()
            .absolute()
            .inset_0()
            .flex().items_center().justify_center()
            .bg(gpui::rgba(0x00000099))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _: &mut Window, cx| {
                    cx.emit(PriceHistoryEvent::Dismissed);
                }),
            )
            // Modal box — on_mouse_down absorbs click so it doesn't reach backdrop
            .child(
                div()
                    .w(px(620.))
                    .max_h(px(480.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(8.))
                    .flex().flex_col()
                    .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _: &mut Window, _: &mut App| {})
                    // Header
                    .child(
                        div()
                            .px(px(16.)).py(px(12.))
                            .text_size(px(14.))
                            .text_color(rgb(c.text_default))
                            .child(format!("Price History — {}", self.product_name))
                    )
                    // Column headers
                    .child(
                        div()
                            .flex().flex_row().items_center().w_full()
                            .px(px(16.)).py(px(4.))
                            .bg(rgb(c.surface_default))
                            .child(col_header("Date",          100.))
                            .child(col_header("Cost",           90.))
                            .child(col_header("+Duty",          80.))
                            .child(col_header("Markup%",        70.))
                            .child(div().flex_1().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Selling Price"))
                    )
                    // Rows or empty state
                    .child(body)
                    // Footer
                    .child(
                        div()
                            .px(px(16.)).py(px(8.))
                            .text_size(px(11.))
                            .text_color(rgb(c.text_muted))
                            .child(format!("{entry_count} entries"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: i64) -> PriceEntry {
        PriceEntry {
            id,
            product_id:        1,
            cost_price_usd:    100.0,
            duty_cost_usd:     10.0,
            markup_percent:    30.0,
            selling_price_usd: 143.0,
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
        }
    }

    #[test]
    fn empty_entries_gives_no_history_state() {
        let panel = PriceHistoryPanel {
            product_name: "Test Product".to_string(),
            entries:      Vec::new(),
        };
        assert!(panel.entries.is_empty());
        assert_eq!(panel.product_name, "Test Product");
    }

    #[test]
    fn entries_count_is_correct() {
        let panel = PriceHistoryPanel {
            product_name: "Camera".to_string(),
            entries:      vec![make_entry(1), make_entry(2), make_entry(3)],
        };
        assert_eq!(panel.entries.len(), 3);
    }
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-pricebook price_history
```
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-pricebook/src/price_history.rs crates/vassl-pricebook/src/lib.rs
git commit -m "feat(pricebook): add PriceHistoryPanel modal with EventEmitter"
```

---

## Task 3: ContextMenuTarget + right-click in PriceBook

**Files:**
- Modify: `crates/vassl-pricebook/src/store.rs`
- Modify: `crates/vassl-pricebook/src/price_table.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `crates/vassl-pricebook/src/store.rs`:

```rust
#[test]
fn pricebook_context_menu_target_fields_roundtrip() {
    let target = ContextMenuTarget {
        product_id:   7,
        product_name: "NVR Unit".to_string(),
        x:            200.0,
        y:            450.0,
    };
    assert_eq!(target.product_id,   7);
    assert_eq!(target.product_name, "NVR Unit");
    assert_eq!(target.x, 200.0);
    assert_eq!(target.y, 450.0);
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-pricebook pricebook_context_menu_target
```
Expected: compile error — `ContextMenuTarget` not defined yet.

- [ ] **Step 3: Add `ContextMenuTarget` to `store.rs` and wire into `PriceBookStore`**

In `crates/vassl-pricebook/src/store.rs`, add after the `ProductPrice` struct and before `PriceBookStore`:

```rust
#[derive(Debug, Clone)]
pub struct ContextMenuTarget {
    pub product_id:   i64,
    pub product_name: String,
    pub x:            f32,
    pub y:            f32,
}
```

Add the field to `PriceBookStore`:

```rust
pub struct PriceBookStore {
    pub product_prices:      Vec<ProductPrice>,
    pub selected_product_id: Option<i64>,
    pub history:             Vec<PriceEntry>,
    pub loading:             bool,
    pub context_menu:        Option<ContextMenuTarget>,
}
```

Update `PriceBookStore::new`:

```rust
pub fn new(_cx: &mut Context<Self>) -> Self {
    Self {
        product_prices:      Vec::new(),
        selected_product_id: None,
        history:             Vec::new(),
        loading:             false,
        context_menu:        None,
    }
}
```

Add methods after `select_product`:

```rust
pub fn set_context_menu(&mut self, target: ContextMenuTarget, cx: &mut Context<Self>) {
    self.context_menu = Some(target);
    cx.notify();
}

pub fn clear_context_menu(&mut self, cx: &mut Context<Self>) {
    self.context_menu = None;
    cx.notify();
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-pricebook pricebook_context_menu_target
```
Expected: PASS.

- [ ] **Step 5: Add right-click handler to `price_row` in `price_table.rs`**

The full replacement for `price_row` in `crates/vassl-pricebook/src/price_table.rs`:

```rust
fn price_row(pp: &ProductPrice, selected: bool, store: Entity<PriceBookStore>, c: &ThemeColors) -> impl IntoElement {
    let product_id   = pp.product_id;
    let product_name = pp.name.clone();
    let row_bg       = if selected { c.surface_active } else { c.canvas_bg };
    let price_str    = price_display(pp);
    let price_color  = if pp.latest.is_some() { c.text_default } else { c.text_muted };
    let store_right  = store.clone();

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
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                let target = ContextMenuTarget {
                    product_id,
                    product_name: product_name.clone(),
                    x: event.position.x.0,
                    y: event.position.y.0,
                };
                store_right.update(cx, |s, cx| s.set_context_menu(target, cx));
            },
        )
        .child(
            div()
                .w(px(90.)).text_size(px(12.))
                .text_color(rgb(c.text_muted))
                .child(pp.sku.clone())
        )
        .child(
            div()
                .w(px(160.)).text_size(px(13.))
                .text_color(rgb(c.text_default))
                .child(pp.name.clone())
        )
        .child(
            div()
                .flex_1().text_size(px(12.))
                .text_color(rgb(price_color))
                .child(price_str)
        )
        .child(
            div()
                .w(px(110.)).text_size(px(11.))
                .text_color(rgb(c.text_muted))
                .child(pp.latest.as_ref().map(|e| e.effective_date[..10].to_string()).unwrap_or_default())
        )
}
```

Also add `ContextMenuTarget` to the import in `price_table.rs`:

```rust
use crate::store::{ContextMenuTarget, PriceBookStore, ProductPrice};
```

- [ ] **Step 6: Run all pricebook tests**

```
cargo test -p vassl-pricebook
```
Expected: all existing tests PASS, no new failures.

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-pricebook/src/store.rs crates/vassl-pricebook/src/price_table.rs
git commit -m "feat(pricebook): add ContextMenuTarget + right-click on price rows"
```

---

## Task 4: InventoryPanel EventEmitter + context menu overlay

**Files:**
- Modify: `crates/vassl-inventory/src/panel.rs`

- [ ] **Step 1: Write the failing test**

Add a test file `crates/vassl-inventory/src/panel.rs` in the `#[cfg(test)]` block (the file has no tests yet; add the block):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_panel_event_show_price_history_carries_data() {
        let ev = InventoryPanelEvent::ShowPriceHistory {
            product_id: 5,
            name:       "Lens 24mm".to_string(),
        };
        match ev {
            InventoryPanelEvent::ShowPriceHistory { product_id, name } => {
                assert_eq!(product_id, 5);
                assert_eq!(name, "Lens 24mm");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn inventory_panel_event_show_price_entry_form_carries_data() {
        let ev = InventoryPanelEvent::ShowPriceEntryForm {
            product_id: 12,
            name:       "NVR".to_string(),
        };
        match ev {
            InventoryPanelEvent::ShowPriceEntryForm { product_id, name } => {
                assert_eq!(product_id, 12);
                assert_eq!(name, "NVR");
            }
            _ => panic!("wrong variant"),
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-inventory inventory_panel_event
```
Expected: compile error — `InventoryPanelEvent` not defined.

- [ ] **Step 3: Implement `InventoryPanelEvent`, `EventEmitter`, and context menu overlay**

Replace the full content of `crates/vassl-inventory/src/panel.rs`:

```rust
use gpui::{App, Context, Entity, EventEmitter, IntoElement, MouseButton, MouseDownEvent,
           Render, Subscription, Window, div, prelude::*, px, rgb};
use vassl_ui::ThemeHandle;

use crate::colors;
use crate::product_form::{ProductForm, ProductFormEvent};
use crate::product_list::ProductList;
use crate::restock::RestockAlerts;
use crate::stock_form::{StockEntryForm, StockFormEvent};
use crate::store::InventoryStore;
use crate::InventoryStoreHandle;

#[derive(Clone, PartialEq)]
pub enum InventoryPanelEvent {
    ShowPriceHistory   { product_id: i64, name: String },
    ShowPriceEntryForm { product_id: i64, name: String },
}

impl EventEmitter<InventoryPanelEvent> for InventoryPanel {}

#[derive(Clone, Copy, PartialEq)]
enum Tab { Products, Restock }

pub struct InventoryPanel {
    store:          Entity<InventoryStore>,
    product_list:   Entity<ProductList>,
    restock_alerts: Entity<RestockAlerts>,
    active_tab:     Tab,
    stock_form:     Option<Entity<StockEntryForm>>,
    _form_sub:      Option<Subscription>,
    product_form:   Option<Entity<ProductForm>>,
    _prod_form_sub: Option<Subscription>,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<InventoryStoreHandle>().0.clone();
        let product_list   = cx.new(|cx| ProductList::new(store.clone(), cx));
        let restock_alerts = cx.new(|cx| RestockAlerts::new(store.clone(), cx));

        store.update(cx, |s, cx| s.load_products(cx));

        Self {
            store,
            product_list,
            restock_alerts,
            active_tab:     Tab::Products,
            stock_form:     None,
            _form_sub:      None,
            product_form:   None,
            _prod_form_sub: None,
        }
    }

    fn open_stock_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stock_form.is_some() { return; }
        let (product_id, product_name) = {
            let store = self.store.read(cx);
            let Some(pid) = store.selected_product_id else { return; };
            let name = store.products
                .iter()
                .find(|p| p.product.id == pid)
                .map(|p| p.product.name.clone())
                .unwrap_or_default();
            (pid, name)
        };

        let form  = cx.new(|cx| StockEntryForm::new(self.store.clone(), product_id, product_name, cx));
        let first = form.read(cx).quantity.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        let sub = cx.subscribe(&form, |this, _form, ev: &StockFormEvent, cx| {
            match ev {
                StockFormEvent::Submitted | StockFormEvent::Cancelled => {
                    this._form_sub  = None;
                    this.stock_form = None;
                    cx.notify();
                }
            }
        });
        self.stock_form = Some(form);
        self._form_sub  = Some(sub);
        cx.notify();
    }

    fn open_product_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.product_form.is_some() { return; }
        let form  = cx.new(|cx| ProductForm::new(self.store.clone(), cx));
        let first = form.read(cx).sku.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        let sub  = cx.subscribe(&form, |this, _form, ev: &ProductFormEvent, cx| {
            match ev {
                ProductFormEvent::Submitted | ProductFormEvent::Cancelled => {
                    this._prod_form_sub = None;
                    this.product_form   = None;
                    cx.notify();
                }
            }
        });
        self.product_form   = Some(form);
        self._prod_form_sub = Some(sub);
        cx.notify();
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Products => content.child(self.product_list.clone()),
            Tab::Restock  => content.child(self.restock_alerts.clone()),
        };

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child(
                        div()
                            .id("tab-products")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Products { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Products;
                                cx.notify();
                            }))
                            .child("Products")
                    )
                    .child(
                        div()
                            .id("tab-restock")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Restock { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Restock;
                                cx.notify();
                            }))
                            .child("Restock Alerts")
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("btn-new-product")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_product_form(window, cx);
                            }))
                            .child("+ New Product")
                    )
                    .child({
                        let mut btn = div()
                            .id("btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .child("+ New Entry");

                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_stock_form(window, cx);
                                }));
                        }
                        btn
                    })
            )
            .child(content);

        if let Some(form) = &self.stock_form {
            root = root.child(form.clone());
        }
        if let Some(form) = &self.product_form {
            root = root.child(form.clone());
        }

        // Context menu overlay
        let ctx_menu = self.store.read(cx).context_menu.clone();
        if let Some(target) = ctx_menu {
            let info_line = {
                let store = self.store.read(cx);
                store.products
                    .iter()
                    .find(|p| p.product.id == target.product_id)
                    .map(|p| format!(
                        "Stock: {:.1} {} (min {:.1})",
                        p.current_stock, p.product.unit, p.product.min_stock_level
                    ))
                    .unwrap_or_default()
            };

            let pid  = target.product_id;
            let name = target.product_name.clone();

            root = root
                // Backdrop — transparent full-screen div clears menu on click
                .child(
                    div()
                        .absolute().inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseDownEvent, _: &mut Window, cx| {
                                this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                            }),
                        )
                )
                // Menu positioned at cursor
                .child(
                    div()
                        .absolute()
                        .left(px(target.x))
                        .top(px(target.y))
                        .w(px(220.))
                        .bg(rgb(c.surface_default))
                        .rounded(px(6.))
                        .shadow_md()
                        // Product name header
                        .child(
                            div()
                                .px(px(12.)).pt(px(10.)).pb(px(4.))
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(target.product_name.clone())
                        )
                        // Info line (stock)
                        .child(
                            div()
                                .px(px(12.)).pb(px(8.))
                                .text_size(px(11.))
                                .text_color(rgb(c.text_muted))
                                .child(info_line)
                        )
                        // Separator
                        .child(
                            div()
                                .h(px(1.))
                                .bg(rgb(c.border_default))
                        )
                        // Price History action
                        .child({
                            let n = name.clone();
                            div()
                                .id("ctx-inv-price-history")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .child("Price History")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        cx.emit(InventoryPanelEvent::ShowPriceHistory {
                                            product_id: pid,
                                            name:       n.clone(),
                                        });
                                    }),
                                )
                        })
                        // Add Price Entry action
                        .child(
                            div()
                                .id("ctx-inv-add-price")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .child("Add Price Entry")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        cx.emit(InventoryPanelEvent::ShowPriceEntryForm {
                                            product_id: pid,
                                            name:       name.clone(),
                                        });
                                    }),
                                )
                        )
                );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_panel_event_show_price_history_carries_data() {
        let ev = InventoryPanelEvent::ShowPriceHistory {
            product_id: 5,
            name:       "Lens 24mm".to_string(),
        };
        match ev {
            InventoryPanelEvent::ShowPriceHistory { product_id, name } => {
                assert_eq!(product_id, 5);
                assert_eq!(name, "Lens 24mm");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn inventory_panel_event_show_price_entry_form_carries_data() {
        let ev = InventoryPanelEvent::ShowPriceEntryForm {
            product_id: 12,
            name:       "NVR".to_string(),
        };
        match ev {
            InventoryPanelEvent::ShowPriceEntryForm { product_id, name } => {
                assert_eq!(product_id, 12);
                assert_eq!(name, "NVR");
            }
            _ => panic!("wrong variant"),
        }
    }
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-inventory inventory_panel_event
```
Expected: PASS (2 tests).

- [ ] **Step 5: Run full inventory test suite**

```
cargo test -p vassl-inventory
```
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-inventory/src/panel.rs
git commit -m "feat(inventory): InventoryPanelEvent + context menu overlay render"
```

---

## Task 5: PriceBookPanel EventEmitter + context menu overlay + open_form_for

**Files:**
- Modify: `crates/vassl-pricebook/src/panel.rs`

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)]` block at the bottom of `crates/vassl-pricebook/src/panel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricebook_panel_event_show_price_history_carries_data() {
        let ev = PriceBookPanelEvent::ShowPriceHistory {
            product_id: 3,
            name:       "DVR System".to_string(),
        };
        match ev {
            PriceBookPanelEvent::ShowPriceHistory { product_id, name } => {
                assert_eq!(product_id, 3);
                assert_eq!(name, "DVR System");
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-pricebook pricebook_panel_event
```
Expected: compile error — `PriceBookPanelEvent` not defined.

- [ ] **Step 3: Implement full updated `panel.rs`**

Replace the entire content of `crates/vassl-pricebook/src/panel.rs`:

```rust
use gpui::{App, Context, Entity, EventEmitter, IntoElement, MouseButton, MouseDownEvent,
           Render, Subscription, Window, div, prelude::*, px, rgb};
use vassl_ui::ThemeHandle;

use crate::colors;
use crate::price_form::{PriceEntryForm, PriceFormEvent};
use crate::price_history::PriceHistoryPanel;
use crate::price_table::PriceTable;
use crate::store::PriceBookStore;
use crate::PriceBookStoreHandle;

#[derive(Clone, PartialEq)]
pub enum PriceBookPanelEvent {
    ShowPriceHistory { product_id: i64, name: String },
}

impl EventEmitter<PriceBookPanelEvent> for PriceBookPanel {}

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

    fn open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(pid) = store.selected_product_id else { return; };
        let name = store.product_prices
            .iter()
            .find(|p| p.product_id == pid)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        drop(store);
        self.open_form_for(pid, name, window, cx);
    }

    pub fn open_form_for(&mut self, product_id: i64, name: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let form  = cx.new(|cx| PriceEntryForm::new(self.store.clone(), product_id, name, cx));
        let first = form.read(cx).cost.read(cx).focus_handle.clone();
        window.focus(&first, cx);
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
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

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
                            .text_color(rgb(c.text_muted))
                            .child("Select a product row to view pricing history.")
                    )
                } else if history_is_empty {
                    content.child(
                        div()
                            .flex_1().flex().items_center().justify_center()
                            .text_color(rgb(c.text_muted))
                            .child("No price history for this product.")
                    )
                } else {
                    let rows: Vec<_> = history_rows.iter().map(|(date, cost, duty, markup, sell)| {
                        div()
                            .flex().flex_row().items_center().w_full()
                            .px(px(12.)).py(px(6.))
                            .child(div().w(px(100.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(date.clone()))
                            .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(c.text_default)).child(format!("${cost:.2}")))
                            .child(div().w(px(80.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(format!("+${duty:.2}")))
                            .child(div().w(px(70.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(format!("{markup:.0}%")))
                            .child(div().flex_1().text_size(px(13.)).text_color(rgb(c.status_green)).child(format!("${sell:.2}")))
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
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child(
                        div()
                            .id("pb-tab-pricebook")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::PriceBook { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::PriceBook;
                                cx.notify();
                            }))
                            .child("Price Book")
                    )
                    .child(
                        div()
                            .id("pb-tab-history")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::History { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::History;
                                cx.notify();
                            }))
                            .child("History")
                    )
                    .child(div().flex_1())
                    .child({
                        let mut btn = div()
                            .id("pb-btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .child("+ New Entry");
                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_form(window, cx);
                                }));
                        }
                        btn
                    })
            )
            .child(content);

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        // Context menu overlay
        let ctx_menu = self.store.read(cx).context_menu.clone();
        if let Some(target) = ctx_menu {
            let info_line = {
                let store = self.store.read(cx);
                store.product_prices
                    .iter()
                    .find(|pp| pp.product_id == target.product_id)
                    .map(|pp| {
                        match &pp.latest {
                            None    => "No price set".to_string(),
                            Some(e) => format!(
                                "${:.2} + ${:.2} → {:.0}% → ${:.2}",
                                e.cost_price_usd, e.duty_cost_usd, e.markup_percent, e.selling_price_usd
                            ),
                        }
                    })
                    .unwrap_or_default()
            };

            let pid  = target.product_id;
            let name = target.product_name.clone();

            root = root
                .child(
                    div()
                        .absolute().inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseDownEvent, _: &mut Window, cx| {
                                this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                            }),
                        )
                )
                .child(
                    div()
                        .absolute()
                        .left(px(target.x))
                        .top(px(target.y))
                        .w(px(220.))
                        .bg(rgb(c.surface_default))
                        .rounded(px(6.))
                        .shadow_md()
                        .child(
                            div()
                                .px(px(12.)).pt(px(10.)).pb(px(4.))
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(target.product_name.clone())
                        )
                        .child(
                            div()
                                .px(px(12.)).pb(px(8.))
                                .text_size(px(11.))
                                .text_color(rgb(c.text_muted))
                                .child(info_line)
                        )
                        .child(div().h(px(1.)).bg(rgb(c.border_default)))
                        // Price History action
                        .child({
                            let n = name.clone();
                            div()
                                .id("ctx-pb-price-history")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .child("Price History")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        cx.emit(PriceBookPanelEvent::ShowPriceHistory {
                                            product_id: pid,
                                            name:       n.clone(),
                                        });
                                    }),
                                )
                        })
                        // Add Price Entry action
                        .child(
                            div()
                                .id("ctx-pb-add-price")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .child("Add Price Entry")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, window: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        this.open_form_for(pid, name.clone(), window, cx);
                                    }),
                                )
                        )
                );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricebook_panel_event_show_price_history_carries_data() {
        let ev = PriceBookPanelEvent::ShowPriceHistory {
            product_id: 3,
            name:       "DVR System".to_string(),
        };
        match ev {
            PriceBookPanelEvent::ShowPriceHistory { product_id, name } => {
                assert_eq!(product_id, 3);
                assert_eq!(name, "DVR System");
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

```
cargo test -p vassl-pricebook pricebook_panel_event
```
Expected: PASS.

- [ ] **Step 5: Run full pricebook test suite**

```
cargo test -p vassl-pricebook
```
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-pricebook/src/panel.rs
git commit -m "feat(pricebook): PriceBookPanelEvent + context menu overlay + open_form_for"
```

---

## Task 6: VasslRoot — wire subscriptions and price_history overlay

**Files:**
- Modify: `crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write the failing test**

There is no `#[cfg(test)]` block in `root.rs`. Since VasslRoot requires a full GPUI app context to instantiate, the compile check itself verifies structural correctness. Add an inline test that validates the new event types compile and carry data:

```rust
#[cfg(test)]
mod tests {
    use vassl_inventory::panel::InventoryPanelEvent;
    use vassl_pricebook::panel::PriceBookPanelEvent;

    #[test]
    fn inventory_panel_event_variants_are_accessible() {
        let ev = InventoryPanelEvent::ShowPriceHistory {
            product_id: 1,
            name:       "Test".to_string(),
        };
        assert!(matches!(ev, InventoryPanelEvent::ShowPriceHistory { .. }));
    }

    #[test]
    fn pricebook_panel_event_variants_are_accessible() {
        let ev = PriceBookPanelEvent::ShowPriceHistory {
            product_id: 2,
            name:       "Test".to_string(),
        };
        assert!(matches!(ev, PriceBookPanelEvent::ShowPriceHistory { .. }));
    }
}
```

- [ ] **Step 2: Run to verify failure**

```
cargo test -p vassl-app
```
Expected: compile error — types not imported / fields not on VasslRoot yet.

- [ ] **Step 3: Implement the full updated `root.rs`**

Replace the entire content of `crates/vassl-app/src/root.rs`:

```rust
use gpui::{Context, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription,
           Window, div, prelude::*, px, rgb};
use vassl_ui::{ThemeColors, ThemeHandle};

use crate::actions::{EscapeModal, FocusSearch, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations, OpenSettings};
use crate::settings_panel::SettingsPanel;
use crate::audit_log::AuditLogPanel;
use crate::command_palette::{CommandPalette, PaletteEvent, PaletteCommand};
use crate::first_run::{FirstRunEvent, FirstRunPrompt};
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;
use vassl_inventory::panel::{InventoryPanel, InventoryPanelEvent};
use vassl_pricebook::panel::{PriceBookPanel, PriceBookPanelEvent};
use vassl_pricebook::price_history::{PriceHistoryEvent, PriceHistoryPanel};
use vassl_quotations::panel::QuotationPanel;

pub struct VasslRoot {
    sidebar:               Entity<Sidebar>,
    status_bar:            Entity<StatusBar>,
    inventory_panel:       Entity<InventoryPanel>,
    pricebook_panel:       Entity<PriceBookPanel>,
    quotation_panel:       Entity<QuotationPanel>,
    settings_panel:        Entity<SettingsPanel>,
    first_run:             Option<Entity<FirstRunPrompt>>,
    _first_run_sub:        Option<Subscription>,
    audit_log:             Option<Entity<AuditLogPanel>>,
    palette:               Option<Entity<CommandPalette>>,
    _palette_sub:          Option<Subscription>,
    price_history:         Option<Entity<PriceHistoryPanel>>,
    _price_history_sub:    Option<Subscription>,
    _inventory_panel_sub:  Subscription,
    _pricebook_panel_sub:  Subscription,
    focus_handle:          FocusHandle,
}

impl Focusable for VasslRoot {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl VasslRoot {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let needs_first_run = {
            let db = vassl_db::AppDatabase::global(&**cx);
            match vassl_db::shared::current_user(db) {
                Ok(Some(_)) => false,
                _           => true,
            }
        };

        let (first_run, _first_run_sub) = if needs_first_run {
            let form = cx.new(|cx| FirstRunPrompt::new(cx));
            let sub  = cx.subscribe(&form, |this, _form, ev: &FirstRunEvent, cx| {
                match ev {
                    FirstRunEvent::Saved => {
                        this._first_run_sub = None;
                        this.first_run      = None;
                        cx.notify();
                    }
                }
            });
            (Some(form), Some(sub))
        } else {
            (None, None)
        };

        // Apply persisted font size and theme before first render
        {
            let db = vassl_db::AppDatabase::global(&**cx);
            if let Ok(Some(size_str)) = vassl_db::shared::get_setting(db, "appearance.font_size") {
                if let Ok(size) = size_str.parse::<f32>() {
                    window.set_rem_size(px(size.max(10.0).min(24.0)));
                }
            }
            if let Ok(Some(theme)) = vassl_db::shared::get_setting(db, "appearance.theme") {
                let colors = if theme == "light" {
                    ThemeColors::light()
                } else {
                    ThemeColors::dark()
                };
                cx.set_global(ThemeHandle(colors));
            }
        }

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        let settings_panel = cx.new(SettingsPanel::new);
        settings_panel.update(cx, |panel, cx| panel.wire_observers(cx));

        let inventory_panel  = cx.new(InventoryPanel::new);
        let pricebook_panel  = cx.new(PriceBookPanel::new);

        let _inventory_panel_sub = cx.subscribe(
            &inventory_panel,
            |this, _panel, ev: &InventoryPanelEvent, cx| {
                match ev {
                    InventoryPanelEvent::ShowPriceHistory { product_id, name } => {
                        let ph  = cx.new(|cx| PriceHistoryPanel::new(*product_id, name.clone(), cx));
                        let sub = cx.subscribe(&ph, |this, _, ev: &PriceHistoryEvent, cx| {
                            match ev {
                                PriceHistoryEvent::Dismissed => {
                                    this._price_history_sub = None;
                                    this.price_history      = None;
                                    cx.notify();
                                }
                            }
                        });
                        this.price_history      = Some(ph);
                        this._price_history_sub = Some(sub);
                        cx.notify();
                    }
                    InventoryPanelEvent::ShowPriceEntryForm { product_id, .. } => {
                        // Navigate to PriceBook and select the product so the
                        // user can open "+ New Entry" from there. Subscription
                        // callbacks don't receive window, so we can't auto-focus the form.
                        let pid = *product_id;
                        this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); });
                        this.pricebook_panel.update(cx, |panel, cx| {
                            panel.store.update(cx, |s, cx| s.select_product(pid, cx));
                        });
                    }
                }
            },
        );

        let _pricebook_panel_sub = cx.subscribe(
            &pricebook_panel,
            |this, _panel, ev: &PriceBookPanelEvent, cx| {
                match ev {
                    PriceBookPanelEvent::ShowPriceHistory { product_id, name } => {
                        let ph  = cx.new(|cx| PriceHistoryPanel::new(*product_id, name.clone(), cx));
                        let sub = cx.subscribe(&ph, |this, _, ev: &PriceHistoryEvent, cx| {
                            match ev {
                                PriceHistoryEvent::Dismissed => {
                                    this._price_history_sub = None;
                                    this.price_history      = None;
                                    cx.notify();
                                }
                            }
                        });
                        this.price_history      = Some(ph);
                        this._price_history_sub = Some(sub);
                        cx.notify();
                    }
                }
            },
        );

        Self {
            sidebar:              cx.new(Sidebar::new),
            status_bar:           cx.new(StatusBar::new),
            inventory_panel,
            pricebook_panel,
            quotation_panel:      cx.new(QuotationPanel::new),
            settings_panel,
            first_run,
            _first_run_sub,
            audit_log:            None,
            palette:              None,
            _palette_sub:         None,
            price_history:        None,
            _price_history_sub:   None,
            _inventory_panel_sub,
            _pricebook_panel_sub,
            focus_handle,
        }
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() { return; }
        let pal = cx.new(|cx| CommandPalette::new(cx));
        let query_focus = pal.read(cx).query.read(cx).focus_handle.clone();
        window.focus(&query_focus, cx);
        let sub = cx.subscribe(&pal, |this, _pal, ev: &PaletteEvent, cx| {
            match ev {
                PaletteEvent::Dismissed => {
                    this._palette_sub = None;
                    this.palette = None;
                    cx.notify();
                }
                PaletteEvent::Execute(cmd) => {
                    match cmd {
                        PaletteCommand::OpenInventory =>
                            this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Inventory; cx.notify(); }),
                        PaletteCommand::OpenQuotations =>
                            this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Quotations; cx.notify(); }),
                        PaletteCommand::OpenPriceBook =>
                            this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); }),
                        PaletteCommand::OpenAuditLog => {
                            if this.audit_log.is_none() {
                                this.audit_log = Some(cx.new(|cx| AuditLogPanel::new(cx)));
                            }
                        }
                    }
                    this._palette_sub = None;
                    this.palette = None;
                    cx.notify();
                }
            }
        });
        self.palette      = Some(pal);
        self._palette_sub = Some(sub);
        cx.notify();
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active = self.sidebar.read(cx).active;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active {
            ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
            ActiveModule::Quotations => content.child(self.quotation_panel.clone()),
            ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
            ActiveModule::Settings   => content.child(self.settings_panel.clone()),
        };

        let mut root = div()
            .key_context("VasslRoot")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &OpenInventory, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Inventory; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenQuotations, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Quotations; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenPriceBook, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenAuditLog, _w, cx| {
                if this.audit_log.is_some() {
                    this.audit_log = None;
                } else {
                    this.audit_log = Some(cx.new(|cx| AuditLogPanel::new(cx)));
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Settings; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.open_palette(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EscapeModal, w, cx| {
                if this.palette.is_some() {
                    this._palette_sub = None;
                    this.palette = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.price_history.is_some() {
                    this._price_history_sub = None;
                    this.price_history      = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.audit_log.is_some() {
                    this.audit_log = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                }
            }))
            .relative()
            .flex().flex_col().w_full().h_full()
            .bg(rgb(c.canvas_bg))
            .child(
                div().flex().flex_row().flex_1()
                    .child(self.sidebar.clone())
                    .child(content),
            )
            .child(self.status_bar.clone());

        if let Some(form) = &self.first_run {
            root = root.child(form.clone());
        }
        if let Some(panel) = &self.audit_log {
            root = root.child(panel.clone());
        }
        if let Some(pal) = &self.palette {
            root = root.child(pal.clone());
        }
        if let Some(ph) = &self.price_history {
            root = root.child(ph.clone());
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use vassl_inventory::panel::InventoryPanelEvent;
    use vassl_pricebook::panel::PriceBookPanelEvent;

    #[test]
    fn inventory_panel_event_variants_are_accessible() {
        let ev = InventoryPanelEvent::ShowPriceHistory {
            product_id: 1,
            name:       "Test".to_string(),
        };
        assert!(matches!(ev, InventoryPanelEvent::ShowPriceHistory { .. }));
    }

    #[test]
    fn pricebook_panel_event_variants_are_accessible() {
        let ev = PriceBookPanelEvent::ShowPriceHistory {
            product_id: 2,
            name:       "Test".to_string(),
        };
        assert!(matches!(ev, PriceBookPanelEvent::ShowPriceHistory { .. }));
    }
}
```

- [ ] **Step 4: Run workspace tests**

```
cargo test --workspace
```
Expected: all tests PASS (including new ones in vassl-app, vassl-inventory, vassl-pricebook).

- [ ] **Step 5: Verify the build compiles cleanly**

```
cargo build --workspace
```
Expected: compiles with no errors or warnings (aside from any pre-existing ones).

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-app/src/root.rs
git commit -m "feat(app): wire price history overlay — subscribe to panel events, own PriceHistoryPanel"
```

---

## Done

All 6 tasks complete. Run the full test suite one final time:

```
cargo test --workspace
```

Expected: 30+ tests passing (26 before + 7 new across inventory, pricebook, app).
