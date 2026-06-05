use std::collections::HashMap;
use std::path::PathBuf;

use vassl_inventory::db::InventoryDb;
use vassl_inventory::importer::{ImportRow, ImportSummary, parse_xlsx};
use vassl_pricebook::db::PriceBookDb;
use vassl_suppliers::db::SupplierDb;

pub async fn run_import(
    path:    PathBuf,
    inv_db:  InventoryDb,
    sup_db:  SupplierDb,
    pb_db:   PriceBookDb,
) -> anyhow::Result<ImportSummary> {
    let rows = parse_xlsx(&path)?;
    import_rows(rows, inv_db, sup_db, pb_db).await
}

pub async fn import_rows(
    rows:   Vec<ImportRow>,
    inv_db: InventoryDb,
    sup_db: SupplierDb,
    pb_db:  PriceBookDb,
) -> anyhow::Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    let mut supplier_cache: HashMap<String, i64> = HashMap::new();

    for row in &rows {
        // ── resolve or create supplier ───────────────────────────────────
        let supplier_id: Option<i64> = match &row.supplier_name {
            None => None,
            Some(sup_name) => {
                if let Some(&id) = supplier_cache.get(sup_name) {
                    Some(id)
                } else {
                    let existing = sup_db.list_suppliers()
                        .unwrap_or_default()
                        .into_iter()
                        .find(|s| s.name.eq_ignore_ascii_case(sup_name))
                        .map(|s| s.id);

                    let id = if let Some(id) = existing {
                        id
                    } else {
                        match sup_db.insert_supplier(sup_name, None, None, None, None).await {
                            Ok(id) => { summary.suppliers_created += 1; id }
                            Err(e) => {
                                summary.errors.push(format!("supplier '{}': {e}", sup_name));
                                continue;
                            }
                        }
                    };
                    supplier_cache.insert(sup_name.clone(), id);
                    Some(id)
                }
            }
        };

        // ── create or update product ─────────────────────────────────────
        let existing = inv_db.list_products().unwrap_or_default()
            .into_iter()
            .find(|p| p.sku == row.sku);

        let product_id = if let Some(existing) = existing {
            match inv_db.update_product(
                existing.id, &row.name, row.category.as_deref(), &row.unit,
                existing.min_stock_level, existing.description.as_deref(), supplier_id,
                row.model_number.as_deref(), row.part_number.as_deref(), row.duty_percent,
            ).await {
                Ok(_) => { summary.products_updated += 1; existing.id }
                Err(e) => {
                    summary.errors.push(format!("update '{}': {e}", row.sku));
                    continue;
                }
            }
        } else {
            match inv_db.insert_product(
                &row.sku, &row.name, row.category.as_deref(), &row.unit, 0.0,
                None, None, supplier_id,
                row.model_number.as_deref(), row.part_number.as_deref(), row.duty_percent,
            ).await {
                Ok(id) => { summary.products_created += 1; id }
                Err(e) => {
                    summary.errors.push(format!("insert '{}': {e}", row.sku));
                    continue;
                }
            }
        };

        // ── add price entry ──────────────────────────────────────────────
        if let Some(cost) = row.cost {
            let duty_usd = cost * row.duty_percent / 100.0;
            let selling  = vassl_core::selling_price(cost, duty_usd, row.markup).unwrap_or(0.0);
            match pb_db.insert_entry(
                product_id, 1.0, cost, duty_usd, row.markup, selling, None, &row.currency,
            ).await {
                Ok(_)  => summary.price_entries += 1,
                Err(e) => summary.errors.push(format!("price entry '{}': {e}", row.sku)),
            }
        }
    }

    Ok(summary)
}
