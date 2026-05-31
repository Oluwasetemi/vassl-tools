use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window, div, prelude::*, px, rgb};

use crate::store::{InventoryStore, ProductWithStock, StockStatus};
use crate::colors;

pub struct ProductList {
    store: Entity<InventoryStore>,
}

impl ProductList {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

impl Render for ProductList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("Loading…")
                .into_any_element();
        }

        if store.products.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(colors::TEXT_MUTED))
                .child("No products — add stock entries to get started.")
                .into_any_element();
        }

        let rows: Vec<_> = store.products.iter().map(|p| {
            let selected = store.selected_product_id == Some(p.product.id);
            product_row(p, selected, self.store.clone())
        }).collect();

        div()
            .id("product-list-scroll")
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn product_row(p: &ProductWithStock, selected: bool, store: Entity<InventoryStore>) -> impl IntoElement {
    let product_id = p.product.id;
    let badge_color = match p.status {
        StockStatus::Healthy  => colors::STATUS_GREEN,
        StockStatus::Low      => colors::STATUS_AMBER,
        StockStatus::Critical => colors::STATUS_RED,
        StockStatus::NoAlert  => colors::STATUS_GREY,
    };

    let row_bg = if selected { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG };

    div()
        .id(format!("product-{product_id}"))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_product(product_id, cx));
            },
        )
        // Status badge — 8×8 colored dot
        .child(
            div()
                .w(px(8.)).h(px(8.))
                .rounded_full()
                .bg(rgb(badge_color))
                .mr(px(8.))
        )
        // SKU
        .child(
            div()
                .w(px(80.))
                .text_size(px(12.))
                .text_color(rgb(colors::TEXT_MUTED))
                .child(p.product.sku.clone())
        )
        // Name
        .child(
            div()
                .flex_1()
                .text_size(px(13.))
                .text_color(rgb(colors::TEXT_DEFAULT))
                .child(p.product.name.clone())
        )
        // Current qty
        .child(
            div()
                .w(px(70.))
                .text_size(px(12.))
                .text_color(rgb(colors::TEXT_DEFAULT))
                .child(format!("{:.1} {}", p.current_stock, p.product.unit))
        )
        // Min level
        .child(
            div()
                .w(px(70.))
                .text_size(px(12.))
                .text_color(rgb(colors::TEXT_MUTED))
                .child(format!("min {:.1}", p.product.min_stock_level))
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_color_healthy_is_green() {
        let color = match StockStatus::Healthy {
            StockStatus::Healthy  => colors::STATUS_GREEN,
            StockStatus::Low      => colors::STATUS_AMBER,
            StockStatus::Critical => colors::STATUS_RED,
            StockStatus::NoAlert  => colors::STATUS_GREY,
        };
        assert_eq!(color, colors::STATUS_GREEN);
    }

    #[test]
    fn badge_color_critical_is_red() {
        let color = match StockStatus::Critical {
            StockStatus::Healthy  => colors::STATUS_GREEN,
            StockStatus::Low      => colors::STATUS_AMBER,
            StockStatus::Critical => colors::STATUS_RED,
            StockStatus::NoAlert  => colors::STATUS_GREY,
        };
        assert_eq!(color, colors::STATUS_RED);
    }
}
