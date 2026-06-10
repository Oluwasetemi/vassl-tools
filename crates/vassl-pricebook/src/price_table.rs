use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
           MouseUpEvent, Render, Window,
           div, prelude::*, px, rems, rgb, uniform_list, UniformListScrollHandle};
use vassl_ui::{ScrollDragState, ThemeColors, ThemeHandle, scrollbar_geometry};

use crate::store::{ContextMenuTarget, PriceBookStore, ProductPrice};

const TRACK_W: f32 = 14.0;

pub struct PriceTable {
    store: Entity<PriceBookStore>,
    pub scroll_handle: UniformListScrollHandle,
    drag: Option<ScrollDragState>,
}

impl PriceTable {
    pub fn new(store: Entity<PriceBookStore>, _cx: &mut Context<Self>) -> Self {
        Self { store, scroll_handle: UniformListScrollHandle::default(), drag: None }
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

        let filtered = store.filtered_product_prices();
        if filtered.is_empty() && !store.product_prices.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child(format!("No results for \"{}\".", store.search_query))
                .into_any_element();
        }
        let count = filtered.len();
        let store_entity = self.store.clone();

        let geom = scrollbar_geometry(&self.scroll_handle);
        let is_dragging = self.drag.is_some();

        let mut track = div()
            .id("pb-sb-track")
            .flex_shrink_0()
            .w(px(TRACK_W))
            .h_full()
            .relative()
            .bg(rgb(c.surface_default));

        if let Some(g) = &geom {
            let thumb_color = if is_dragging { rgb(c.text_default) } else { rgb(c.text_muted) };
            let (viewport_h, thumb_h, max_scroll) = (g.viewport_h, g.thumb_h, g.max_scroll);
            track = track.child(
                div()
                    .id("pb-sb-thumb")
                    .absolute()
                    .top(px(g.thumb_top))
                    .left(px(2.))
                    .w(px(TRACK_W - 4.))
                    .h(px(thumb_h))
                    .rounded(px(6.))
                    .bg(thumb_color)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        this.drag = Some(ScrollDragState {
                            drag_offset: ev.position.y.as_f32(),
                            thumb_h,
                            viewport_h,
                            max_scroll,
                        });
                        cx.notify();
                    }))
            );
        }

        let mut root = div()
            .relative()
            .flex_1().flex().flex_row().min_h(px(0.))
            .child(
                uniform_list(
                    "price-table",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let store = this.store.read(cx);
                        let filtered = store.filtered_product_prices();
                        let c = cx.global::<ThemeHandle>().0.clone();
                        let selected = store.selected_product_id;
                        let scroll = this.scroll_handle.clone();
                        range.map(|ix| {
                            let pp = &filtered[ix];
                            let is_selected = selected == Some(pp.product_id);
                            price_row(pp, ix, is_selected, store_entity.clone(), scroll.clone(), &c)
                        }).collect()
                    }),
                )
                .track_scroll(&self.scroll_handle)
                .flex_1()
            )
            .child(track);

        if is_dragging {
            root = root.child(
                div()
                    .id("pb-sb-overlay")
                    .absolute().inset_0()
                    .cursor_pointer()
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        if let Some(drag) = &this.drag {
                            let new_offset = drag.compute_offset(ev.position.y.as_f32());
                            this.scroll_handle.0.borrow().base_handle.set_offset(
                                gpui::point(gpui::px(0.), gpui::px(new_offset))
                            );
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| {
                        this.drag = None;
                        cx.notify();
                    }))
            );
        }

        root.into_any_element()
    }
}

fn price_row(pp: &ProductPrice, ix: usize, selected: bool, store: Entity<PriceBookStore>, scroll_handle: UniformListScrollHandle, c: &ThemeColors) -> impl IntoElement {
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
                scroll_handle.scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
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
                .w(px(220.)).text_size(rems(1.))
                .text_color(rgb(c.text_default))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
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
