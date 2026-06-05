use anyhow::Context;
use rusqlite::{Connection, OpenFlags, params};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Price-book tables in vass.db ───────────────────────────────────────────
// Each entry: (table_name, has_product_group, is_installation_material)
const PB_TABLES: &[(&str, bool, bool)] = &[
    ("pb_access_control",      true,  false),
    ("pb_cctv_products",       true,  false),
    ("pb_fire_alarm",          true,  false),
    ("pb_id_card_accessories", false, false),
    ("pb_automated_barrier",   false, false),
    ("pb_intrusion_products",  true,  false),
    ("pb_installation_material", false, true),
];

// ── Stock-count tables in vass.db ──────────────────────────────────────────
const SC_TABLES: &[&str] = &[
    "sc_cctv",
    "sc_access_control",
    "sc_fire_alarm_intrusion",
    "sc_id_accessories",
    "sc_cctv_accessories",
    "sc_access_accessories",
];

#[derive(Debug)]
struct PbRow {
    manufacturer:     Option<String>,
    product_description: String,
    part_number:      Option<String>,
    cost_price_usd:   Option<f64>,
    duty_fraction:    Option<f64>,
    markup_fraction:  Option<f64>,
    selling_price_usd: Option<f64>,
    product_group:    Option<String>,
    currency:         String,
    #[allow(dead_code)]
    source_table:     String,
}

#[derive(Debug, Default)]
struct ImportSummary {
    products_created:  usize,
    products_updated:  usize,
    suppliers_created: usize,
    price_entries:     usize,
    stock_entries:     usize,
    skipped:           usize,
}

fn app_db_path() -> PathBuf {
    dirs::data_local_dir()
        .expect("no local data dir")
        .join("VASSL")
        .join("0-global")
        .join("db.sqlite")
}

fn slug_from(name: &str) -> String {
    name.split_whitespace()
        .take(3)
        .map(|w| w.chars().take(4).collect::<String>().to_uppercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn is_header_row(s: &str) -> bool {
    let s = s.to_uppercase();
    s.contains("MANUFACTURER") || s.contains("VENDOR") || s.contains("DESCRIPTION")
}

/// Read all pb_* rows from the source DB, normalising schema differences.
fn read_pb_rows(src: &Connection) -> anyhow::Result<Vec<PbRow>> {
    let mut all = Vec::new();

    for &(table, has_product_group, is_install_material) in PB_TABLES {
        let sql = if is_install_material {
            format!(
                "SELECT manufacturer, product_description, part_number,
                        cost_price_usd, duty, markup,
                        vass_selling_price_usd, NULL
                 FROM {table} WHERE id > 1"
            )
        } else if has_product_group {
            format!(
                "SELECT manufacturer, product_description, part_number,
                        cost_price_usd, duty, markup,
                        selling_price_usd, product_group
                 FROM {table} WHERE id > 1"
            )
        } else {
            format!(
                "SELECT manufacturer, product_description, part_number,
                        cost_price_usd, duty, markup,
                        selling_price_usd, NULL
                 FROM {table} WHERE id > 1"
            )
        };

        let mut stmt = src.prepare(&sql)
            .with_context(|| format!("prepare query for {table}"))?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<f64>>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?.collect::<Result<Vec<_>, _>>()?;

        for (manufacturer, description, part_number,
             cost_usd, duty, markup, selling, group) in rows
        {
            let name = match description {
                Some(ref d) if !d.trim().is_empty() && !is_header_row(d) => d.trim().to_string(),
                _ => continue,
            };
            let manufacturer = manufacturer
                .filter(|m| !m.trim().is_empty() && !is_header_row(m));

            all.push(PbRow {
                manufacturer,
                product_description: name,
                part_number: part_number.filter(|p| !p.trim().is_empty()),
                cost_price_usd: cost_usd,
                duty_fraction: duty,
                markup_fraction: markup,
                selling_price_usd: selling,
                product_group: group.filter(|g| !g.trim().is_empty()),
                currency: "USD".into(),
                source_table: table.to_string(),
            });
        }
    }

    Ok(all)
}

struct StockInfo {
    qty:          f64,
    model_number: Option<String>,
}

/// Build part_number → (stock_qty_2025, model_number) from all sc_* tables.
fn read_stock_map(src: &Connection) -> anyhow::Result<HashMap<String, StockInfo>> {
    let mut map: HashMap<String, StockInfo> = HashMap::new();
    for &table in SC_TABLES {
        let mut stmt = src.prepare(
            &format!("SELECT part_number, stock_qty_2025, model_number FROM {table} WHERE id > 1")
        ).with_context(|| format!("prepare stock query for {table}"))?;

        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<f64>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?.collect::<Result<Vec<_>, _>>()?;

        for (part, qty, model) in rows {
            if let Some(p) = part {
                let p = p.trim().to_string();
                if p.is_empty() { continue; }
                let q = qty.unwrap_or(0.0);
                let model = model.filter(|m| !m.trim().is_empty());
                map.entry(p)
                   .and_modify(|e| { e.qty += q; if e.model_number.is_none() { e.model_number = model.clone(); } })
                   .or_insert(StockInfo { qty: q, model_number: model });
            }
        }
    }
    Ok(map)
}

fn import(
    src: &Connection,
    dst: &Connection,
    dry_run: bool,
) -> anyhow::Result<ImportSummary> {
    let rows     = read_pb_rows(src)?;
    let stock_map = read_stock_map(src)?;
    let now = chrono::Utc::now().to_rfc3339();
    let mut summary = ImportSummary::default();
    let mut supplier_cache: HashMap<String, i64> = HashMap::new();

    for row in &rows {
        // ── supplier ──────────────────────────────────────────────────────
        let supplier_id: Option<i64> = match &row.manufacturer {
            None => None,
            Some(sup_name) => {
                if let Some(&id) = supplier_cache.get(sup_name) {
                    Some(id)
                } else {
                    let existing: Option<i64> = dst.query_row(
                        "SELECT id FROM suppliers WHERE name = ?1",
                        params![sup_name],
                        |r| r.get(0),
                    ).ok();

                    let id = if let Some(id) = existing {
                        id
                    } else if !dry_run {
                        dst.execute(
                            "INSERT INTO suppliers (name, created_at) VALUES (?1, ?2)",
                            params![sup_name, now],
                        )?;
                        dst.last_insert_rowid()
                    } else {
                        -(summary.suppliers_created as i64 + 1)
                    };
                    if existing.is_none() { summary.suppliers_created += 1; }
                    supplier_cache.insert(sup_name.clone(), id);
                    Some(id)
                }
            }
        };

        // ── SKU derivation ────────────────────────────────────────────────
        // part_number + model_number > part_number alone > name slug
        let model_from_stock = row.part_number.as_deref()
            .and_then(|p| stock_map.get(p))
            .and_then(|s| s.model_number.as_deref())
            .filter(|mn| Some(*mn) != row.part_number.as_deref()); // skip if identical to part
        let sku = match (&row.part_number, model_from_stock) {
            (Some(pn), Some(mn)) => format!("{pn}-{mn}"),
            (Some(pn), None)     => pn.clone(),
            (None, _)            => slug_from(&row.product_description),
        };

        // ── product ───────────────────────────────────────────────────────
        let existing_pid: Option<i64> = dst.query_row(
            "SELECT id FROM products WHERE sku = ?1",
            params![sku],
            |r| r.get(0),
        ).ok();

        let duty_pct = row.duty_fraction.unwrap_or(0.0) * 100.0;

        let product_id = if let Some(pid) = existing_pid {
            if !dry_run {
                dst.execute(
                    "UPDATE products SET name=?1, category=?2, duty_percent=?3,
                             part_number=?4, preferred_supplier_id=?5
                     WHERE id=?6",
                    params![row.product_description, row.product_group, duty_pct,
                            row.part_number, supplier_id, pid],
                )?;
            }
            summary.products_updated += 1;
            pid
        } else {
            let pid = if !dry_run {
                dst.execute(
                    "INSERT INTO products
                     (sku, name, category, unit, min_stock_level, part_number,
                      duty_percent, preferred_supplier_id, created_at)
                     VALUES (?1,?2,?3,'pcs',0,?4,?5,?6,?7)",
                    params![sku, row.product_description, row.product_group,
                            row.part_number, duty_pct, supplier_id, now],
                )?;
                dst.last_insert_rowid()
            } else {
                -(summary.products_created as i64 + 1)
            };
            summary.products_created += 1;
            pid
        };

        // ── price book entry ──────────────────────────────────────────────
        if let Some(cost) = row.cost_price_usd {
            let duty_usd  = cost * row.duty_fraction.unwrap_or(0.0);
            let markup_pct = row.markup_fraction.unwrap_or(0.3) * 100.0;
            let selling   = row.selling_price_usd
                .unwrap_or_else(|| (cost + duty_usd) * (1.0 + markup_pct / 100.0));

            if !dry_run {
                dst.execute(
                    "INSERT INTO price_book_entries
                     (product_id, quantity, cost_price_usd, duty_cost_usd,
                      markup_percent, selling_price_usd, effective_date, currency)
                     VALUES (?1,1,?2,?3,?4,?5,?6,?7)",
                    params![product_id, cost, duty_usd, markup_pct, selling, now, row.currency],
                )?;
            }
            summary.price_entries += 1;
        }

        // ── initial stock ─────────────────────────────────────────────────
        if let Some(part) = &row.part_number {
            if let Some(stock) = stock_map.get(part.as_str()) {
                if stock.qty > 0.0 {
                    let unit_cost = row.cost_price_usd.unwrap_or(0.0);
                    if !dry_run {
                        dst.execute(
                            "INSERT INTO stock_entries
                             (product_id, quantity, unit_cost_usd, supplier,
                              acquired_at, acquisition_type)
                             VALUES (?1,?2,?3,?4,?5,'restock')",
                            params![product_id, stock.qty, unit_cost,
                                    row.manufacturer, now],
                        )?;
                    }
                    summary.stock_entries += 1;
                }
            }
        }
    }

    Ok(summary)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let src_path = args.iter().skip(1).find(|a| !a.starts_with('-'))
        .context("Usage: seed <path/to/vass.db> [--dry-run]")?;

    let src_path = PathBuf::from(src_path);
    anyhow::ensure!(src_path.exists(), "source DB not found: {}", src_path.display());

    let dst_path = app_db_path();
    anyhow::ensure!(
        dst_path.exists(),
        "VASSL app DB not found at {}.\nRun the VASSL app first to initialise the database.",
        dst_path.display()
    );

    println!("Source DB : {}", src_path.display());
    println!("Target DB : {}", dst_path.display());
    if dry_run { println!("\n[DRY RUN] No changes will be written.\n"); }

    let src = Connection::open_with_flags(
        &src_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let dst = Connection::open(&dst_path)?;
    dst.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    // Preview rows in dry-run mode
    if dry_run {
        let rows      = read_pb_rows(&src)?;
        let stock_map = read_stock_map(&src)?;
        println!("  {} product rows found across pb_* tables\n", rows.len());
        for r in rows.iter().take(10) {
            let model = r.part_number.as_deref()
                .and_then(|p| stock_map.get(p))
                .and_then(|s| s.model_number.as_deref())
                .filter(|mn| Some(*mn) != r.part_number.as_deref());
            let sku = match (&r.part_number, model) {
                (Some(pn), Some(mn)) => format!("{pn}-{mn}"),
                (Some(pn), None)     => pn.clone(),
                (None, _)            => slug_from(&r.product_description),
            };
            println!("  sku={sku:35}  cost={:?}  duty={:.1}%  markup={:.0}%",
                r.cost_price_usd,
                r.duty_fraction.unwrap_or(0.0) * 100.0,
                r.markup_fraction.unwrap_or(0.0) * 100.0,
            );
        }
        if rows.len() > 10 { println!("  … and {} more", rows.len() - 10); }
        println!();
    }

    let summary = import(&src, &dst, dry_run)?;

    if dry_run { println!("[DRY RUN] Would have:"); } else { println!("Done:"); }
    println!("  {} products created",   summary.products_created);
    println!("  {} products updated",   summary.products_updated);
    println!("  {} suppliers created",  summary.suppliers_created);
    println!("  {} price entries added", summary.price_entries);
    println!("  {} stock entries added", summary.stock_entries);
    println!("  {} rows skipped (empty name/header)", summary.skipped);

    Ok(())
}
