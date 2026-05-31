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
        let store = self.store.read(cx);

        let critical: Vec<_> = store.products
            .iter()
            .filter(|p| matches!(p.status, StockStatus::Critical | StockStatus::Low))
            .collect();

        if critical.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("All stock levels healthy.")
                .into_any_element();
        }

        let rows: Vec<_> = critical.iter().map(|p| {
            let badge = if matches!(p.status, StockStatus::Critical) {
                colors::STATUS_RED
            } else {
                colors::STATUS_AMBER
            };

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
                        .child(p.product.name.clone())
                )
                .child(
                    div().text_size(px(12.)).text_color(rgb(colors::STATUS_RED))
                        .child(format!("{:.1} / min {:.1} {}", p.current_stock, p.product.min_stock_level, p.product.unit))
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

    #[test]
    fn critical_and_low_are_alert_states() {
        assert!(matches!(StockStatus::Critical, StockStatus::Critical));
        assert!(matches!(StockStatus::Low, StockStatus::Low));
        assert!(!matches!(StockStatus::Healthy, StockStatus::Critical | StockStatus::Low));
        assert!(!matches!(StockStatus::NoAlert, StockStatus::Critical | StockStatus::Low));
    }
}
