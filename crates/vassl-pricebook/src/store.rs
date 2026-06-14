use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::PriceEntry;

use crate::db::PriceBookDb;

const DEFAULT_RATE: f64 = 157.50;

#[derive(Debug, Clone)]
pub struct ProductPrice {
    pub product_id:   i64,
    pub sku:          String,
    pub name:         String,
    pub duty_percent: f64,
    pub latest:       Option<PriceEntry>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuTarget {
    pub product_id:   i64,
    pub product_name: String,
    pub x:            f32,
    pub y:            f32,
}

pub struct PriceBookStore {
    pub product_prices:      Vec<ProductPrice>,
    pub selected_product_id: Option<i64>,
    pub detail_generation:   u32,
    pub history:             Vec<PriceEntry>,
    pub loading:             bool,
    pub context_menu:        Option<ContextMenuTarget>,
    pub search_query:        String,
    pub edit_target:         Option<vassl_core::PriceEntry>,
    pub usd_to_jmd_rate:     f64,
}

pub struct PriceBookStoreHandle(pub Entity<PriceBookStore>);
impl Global for PriceBookStoreHandle {}

#[derive(Debug)]
pub enum PriceBookEvent {
    ProductsLoaded,
    HistoryLoaded,
}

impl EventEmitter<PriceBookEvent> for PriceBookStore {}

impl PriceBookStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            product_prices:      Vec::new(),
            selected_product_id: None,
            detail_generation:   0,
            history:             Vec::new(),
            loading:             false,
            context_menu:        None,
            search_query:        String::new(),
            edit_target:         None,
            usd_to_jmd_rate:     DEFAULT_RATE,
        }
    }

    pub fn load_products(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;

        let app_db = vassl_db::AppDatabase::global(&**cx);
        if let Ok(Some(s)) = vassl_db::shared::get_setting(app_db, "pricebook.usd_to_jmd_rate") {
            if let Ok(r) = s.parse::<f64>() { self.usd_to_jmd_rate = r; }
        }

        cx.notify();

        let db = PriceBookDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { db.list_products_with_latest_price() })
                .await;

            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match result {
                    Ok(rows) => {
                        store.product_prices = rows
                            .into_iter()
                            .map(|(pid, sku, name, latest)| ProductPrice { product_id: pid, sku, name, duty_percent: 0.0, latest })
                            .collect();
                        cx.emit(PriceBookEvent::ProductsLoaded);
                    }
                    Err(e) => tracing::error!("load_products_with_latest_price failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_product(&mut self, product_id: i64, cx: &mut Context<Self>) {
        if self.selected_product_id == Some(product_id) { return; }
        self.selected_product_id = Some(product_id);
        self.history.clear();
        cx.notify();

        let db = PriceBookDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { db.list_entries_for_product(product_id) })
                .await;

            let _ = this.update(cx, |store, cx| {
                if store.selected_product_id != Some(product_id) { return; } // stale result
                match result {
                    Ok(entries) => {
                        store.history = entries;
                        cx.emit(PriceBookEvent::HistoryLoaded);
                    }
                    Err(e) => tracing::error!("list_entries_for_product failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_product_and_detail(&mut self, product_id: i64, cx: &mut Context<Self>) {
        self.select_product(product_id, cx);
        self.detail_generation += 1;
        cx.notify();
    }

    pub fn delete_price_entry(&mut self, entry_id: i64, product_id: i64, cx: &mut Context<Self>) {
        let db = PriceBookDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.delete_price_entry(entry_id).await;
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(_) => {
                        store.history.retain(|e| e.id != entry_id);
                        if let Some(pp) = store.product_prices.iter_mut().find(|pp| pp.product_id == product_id) {
                            if pp.latest.as_ref().map(|e| e.id) == Some(entry_id) {
                                pp.latest = store.history.first().cloned();
                            }
                        }
                        cx.notify();
                    }
                    Err(e) => tracing::error!("delete_price_entry failed: {e:?}"),
                }
            });
        }).detach();
    }

    pub fn set_edit_target(&mut self, entry: vassl_core::PriceEntry, cx: &mut Context<Self>) {
        self.edit_target = Some(entry);
        cx.notify();
    }

    pub fn clear_edit_target(&mut self, cx: &mut Context<Self>) {
        self.edit_target = None;
        cx.notify();
    }

    pub fn set_context_menu(&mut self, target: ContextMenuTarget, cx: &mut Context<Self>) {
        self.context_menu = Some(target);
        cx.notify();
    }

    pub fn clear_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        cx.notify();
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.search_query == query { return; }
        self.search_query = query;
        cx.notify();
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let filtered = self.filtered_product_prices();
        if filtered.is_empty() { return None; }
        let cur = self.selected_product_id
            .and_then(|id| filtered.iter().position(|pp| pp.product_id == id));
        let next = match cur { None => 0, Some(i) => (i + 1).min(filtered.len() - 1) };
        self.select_product(filtered[next].product_id, cx);
        Some(next)
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        let filtered = self.filtered_product_prices();
        if filtered.is_empty() { return None; }
        let cur = self.selected_product_id
            .and_then(|id| filtered.iter().position(|pp| pp.product_id == id));
        let next = match cur { None => 0, Some(0) => 0, Some(i) => i - 1 };
        self.select_product(filtered[next].product_id, cx);
        Some(next)
    }

    pub fn filtered_product_prices(&self) -> Vec<&ProductPrice> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            return self.product_prices.iter().collect();
        }
        self.product_prices.iter().filter(|pp| {
            pp.name.to_lowercase().contains(&q) || pp.sku.to_lowercase().contains(&q)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::PriceEntry;

    fn make_entry(id: i64, product_id: i64, cost: f64) -> PriceEntry {
        PriceEntry {
            id,
            product_id,
            quantity:          1.0,
            cost_price_usd:    cost,
            duty_cost_usd:     0.0,
            markup_percent:    30.0,
            selling_price_usd: vassl_core::selling_price(cost, 0.0, 30.0).unwrap_or(0.0),
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
            currency:          "USD".to_string(),
        }
    }

    #[test]
    fn filtered_product_prices_empty_query_returns_all() {
        let store = PriceBookStore {
            product_prices: vec![
                ProductPrice { product_id: 1, sku: "CAM-001".into(), name: "IP Camera".into(), duty_percent: 0.0, latest: None },
                ProductPrice { product_id: 2, sku: "NVR-001".into(), name: "NVR Unit".into(),  duty_percent: 0.0, latest: None },
            ],
            selected_product_id: None,
            detail_generation: 0,
            history: vec![],
            loading: false,
            context_menu: None,
            search_query: String::new(),
            edit_target: None,
            usd_to_jmd_rate: DEFAULT_RATE,
        };
        assert_eq!(store.filtered_product_prices().len(), 2);
    }

    #[test]
    fn filtered_product_prices_matches_name_case_insensitive() {
        let store = PriceBookStore {
            product_prices: vec![
                ProductPrice { product_id: 1, sku: "CAM-001".into(), name: "IP Camera".into(), duty_percent: 0.0, latest: None },
                ProductPrice { product_id: 2, sku: "NVR-001".into(), name: "NVR Unit".into(),  duty_percent: 0.0, latest: None },
            ],
            selected_product_id: None,
            detail_generation: 0,
            history: vec![],
            loading: false,
            context_menu: None,
            search_query: "camera".into(),
            edit_target: None,
            usd_to_jmd_rate: DEFAULT_RATE,
        };
        let results = store.filtered_product_prices();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "IP Camera");
    }

    #[test]
    fn filtered_product_prices_matches_sku() {
        let store = PriceBookStore {
            product_prices: vec![
                ProductPrice { product_id: 1, sku: "CAM-001".into(), name: "IP Camera".into(), duty_percent: 0.0, latest: None },
            ],
            selected_product_id: None,
            detail_generation: 0,
            history: vec![],
            loading: false,
            context_menu: None,
            search_query: "cam-".into(),
            edit_target: None,
            usd_to_jmd_rate: DEFAULT_RATE,
        };
        assert_eq!(store.filtered_product_prices().len(), 1);
    }

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

    #[test]
    fn product_price_with_no_entry_has_no_latest() {
        let pp = ProductPrice {
            product_id:   1,
            sku:          "X".to_string(),
            name:         "Y".to_string(),
            duty_percent: 0.0,
            latest:       None,
        };
        assert!(pp.latest.is_none());
    }

    #[test]
    fn product_price_with_entry_exposes_selling_price() {
        let pp = ProductPrice {
            product_id:   1,
            sku:          "A".to_string(),
            name:         "B".to_string(),
            duty_percent: 0.0,
            latest:       Some(make_entry(1, 1, 100.0)),
        };
        assert_eq!(pp.latest.unwrap().selling_price_usd, 130.0);
    }

}
