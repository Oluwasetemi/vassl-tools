# Price History Context Menu Design

**Goal:** Add a right-click context menu to product rows in both the Inventory and Price Book panels, showing contextual product information and two actions: Price History and Add Price Entry.

---

## Architecture

Context menu state is stored in each module's store (`InventoryStore`, `PriceBookStore`) as an optional `ContextMenuTarget`. This mirrors the existing `selected_product_id` pattern — UI state that drives rendering without a separate entity.

Right-click is captured in the list/table row components (`product_list.rs`, `price_table.rs`) via `on_mouse_down(MouseButton::Right, ...)`, which writes to the store. Each panel's `render()` reads the store and overlays the context menu when set.

Cross-crate coordination: `PriceBookPanel` handles both actions directly (same crate as `PriceHistoryPanel` and `PriceEntryForm`). `InventoryPanel` cannot depend on `vassl-pricebook` (circular), so it becomes an `EventEmitter` and sends `ShowPriceHistory` / `ShowPriceEntryForm` events to `VasslRoot`, which creates the panels.

The existing "History" tab in `PriceBookPanel` is unchanged.

---

## Context Menu Content

### In Inventory panel
```
[Product Name]
Stock: 12.0 pcs  (min 5.0)
──────────────────────────
Price History
Add Price Entry
```

The info line shows `current_stock` and `min_stock_level` from the already-loaded `InventoryStore`.

### In Price Book panel
```
[Product Name]
$100.00 + $10.00 → 30% → $143.00
──────────────────────────────────
Price History
Add Price Entry
```

The info line shows the latest `PriceEntry` fields from the already-loaded `PriceBookStore`. If no price entry exists, shows "No price set".

---

## Data Layer

No new DB queries or migrations needed. `PriceBookDb::list_entries_for_product` already exists and is used by `PriceHistoryPanel` on creation.

---

## New Component: `PriceHistoryPanel`

**File:** `vassl-pricebook/src/price_history.rs`

```rust
pub struct PriceHistoryPanel {
    product_name: String,
    entries: Vec<PriceEntry>,
}

pub enum PriceHistoryEvent { Dismissed }
impl EventEmitter<PriceHistoryEvent> for PriceHistoryPanel {}
```

- Constructor: `new(product_id: i64, product_name: String, cx: &mut Context<Self>)` — loads entries synchronously from `PriceBookDb` at construction time.
- Renders as a centered modal overlay (~620px wide) with a semi-transparent backdrop.
- Header: "Price History — {product_name}"
- Table columns: Date | Cost | +Duty | Markup% | Selling Price
- Each row: one `PriceEntry`, newest first (DB already returns them in this order).
- Footer: "{n} entries"
- Empty state: "No price history for this product."
- Dismisses on Esc (`EscapeModal` action) or left-click on backdrop.
- Emits `PriceHistoryEvent::Dismissed` on close.

---

## Context Menu State

```rust
pub struct ContextMenuTarget {
    pub product_id:   i64,
    pub product_name: String,
    pub x:            f32,
    pub y:            f32,
}
```

Added to `InventoryStore` and `PriceBookStore`:
```rust
pub context_menu: Option<ContextMenuTarget>,
```

Set by right-click in the row component. Cleared when any menu action is chosen or the backdrop is clicked.

---

## Context Menu Rendering

Each panel's `render()` checks `store.context_menu`. When set:

1. A full-screen transparent backdrop div captures left-click to clear the context menu.
2. An absolute-positioned menu div at `(x, y)` renders:
   - Header section: product name (bold) + info line (muted)
   - Separator
   - "Price History" item
   - "Add Price Entry" item
3. Both items trigger their respective action and clear the context menu.

Menu width: `px(220.)`. Positioned so it doesn't clip the right/bottom edges (no edge-detection needed for v1 — position at cursor as-is).

---

## Action: Price History

**From `PriceBookPanel`:** Creates `PriceHistoryPanel` inline. Panel owns:
```rust
price_history: Option<Entity<PriceHistoryPanel>>,
_price_history_sub: Option<Subscription>,
```
Subscribes to `PriceHistoryEvent::Dismissed` to clear the field.

**From `InventoryPanel`:** Emits `InventoryPanelEvent::ShowPriceHistory { product_id: i64, name: String }`. `VasslRoot` handles this by creating `PriceHistoryPanel` and subscribing to its `Dismissed` event.

---

## Action: Add Price Entry

**From `PriceBookPanel`:** Calls `open_form()` with the right-clicked product_id/name (same as existing "New Entry" button logic, but bypasses the `selected_product_id` requirement).

`open_form()` is refactored to accept an explicit `(product_id: i64, name: String)` instead of reading from `store.selected_product_id`, so both the toolbar button and context menu can use it.

**From `InventoryPanel`:** Emits `InventoryPanelEvent::ShowPriceEntryForm { product_id: i64, name: String }`. `VasslRoot` handles this by calling `pricebook_panel.update(cx, |p, cx| p.open_form_for(product_id, name, window, cx))`.

---

## Event Types

### `InventoryPanelEvent` (new)

```rust
pub enum InventoryPanelEvent {
    ShowPriceHistory  { product_id: i64, name: String },
    ShowPriceEntryForm { product_id: i64, name: String },
}
impl EventEmitter<InventoryPanelEvent> for InventoryPanel {}
```

`VasslRoot` subscribes to `InventoryPanel` at construction and handles both variants.

---

## File Map

| File | Change |
|---|---|
| `vassl-pricebook/src/price_history.rs` | **New** — `PriceHistoryPanel` + `PriceHistoryEvent` |
| `vassl-pricebook/src/lib.rs` | Export `pub mod price_history` |
| `vassl-inventory/src/store.rs` | Add `context_menu: Option<ContextMenuTarget>` and `clear_context_menu()` |
| `vassl-inventory/src/product_list.rs` | Add `on_mouse_down(Right, ...)` to product rows |
| `vassl-inventory/src/panel.rs` | Context menu overlay render + `EventEmitter<InventoryPanelEvent>` |
| `vassl-pricebook/src/store.rs` | Add `context_menu: Option<ContextMenuTarget>` and `clear_context_menu()` |
| `vassl-pricebook/src/price_table.rs` | Add `on_mouse_down(Right, ...)` to price rows |
| `vassl-pricebook/src/panel.rs` | Context menu overlay + own `PriceHistoryPanel` + refactor `open_form()` |
| `vassl-app/src/root.rs` | Subscribe to `InventoryPanel` events + own `price_history: Option<Entity<PriceHistoryPanel>>` |

---

## Testing

- Unit: `ContextMenuTarget` fields are set correctly from a right-click position
- Unit: `PriceHistoryPanel` with no entries renders empty state
- Unit: `PriceHistoryPanel` with entries renders correct row count
- Unit: `InventoryPanelEvent` variants carry the correct product_id and name
- Unit: `open_form_for(product_id, name)` opens form regardless of `selected_product_id`

---

## Out of Scope

- Edge detection / repositioning the context menu if it clips the window edge
- Keyboard navigation within the context menu
- Deleting or editing existing price history entries
- The existing "History" tab in `PriceBookPanel` is unchanged
