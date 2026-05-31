use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::{Product, StockEntry};

use crate::db::InventoryDb;

// ── View model types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum StockStatus {
    Healthy,         // current > min * 1.2
    Low,             // min < current <= min * 1.2
    Critical,        // current <= min (and min > 0)
    NoAlert,         // min_stock_level == 0
}

impl StockStatus {
    pub fn from_levels(current: f64, min: f64) -> Self {
        if min == 0.0 { return Self::NoAlert; }
        if current <= min { Self::Critical }
        else if current <= min * 1.2 { Self::Low }
        else { Self::Healthy }
    }
}

#[derive(Debug, Clone)]
pub struct ProductWithStock {
    pub product: Product,
    pub current_stock: f64,
    pub status: StockStatus,
}

// ── InventoryStore ────────────────────────────────────────────────────────────

pub struct InventoryStore {
    pub products: Vec<ProductWithStock>,
    pub selected_product_id: Option<i64>,
    pub stock_entries: Vec<StockEntry>,   // entries for selected product
    pub loading: bool,
}

pub enum InventoryEvent {
    ProductsLoaded,
    StockEntriesLoaded,
}

impl EventEmitter<InventoryEvent> for InventoryStore {}

impl InventoryStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            products: Vec::new(),
            selected_product_id: None,
            stock_entries: Vec::new(),
            loading: false,
        }
    }

    /// Async: fetch all products with current stock from DB, update self, notify.
    pub fn load_products(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();

        let db = InventoryDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move {
                    let products = db.list_products()?;
                    let with_stock: anyhow::Result<Vec<ProductWithStock>> = products
                        .into_iter()
                        .map(|p| {
                            let current = db.current_stock(p.id)?;
                            let status = StockStatus::from_levels(current, p.min_stock_level);
                            Ok(ProductWithStock { product: p, current_stock: current, status })
                        })
                        .collect();
                    with_stock
                })
                .await;

            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match result {
                    Ok(products) => {
                        store.products = products;
                        cx.emit(InventoryEvent::ProductsLoaded);
                    }
                    Err(e) => tracing::error!("load_products failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Async: select a product and load its stock entries.
    pub fn select_product(&mut self, product_id: i64, cx: &mut Context<Self>) {
        self.selected_product_id = Some(product_id);
        cx.notify();

        let db = InventoryDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_stock_entries(product_id) })
                .await;

            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(entries) => {
                        store.stock_entries = entries;
                        cx.emit(InventoryEvent::StockEntriesLoaded);
                    }
                    Err(e) => tracing::error!("select_product failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Global for InventoryStore {}

/// Newtype wrapper so `Entity<InventoryStore>` can be stored as a GPUI global.
///
/// `Entity<T>` is defined in `gpui`, so we cannot implement `gpui::Global` for
/// it directly (orphan rule). Instead, panel views call
/// `cx.global::<InventoryStoreHandle>().0.clone()` to get the entity handle.
pub struct InventoryStoreHandle(pub Entity<InventoryStore>);
impl Global for InventoryStoreHandle {}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_status_healthy() {
        assert_eq!(StockStatus::from_levels(10.0, 5.0), StockStatus::Healthy);
    }

    #[test]
    fn stock_status_low() {
        // 5.5 is within 20% above min=5 (threshold: 5.0 * 1.2 = 6.0)
        assert_eq!(StockStatus::from_levels(5.5, 5.0), StockStatus::Low);
    }

    #[test]
    fn stock_status_critical() {
        assert_eq!(StockStatus::from_levels(3.0, 5.0), StockStatus::Critical);
        assert_eq!(StockStatus::from_levels(5.0, 5.0), StockStatus::Critical); // exactly at min
    }

    #[test]
    fn stock_status_no_alert_when_min_zero() {
        assert_eq!(StockStatus::from_levels(0.0, 0.0), StockStatus::NoAlert);
    }
}
