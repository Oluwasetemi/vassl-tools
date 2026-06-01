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

// ── ContextMenuTarget ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContextMenuTarget {
    pub product_id:   i64,
    pub product_name: String,
    pub x:            f32,
    pub y:            f32,
}

// ── InventoryStore ────────────────────────────────────────────────────────────

pub struct InventoryStore {
    pub products: Vec<ProductWithStock>,
    pub selected_product_id: Option<i64>,
    pub stock_entries: Vec<StockEntry>,   // entries for selected product
    pub loading: bool,
    pub context_menu: Option<ContextMenuTarget>,
}

#[derive(Debug)]
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
            context_menu: None,
        }
    }

    /// Async: fetch all products with current stock from DB, update self, notify.
    pub fn load_products(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = InventoryDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move {
                    db.list_products_with_stock()?.into_iter().map(|(p, current_stock)| {
                        let status = StockStatus::from_levels(current_stock, p.min_stock_level);
                        Ok(ProductWithStock { product: p, current_stock, status })
                    }).collect::<anyhow::Result<Vec<_>>>()
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
        if self.selected_product_id == Some(product_id) { return; }
        self.selected_product_id = Some(product_id);
        self.stock_entries.clear();  // prevent stale entries showing for new product
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

    pub fn set_context_menu(&mut self, target: ContextMenuTarget, cx: &mut Context<Self>) {
        self.context_menu = Some(target);
        cx.notify();
    }

    pub fn clear_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        cx.notify();
    }
}

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
        let target: Option<ContextMenuTarget> = None;
        assert!(target.is_none());
    }

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

    #[test]
    fn stock_status_low_at_exact_boundary() {
        // 6.0 == 5.0 * 1.2, which is exactly at the Low/Healthy boundary (<=), so Low
        assert_eq!(StockStatus::from_levels(6.0, 5.0), StockStatus::Low);
    }
}
