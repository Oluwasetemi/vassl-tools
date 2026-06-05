use anyhow::{Context, bail};
use calamine::{Data, Reader, Xlsx, open_workbook};
use std::path::Path;

// ── Column header names ────────────────────────────────────────────────────
// Each constant is a comma-separated priority list matched case-insensitively.
// The first variant found in the sheet's header row wins.
// Empty string ("") marks an optional column — always returns None if not found.
pub mod columns {
    pub const SKU:          &[&str] = &[];
    pub const NAME:         &[&str] = &["Product Description", "Item Description", "Description", "Product", "PRODUCT DESCRIPTION"];
    pub const CATEGORY:     &[&str] = &["Type", "Category"];
    pub const MODEL_NUMBER: &[&str] = &["Model #", "Model Number", "Model"];
    pub const PART_NUMBER:  &[&str] = &["Part Number", "Part #", "PART#", "PART #"];
    pub const DUTY_PERCENT: &[&str] = &["DUTY", "Duty %", "Duty"];
    pub const MARKUP:       &[&str] = &["MU", "Markup", "Markup %", "MARKUP"];
    pub const COST:         &[&str] = &["Cost Price USD", "Unit Cost USD", "Cost Price", "Cost", "COST"];
    pub const CURRENCY:     &[&str] = &[];
    pub const SUPPLIER:     &[&str] = &["Manufacturer", "Manufacturer/ Vendor", "Supplier", "Vendor"];
    pub const UNIT:         &[&str] = &["Unit", "UNIT"];

    // true  → duty is a fraction (0.425 = 42.5%)
    // false → duty is already a percent (42.5)
    pub const DUTY_IS_FRACTION: bool = false;

    pub const DEFAULT_UNIT:     &str = "pcs";
    pub const DEFAULT_CURRENCY: &str = "JMD";
}
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ImportRow {
    pub sku:           String,
    pub name:          String,
    pub category:      Option<String>,
    pub model_number:  Option<String>,
    pub part_number:   Option<String>,
    pub duty_percent:  f64,
    pub markup:        f64,
    pub cost:          Option<f64>,
    pub currency:      String,
    pub supplier_name: Option<String>,
    pub unit:          String,
}

#[derive(Debug, Default, Clone)]
pub struct ImportSummary {
    pub products_created:  usize,
    pub products_updated:  usize,
    pub suppliers_created: usize,
    pub price_entries:     usize,
    pub errors:            Vec<String>,
}

fn cell_str(cell: &Data) -> Option<String> {
    match cell {
        Data::String(s) => {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        }
        Data::Float(f) => Some(f.to_string()),
        Data::Int(i)   => Some(i.to_string()),
        _ => None,
    }
}

fn cell_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(f) => Some(*f),
        Data::Int(i)   => Some(*i as f64),
        Data::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn get_str(row: &[Data], col: Option<usize>) -> Option<String> {
    col.and_then(|i| row.get(i)).and_then(cell_str)
}

fn get_f64(row: &[Data], col: Option<usize>) -> Option<f64> {
    col.and_then(|i| row.get(i)).and_then(cell_f64)
}

pub fn parse_xlsx(path: &Path) -> anyhow::Result<Vec<ImportRow>> {
    let mut wb: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("could not open {}", path.display()))?;

    let sheet_names = wb.sheet_names().into_iter().collect::<Vec<_>>();
    if sheet_names.is_empty() {
        bail!("workbook has no sheets");
    }

    // Match the first variant that appears in the headers (case-insensitive).
    let col_any = |headers: &[String], variants: &[&str]| -> Option<usize> {
        for &name in variants {
            let target = name.to_uppercase();
            if let Some(i) = headers.iter().position(|h| h == &target) {
                return Some(i);
            }
        }
        None
    };

    let mut out = Vec::new();
    for sheet_name in sheet_names {
        let range = match wb.worksheet_range(&sheet_name) {
            Ok(r) => r,
            Err(e) => { tracing::warn!("skipping sheet {sheet_name:?}: {e}"); continue; }
        };

        let mut rows_iter = range.rows();
        let headers: Vec<String> = match rows_iter.next() {
            Some(r) => r.iter().map(|c| cell_str(c).unwrap_or_default().to_uppercase()).collect(),
            None    => { tracing::warn!("sheet {sheet_name:?} is empty, skipping"); continue; }
        };

        // Skip sheets that don't have any recognised name column
        let i_name = match col_any(&headers, columns::NAME) {
            Some(i) => i,
            None => {
                tracing::warn!("sheet {sheet_name:?}: no name column found, skipping (tried {:?})", columns::NAME);
                continue;
            }
        };

        let i_sku    = col_any(&headers, columns::SKU);
        let i_cat    = col_any(&headers, columns::CATEGORY);
        let i_model  = col_any(&headers, columns::MODEL_NUMBER);
        let i_part   = col_any(&headers, columns::PART_NUMBER);
        let i_duty   = col_any(&headers, columns::DUTY_PERCENT);
        let i_markup = col_any(&headers, columns::MARKUP);
        let i_cost   = col_any(&headers, columns::COST);
        let i_curr   = col_any(&headers, columns::CURRENCY);
        let i_sup    = col_any(&headers, columns::SUPPLIER);
        let i_unit   = col_any(&headers, columns::UNIT);

        // Derive a category from the sheet name when no explicit column exists
        let sheet_category = Some(sheet_name.trim().to_string());

        let before = out.len();
        for (row_idx, row) in rows_iter.enumerate() {
            let name = match row.get(i_name).and_then(cell_str) {
                Some(n) => n,
                None    => { continue; } // blank rows are common in these sheets
            };

            let sku = get_str(row, i_sku).unwrap_or_else(|| slug_from(&name));

            let mut duty = get_f64(row, i_duty).unwrap_or(0.0);
            if columns::DUTY_IS_FRACTION { duty *= 100.0; }

            let category = get_str(row, i_cat).or_else(|| sheet_category.clone());

            out.push(ImportRow {
                sku,
                name,
                category,
                model_number: get_str(row, i_model),
                part_number:  get_str(row, i_part),
                duty_percent: duty,
                markup:       get_f64(row, i_markup).unwrap_or(30.0),
                cost:         get_f64(row, i_cost),
                currency:     get_str(row, i_curr).unwrap_or_else(|| columns::DEFAULT_CURRENCY.to_string()),
                supplier_name: get_str(row, i_sup),
                unit:         get_str(row, i_unit).unwrap_or_else(|| columns::DEFAULT_UNIT.to_string()),
            });
            let _ = row_idx; // suppress unused warning
        }
        tracing::info!("sheet {sheet_name:?}: imported {} rows", out.len() - before);
    }

    if out.is_empty() {
        bail!("no importable rows found — check that sheets contain one of: {:?}", columns::NAME);
    }
    Ok(out)
}

fn slug_from(name: &str) -> String {
    name.split_whitespace()
        .take(3)
        .map(|w| w.chars().take(4).collect::<String>().to_uppercase())
        .collect::<Vec<_>>()
        .join("-")
}
