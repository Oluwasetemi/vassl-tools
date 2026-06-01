# Product Description Field Design

**Goal:** Add an internal multi-line `description` field to products alongside the existing `notes` field.

**Scope:** Internal only — description appears in the product create/edit form, not in quotation line items or any customer-facing surface.

---

## Architecture

No new files. Three existing files modified, one new DB migration.

### Data Layer

**Migration** — add column to `products` table:
```sql
ALTER TABLE products ADD COLUMN description TEXT;
```

**`vassl-core/src/product.rs`** — add field to struct:
```rust
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub description: Option<String>,   // NEW
    pub notes: Option<String>,
    pub created_at: String,
}
```

**`vassl-inventory/src/db.rs`** — update all three query sites:
- `insert_product` — include `description` in `INSERT`
- `update_product` — include `description` in `UPDATE`
- `list_products` / `get_product` — include `description` in `SELECT`

### Form Layer

**`vassl-inventory/src/product_form.rs`**

Add `description: Entity<TextInput>` to `ProductForm` struct.

Render below the existing fields:
- Label: **"Description"**
- Placeholder: `"e.g. Wide-angle camera lens, 24mm, F/1.8, compatible with Sony E-mount"`
- Height: tall variant (~3× normal input height) to hint at multi-line use
- Optional — no validation required

Tab order (extended):
```
SKU → Name → Category → Unit → Min Stock → Description → (Save)
```

Add `TabField::Description` and `BackTabField::Description` to the existing keyboard action enum and match arms.

On save: pass `description` value through to `insert_product` / `update_product`.

---

## Testing

- Unit test: `description` round-trips through insert → select
- Unit test: `description = None` does not break existing products (migration is backward-compatible)
- Unit test: Tab order includes Description after Min Stock

---

## Out of Scope

- Description is not shown in the inventory product list row
- Description is not shown in quotation line items
- Description is not searchable (search is a separate subsystem)
- `notes` field is unchanged — it remains in the DB and struct but is not exposed in the form (pre-existing state)
