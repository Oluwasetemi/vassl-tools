use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rems, rgb, uniform_list, UniformListScrollHandle};
use vassl_ui::{ThemeColors, ThemeHandle};

use crate::store::{ContextMenuTarget, InventoryStore, ProductWithStock, StockStatus};

pub struct ProductList {
    store: Entity<InventoryStore>,
    scroll_handle: UniformListScrollHandle,
}

impl ProductList {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self { store, scroll_handle: UniformListScrollHandle::default() }
    }
}

impl Render for ProductList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(c.text_muted))
                .child("Loading…")
                .into_any_element();
        }

        if store.products.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(c.text_default))
                .child("No products — add stock entries to get started.")
                .into_any_element();
        }

        let filtered = store.filtered_products();
        if filtered.is_empty() && !store.products.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child(format!("No results for \"{}\".", store.search_query))
                .into_any_element();
        }
        let count = filtered.len();
        let store_entity = self.store.clone();

        uniform_list(
            "product-list",
            count,
            cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                let store = this.store.read(cx);
                let filtered = store.filtered_products();
                let c = cx.global::<ThemeHandle>().0.clone();
                range.map(|ix| {
                    let p = &filtered[ix];
                    let selected = store.selected_product_id == Some(p.product.id);
                    product_row(p, selected, store_entity.clone(), &c)
                }).collect()
            }),
        )
        .track_scroll(&self.scroll_handle)
        .flex_1()
        .into_any_element()
    }
}

fn product_row(p: &ProductWithStock, selected: bool, store: Entity<InventoryStore>, c: &ThemeColors) -> impl IntoElement {
    let product_id    = p.product.id;
    let product_name  = p.product.name.clone();
    let badge_color = match p.status {
        StockStatus::Healthy  => c.status_green,
        StockStatus::Low      => c.status_amber,
        StockStatus::Critical => c.status_red,
        StockStatus::NoAlert  => c.status_grey,
    };

    let row_bg      = if selected { c.surface_active } else { c.canvas_bg };
    let hover_bg    = rgb(c.surface_hover);
    let store_right = store.clone();

    div()
        .id(format!("product-{product_id}"))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .bg(rgb(row_bg))
        .when(!selected, |d| d.hover(move |s| s.bg(hover_bg)))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_product(product_id, cx));
            },
        )
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                let target = ContextMenuTarget {
                    product_id,
                    product_name: product_name.clone(),
                    x: event.position.x.as_f32(),
                    y: event.position.y.as_f32(),
                };
                store_right.update(cx, |s, cx| s.set_context_menu(target, cx));
            },
        )
        .child(
            div()
                .w(px(8.)).h(px(8.))
                .rounded_full()
                .bg(rgb(badge_color))
                .mr(px(8.))
        )
        .child(
            div()
                .w(px(120.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_muted))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(p.product.sku.clone())
        )
        .child(
            div()
                .flex_1()
                .text_size(rems(1.))
                .text_color(rgb(c.text_default))
                .child(p.product.name.clone())
        )
        .child(
            div()
                .w(px(70.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_default))
                .child(format!("{:.1} {}", p.current_stock, p.product.unit))
        )
        .child(
            div()
                .w(px(70.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_muted))
                .child(format!("min {:.1}", p.product.min_stock_level))
        )
}

#[cfg(test)]
mod tests {
    use crate::colors;

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
