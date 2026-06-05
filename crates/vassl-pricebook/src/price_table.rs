use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rems, rgb};
use vassl_ui::{ThemeColors, ThemeHandle};

use crate::store::{ContextMenuTarget, PriceBookStore, ProductPrice};

pub struct PriceTable {
    store: Entity<PriceBookStore>,
}

impl PriceTable {
    pub fn new(store: Entity<PriceBookStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

pub fn price_display(pp: &ProductPrice) -> String {
    match &pp.latest {
        None => "—".to_string(),
        Some(e) => format!(
            "${:.2}  +${:.2}  →  {:.0}%  →  ${:.2}",
            e.cost_price_usd, e.duty_cost_usd, e.markup_percent, e.selling_price_usd
        ),
    }
}

impl Render for PriceTable {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child("Loading…")
                .into_any_element();
        }

        if store.product_prices.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_default))
                .child("No products found.")
                .into_any_element();
        }

        let selected = store.selected_product_id;
        let filtered = store.filtered_product_prices();
        if filtered.is_empty() && !store.product_prices.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child(format!("No results for \"{}\".", store.search_query))
                .into_any_element();
        }
        let rows: Vec<_> = filtered.iter().map(|pp| {
            let is_selected = selected == Some(pp.product_id);
            price_row(pp, is_selected, self.store.clone(), &c)
        }).collect();

        div()
            .id("price-table-scroll")
            .flex_1().flex().flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn price_row(pp: &ProductPrice, selected: bool, store: Entity<PriceBookStore>, c: &ThemeColors) -> impl IntoElement {
    let product_id   = pp.product_id;
    let product_name = pp.name.clone();
    let row_bg       = if selected { c.surface_active } else { c.canvas_bg };
    let hover_bg     = rgb(c.surface_hover);
    let price_str    = price_display(pp);
    let price_color  = if pp.latest.is_some() { c.text_default } else { c.text_muted };
    let store_right  = store.clone();

    div()
        .id(format!("pb-row-{product_id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(6.))
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
                .w(px(120.)).text_size(rems(0.923))
                .text_color(rgb(c.text_muted))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(pp.sku.clone())
        )
        .child(
            div()
                .w(px(160.)).text_size(rems(1.))
                .text_color(rgb(c.text_default))
                .child(pp.name.clone())
        )
        .child(
            div()
                .flex_1().text_size(rems(0.923))
                .text_color(rgb(price_color))
                .child(price_str)
        )
        .child(
            div()
                .w(px(110.)).text_size(rems(0.846))
                .text_color(rgb(c.text_muted))
                .child(pp.latest.as_ref().map(|e| e.effective_date.get(..10).unwrap_or(&e.effective_date).to_string()).unwrap_or_default())
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::PriceEntry;

    fn make_pp(id: i64, name: &str, cost: Option<f64>) -> ProductPrice {
        let latest = cost.map(|c| PriceEntry {
            id,
            product_id:        id,
            quantity:          1.0,
            cost_price_usd:    c,
            duty_cost_usd:     0.0,
            markup_percent:    30.0,
            selling_price_usd: vassl_core::selling_price(c, 0.0, 30.0).unwrap_or(0.0),
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
            currency:          "USD".to_owned(),
        });
        ProductPrice { product_id: id, sku: format!("SKU-{id}"), name: name.to_string(), latest, duty_percent: 42.5 }
    }

    #[test]
    fn format_price_with_entry() {
        let pp = make_pp(1, "Camera", Some(100.0));
        let display = price_display(&pp);
        assert!(display.contains("100"), "should show cost");
        assert!(display.contains("130"), "should show selling price");
    }

    #[test]
    fn format_price_no_entry() {
        let pp = make_pp(2, "NVR", None);
        let display = price_display(&pp);
        assert_eq!(display, "—");
    }
}
