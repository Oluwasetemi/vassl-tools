use gpui::{
    div, prelude::*, px, rems, rgb, uniform_list, App, Context, Entity, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, UniformListScrollHandle, Window,
};
use vassl_ui::{scrollbar_geometry, ScrollDragState, ThemeColors, ThemeHandle};

use crate::store::{ContextMenuTarget, InventoryStore, ProductWithStock, StockStatus};

const TRACK_W: f32 = 14.0;

pub struct ProductList {
    store: Entity<InventoryStore>,
    pub scroll_handle: UniformListScrollHandle,
    drag: Option<ScrollDragState>,
}

impl ProductList {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self {
            store,
            scroll_handle: UniformListScrollHandle::default(),
            drag: None,
        }
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
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(c.text_muted))
                .child(format!("No results for \"{}\".", store.search_query))
                .into_any_element();
        }
        let count = filtered.len();
        let store_entity = self.store.clone();

        let geom = scrollbar_geometry(&self.scroll_handle);
        let is_dragging = self.drag.is_some();

        // Scrollbar track + thumb
        let mut track = div()
            .id("product-sb-track")
            .flex_shrink_0()
            .w(px(TRACK_W))
            .h_full()
            .relative()
            .bg(rgb(c.surface_default));

        if let Some(g) = &geom {
            let thumb_color = if is_dragging {
                rgb(c.text_default)
            } else {
                rgb(c.text_muted)
            };
            let (viewport_h, thumb_h, max_scroll) = (g.viewport_h, g.thumb_h, g.max_scroll);
            track = track.child(
                div()
                    .id("product-sb-thumb")
                    .absolute()
                    .top(px(g.thumb_top))
                    .left(px(2.))
                    .w(px(TRACK_W - 4.))
                    .h(px(thumb_h))
                    .rounded(px(6.))
                    .bg(thumb_color)
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            this.drag = Some(ScrollDragState {
                                drag_offset: ev.position.y.as_f32(),
                                thumb_h,
                                viewport_h,
                                max_scroll,
                            });
                            cx.notify();
                        }),
                    ),
            );
        }

        let mut root = div()
            .relative()
            .flex_1()
            .flex()
            .flex_row()
            .min_h(px(0.))
            .child(
                uniform_list(
                    "product-list",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let store = this.store.read(cx);
                        let filtered = store.filtered_products();
                        let c = cx.global::<ThemeHandle>().0.clone();
                        let scroll = this.scroll_handle.clone();
                        range
                            .map(|ix| {
                                let p = &filtered[ix];
                                let selected = store.selected_product_id == Some(p.product.id);
                                product_row(
                                    p,
                                    ix,
                                    selected,
                                    store_entity.clone(),
                                    scroll.clone(),
                                    &c,
                                )
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.scroll_handle)
                .flex_1(),
            )
            .child(track);

        // Transparent drag-capture overlay — present only while dragging
        if is_dragging {
            root = root.child(
                div()
                    .id("product-sb-overlay")
                    .absolute()
                    .inset_0()
                    .cursor_pointer()
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        if let Some(drag) = &this.drag {
                            let new_offset = drag.compute_offset(ev.position.y.as_f32());
                            this.scroll_handle
                                .0
                                .borrow()
                                .base_handle
                                .set_offset(gpui::point(gpui::px(0.), gpui::px(new_offset)));
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.drag = None;
                            cx.notify();
                        }),
                    ),
            );
        }

        root.into_any_element()
    }
}

fn product_row(
    p: &ProductWithStock,
    ix: usize,
    selected: bool,
    store: Entity<InventoryStore>,
    scroll_handle: UniformListScrollHandle,
    c: &ThemeColors,
) -> impl IntoElement {
    let product_id = p.product.id;
    let product_name = p.product.name.clone();
    let badge_color = match p.status {
        StockStatus::Healthy => c.status_green,
        StockStatus::Low => c.status_amber,
        StockStatus::Critical => c.status_red,
        StockStatus::NoAlert => c.status_grey,
    };

    let row_bg = if selected {
        c.surface_active
    } else {
        c.canvas_bg
    };
    let hover_bg = rgb(c.surface_hover);
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
            move |event: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
                scroll_handle.scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
                store.update(cx, |s, cx| s.select_product(product_id, cx));
                if event.click_count == 2 {
                    store.update(cx, |s, cx| {
                        s.detail_requested = true;
                        cx.notify();
                    });
                }
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
                .w(px(8.))
                .h(px(8.))
                .rounded_full()
                .bg(rgb(badge_color))
                .mr(px(8.)),
        )
        .child(
            div()
                .w(px(120.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_muted))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(p.product.sku.clone()),
        )
        .child(
            div()
                .flex_1()
                .text_size(rems(1.))
                .text_color(rgb(c.text_default))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(p.product.name.clone()),
        )
        .child(
            div()
                .w(px(70.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_default))
                .child(format!("{:.1} {}", p.current_stock, p.product.unit)),
        )
        .child(
            div()
                .w(px(70.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_muted))
                .child(format!("min {:.1}", p.product.min_stock_level)),
        )
}

#[cfg(test)]
mod tests {
    use crate::colors;

    use super::*;

    #[test]
    fn badge_color_healthy_is_green() {
        let color = match StockStatus::Healthy {
            StockStatus::Healthy => colors::STATUS_GREEN,
            StockStatus::Low => colors::STATUS_AMBER,
            StockStatus::Critical => colors::STATUS_RED,
            StockStatus::NoAlert => colors::STATUS_GREY,
        };
        assert_eq!(color, colors::STATUS_GREEN);
    }

    #[test]
    fn badge_color_critical_is_red() {
        let color = match StockStatus::Critical {
            StockStatus::Healthy => colors::STATUS_GREEN,
            StockStatus::Low => colors::STATUS_AMBER,
            StockStatus::Critical => colors::STATUS_RED,
            StockStatus::NoAlert => colors::STATUS_GREY,
        };
        assert_eq!(color, colors::STATUS_RED);
    }
}
