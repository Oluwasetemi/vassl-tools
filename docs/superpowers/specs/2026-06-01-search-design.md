# Search Design (Per-Module + Global)

**Goal:** Add real-time filtering to every list panel (Inventory, Price Book, Suppliers) and a Cmd+Shift+F global search overlay that searches across Products, Suppliers, and Projects.

---

## Architecture

Two independent features sharing one pattern:

**Per-module filter** — a filter `TextInput` in each panel's toolbar. The panel reads the query text directly from the input entity in `render()` (same approach as `CommandPalette`), stores it in the module's store, and the list component filters its rows against that query. No observers, no callbacks — pure GPUI reactive dependency tracking.

**Global search overlay** — a new `GlobalSearch` entity in `vassl-app` that reads all store globals, builds `SearchHit` results in render, shows them as a keyboard-navigable list (like `CommandPalette`), and emits `GlobalSearchEvent::Navigate(SearchHit)`. `VasslRoot` handles navigation by switching the sidebar module and selecting the item.

---

## Per-Module Filter

### Store changes (Inventory, PriceBook, Suppliers)

Each store gains one field and two methods:

```rust
pub search_query: String,

pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
    self.search_query = query;
    cx.notify();
}

// Returns references filtered by search_query against name/sku/category (Inventory),
// name/sku (PriceBook), or name/email/phone (Suppliers).
pub fn filtered_products(&self) -> Vec<&ProductWithStock> { … }
pub fn filtered_product_prices(&self) -> Vec<&ProductPrice> { … }
pub fn filtered_suppliers(&self) -> Vec<&Supplier> { … }
```

### Panel changes

Each panel gains:

```rust
search_input: Entity<TextInput>,
```

Initialised in `new()` with placeholder `"Filter…"`. In `render()`, the panel reads the input text and pushes it to the store:

```rust
let q = self.search_input.read(cx).text().to_string();
if q != self.store.read(cx).search_query {
    self.store.update(cx, |s, cx| s.set_search_query(q, cx));
}
```

The toolbar renders the filter input and a "×" clear button (visible only when query is non-empty):

```
[Filter…_________________________] [×]     [+ New …]
```

### List component changes

Each list component (`ProductList`, `PriceTable`, `SupplierList`) calls `filtered_products()` / `filtered_product_prices()` / `filtered_suppliers()` instead of reading `store.products` / etc. directly. Empty-state message when the filter matches nothing:

```
No results for "nvr".
```

---

## Global Search Overlay

**Keybinding:** `secondary-shift-f` (Cmd+Shift+F on macOS)

**Action:** `OpenGlobalSearch` in `VasslRoot` key context.

### `SearchHit` and `SearchResultKind`

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum SearchResultKind {
    Product  { id: i64, sku: String },
    Supplier { id: i64, email: Option<String> },
    Project  { id: i64, client: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub module: ActiveModule,
    pub kind:   SearchResultKind,
    pub label:  String,   // product name / supplier name / project name
    pub sub:    String,   // sku / email / client
}
```

### `GlobalSearch`

```rust
pub struct GlobalSearch {
    pub query:    Entity<TextInput>,
    selected_idx: usize,
    focus_handle: FocusHandle,
}

pub enum GlobalSearchEvent {
    Dismissed,
    Navigate(SearchHit),
}
impl EventEmitter<GlobalSearchEvent> for GlobalSearch {}
```

`build_hits(query, inventory, pricebook_or_suppliers)` — pure function, tested in isolation. Searches: products (from `InventoryStore`), suppliers (from `SupplierStore`), projects (from `QuotationStore`/globals). PriceBook data is intentionally excluded — products appear once under Inventory.

Hit ordering: Products first, then Suppliers, then Projects. Max 50 hits total.

### Rendering

Same layout as `CommandPalette`: backdrop + centred box, `on_mouse_down` on backdrop emits `Dismissed`. Keyboard: ↓/↑ navigate, Enter confirms, Esc dismisses.

### `VasslRoot` navigation handler

```rust
GlobalSearchEvent::Navigate(hit) => {
    this.sidebar.update(cx, |s, cx| { s.active = hit.module; cx.notify(); });
    match &hit.kind {
        SearchResultKind::Product  { id, .. } => {
            this.inventory_panel.update(cx, |p, cx| {
                p.store.update(cx, |s, cx| s.select_product(*id, cx));
            });
        }
        SearchResultKind::Supplier { id, .. } => {
            this.suppliers_panel.update(cx, |p, cx| {
                p.store.update(cx, |s, cx| s.select_supplier(*id, cx));
            });
        }
        SearchResultKind::Project  { id, .. } => {
            // navigate to Quotations; project selection handled by quotation panel
            let _ = id; // TODO: quotation panel select_project in follow-up
        }
    }
}
```

---

## File Map

| File | Change |
|---|---|
| `vassl-inventory/src/store.rs` | Add `search_query`, `set_search_query`, `filtered_products` |
| `vassl-inventory/src/product_list.rs` | Use `filtered_products()` instead of `products` |
| `vassl-inventory/src/panel.rs` | Add `search_input`, toolbar filter input + clear button |
| `vassl-pricebook/src/store.rs` | Add `search_query`, `set_search_query`, `filtered_product_prices` |
| `vassl-pricebook/src/price_table.rs` | Use `filtered_product_prices()` |
| `vassl-pricebook/src/panel.rs` | Add `search_input`, toolbar filter input + clear button |
| `vassl-suppliers/src/store.rs` | Add `search_query`, `set_search_query`, `filtered_suppliers` |
| `vassl-suppliers/src/supplier_list.rs` | Use `filtered_suppliers()` |
| `vassl-suppliers/src/panel.rs` | Add `search_input`, toolbar filter input + clear button |
| `vassl-app/src/global_search.rs` | **New** — `GlobalSearch`, `SearchHit`, `SearchResultKind`, `build_hits` |
| `vassl-app/src/root.rs` | Add `global_search` entity + sub + `EscapeModal` arm + render child |
| `vassl-app/src/main.rs` | Import `OpenGlobalSearch` + keybinding `secondary-shift-f` |

---

## Testing

- Unit: `filtered_products` returns all when query is empty
- Unit: `filtered_products` case-insensitive match on name, sku, category
- Unit: `filtered_products` returns empty slice when no match
- Unit: `filtered_product_prices` same coverage
- Unit: `filtered_suppliers` matches on name and email
- Unit: `build_hits` returns all when query is empty → empty vec (global search shows nothing until typed)
- Unit: `build_hits` matches product by name/sku
- Unit: `build_hits` matches supplier by name/email
- Unit: `build_hits` max 50 hits
- Unit: `GlobalSearchEvent` variants carry correct data

---

## Out of Scope

- Fuzzy matching (substring match is sufficient for v1)
- Highlighted match ranges in results
- Quotation project navigation (placeholder in VasslRoot handler)
- Per-module search persistence across sessions
- Debounce (GPUI renders are cheap; instant filtering is fine)
