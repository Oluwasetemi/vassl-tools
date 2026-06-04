# Supplier Management Design

**Goal:** Add a structured Suppliers module — a new sidebar panel where users can create and edit supplier records (name, contact, email, phone, notes).

**Scope:** Internal supplier directory only. The existing `stock_entries.supplier TEXT` free-text field is left unchanged in this iteration; FK linkage is a follow-up.

---

## Architecture

New crate `vassl-suppliers` following the exact pattern of `vassl-inventory` and `vassl-pricebook`. No new cross-crate dependencies between domain crates — `vassl-app` depends on all of them and does the wiring.

```
crates/vassl-suppliers/
  Cargo.toml
  src/
    lib.rs
    db.rs
    store.rs
    panel.rs
    supplier_form.rs
    supplier_list.rs
```

---

## Data Layer

**New types in `vassl-core/src/supplier.rs`:**

```rust
pub struct Supplier {
    pub id:             i64,
    pub name:           String,
    pub contact_person: Option<String>,
    pub email:          Option<String>,
    pub phone:          Option<String>,
    pub notes:          Option<String>,
    pub created_at:     String,
}

pub struct NewSupplier {
    pub name:           String,
    pub contact_person: Option<String>,
    pub email:          Option<String>,
    pub phone:          Option<String>,
    pub notes:          Option<String>,
}
```

**Migration — new table in `SupplierDb`:**

```sql
CREATE TABLE IF NOT EXISTS suppliers (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT UNIQUE NOT NULL,
    contact_person TEXT,
    email          TEXT,
    phone          TEXT,
    notes          TEXT,
    created_at     TEXT NOT NULL
)
```

**`SupplierDb` methods:**
- `list_suppliers() -> Vec<Supplier>` — ordered by name
- `insert_supplier(name, contact, email, phone, notes) -> ()` — errors on duplicate name
- `update_supplier(id, name, contact, email, phone, notes) -> ()`

---

## Store

```rust
pub struct SupplierStore {
    pub suppliers:            Vec<Supplier>,
    pub selected_supplier_id: Option<i64>,
    pub loading:              bool,
}

pub enum SupplierEvent { SuppliersLoaded }
impl EventEmitter<SupplierEvent> for SupplierStore {}
```

Methods: `load_suppliers()` (async, same pattern as `load_products`), `select_supplier(id)`.

---

## UI

### Sidebar

Adds a sixth button "Su" for `ActiveModule::Suppliers` between PriceBook and Settings.

### SupplierList

Scrollable list of supplier rows. Each row shows:
```
[Supplier Name]   contact@email.com   +1 555 0100
```
Clicking selects the supplier (highlights row, populates detail area or enables Edit button).

### SupplierPanel

Toolbar: `[+ New Supplier]`  `[Edit]` (enabled when a supplier is selected)
Content: `SupplierList`

On "+ New Supplier": opens `SupplierForm` in create mode.
On "Edit": opens `SupplierForm` pre-filled with selected supplier.

### SupplierForm

Modal overlay (same pattern as `ProductForm`). Fields:

| Field | Type | Required |
|---|---|---|
| Name | TextInput | Yes |
| Contact Person | TextInput | No |
| Email | TextInput | No |
| Phone | TextInput | No |
| Notes | TextInput (tall) | No |

Tab order: Name → Contact Person → Email → Phone → Notes → (Save)

Validation: Name must not be empty. DB unique constraint on name — surface as form error.

Form modes:
- **Create**: `SupplierForm::new(store, cx)`
- **Edit**: `SupplierForm::edit(store, supplier, cx)` — pre-fills all fields, calls `update_supplier` on submit

Emits `SupplierFormEvent::Submitted | Cancelled` (same as `ProductFormEvent` pattern).

---

## Changes to Existing Files

| File | Change |
|---|---|
| `vassl-core/src/supplier.rs` | **New** — `Supplier`, `NewSupplier` |
| `vassl-core/src/lib.rs` | Export `pub mod supplier` + re-exports |
| `Cargo.toml` | Add `vassl-suppliers` to `[workspace]` members |
| `vassl-app/Cargo.toml` | Add `vassl-suppliers` dependency |
| `vassl-app/src/sidebar.rs` | Add `ActiveModule::Suppliers` button |
| `vassl-app/src/root.rs` | Add `suppliers_panel: Entity<SupplierPanel>`, render in `ActiveModule::Suppliers` arm |

---

## Testing

- Unit: `Supplier` and `NewSupplier` field access
- Unit: `SupplierEvent` variants
- Unit: `insert_supplier` round-trips through `list_suppliers`
- Unit: duplicate name returns error
- Unit: `update_supplier` changes fields
- Unit: `SupplierForm` name validation rejects empty string

---

## Out of Scope

- Delete supplier (no referential integrity to manage yet)
- Linking `stock_entries.supplier` text field to FK
- Supplier-specific products list
- Search/filter within supplier list
