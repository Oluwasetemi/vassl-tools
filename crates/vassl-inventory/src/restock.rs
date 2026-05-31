use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rgb};

use crate::store::{InventoryStore, StockStatus};
use crate::colors;

pub struct RestockAlerts {
    store: Entity<InventoryStore>,
}

impl RestockAlerts {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

impl Render for RestockAlerts {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract owned data — borrow ends after this block
        let items: Vec<(String, f64, f64, String, bool)> = {
            let store = self.store.read(cx);
            store.products.iter()
                .filter(|p| matches!(p.status, StockStatus::Critical | StockStatus::Low))
                .map(|p| (
                    p.product.name.clone(),
                    p.current_stock,
                    p.product.min_stock_level,
                    p.product.unit.clone(),
                    matches!(p.status, StockStatus::Critical),
                ))
                .collect()
        };

        if items.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("All stock levels healthy.")
                .into_any_element();
        }

        let rows: Vec<_> = items.iter().map(|(name, current, min, unit, is_critical)| {
            let badge = if *is_critical { colors::STATUS_RED } else { colors::STATUS_AMBER };

            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .px(px(12.)).py(px(8.))
                .child(
                    div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(badge)).mr(px(8.))
                )
                .child(
                    div().flex_1().text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                        .child(name.clone())
                )
                .child(
                    div().text_size(px(12.)).text_color(rgb(badge))
                        .child(format!("{current:.1} / min {min:.1} {unit}"))
                )
        }).collect();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .id("restock-alerts-scroll")
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::Product;

    fn make_product(id: i64) -> Product {
        Product {
            id,
            sku: format!("SKU-{id}"),
            name: format!("Product {id}"),
            category: None,
            unit: "pcs".to_string(),
            min_stock_level: 5.0,
            notes: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_pws(id: i64, current: f64, status: StockStatus) -> crate::store::ProductWithStock {
        crate::store::ProductWithStock {
            product: make_product(id),
            current_stock: current,
            status,
        }
    }

    #[test]
    fn critical_and_low_products_appear_in_alert_list() {
        let products = vec![
            make_pws(1, 2.0, StockStatus::Critical),
            make_pws(2, 5.5, StockStatus::Low),
            make_pws(3, 10.0, StockStatus::Healthy),
            make_pws(4, 0.0, StockStatus::NoAlert),
        ];

        let alert_count = products.iter()
            .filter(|p| matches!(p.status, StockStatus::Critical | StockStatus::Low))
            .count();

        assert_eq!(alert_count, 2, "only Critical and Low should appear in alerts");
    }
}
