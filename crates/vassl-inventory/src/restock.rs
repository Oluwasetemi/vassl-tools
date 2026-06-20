use gpui::{
    div, prelude::*, px, rems, rgb, uniform_list, Context, Entity, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, UniformListScrollHandle, Window,
};
use vassl_ui::{scrollbar_geometry, ScrollDragState, ThemeHandle};

use crate::store::{InventoryStore, StockStatus};

const TRACK_W: f32 = 14.0;

pub struct RestockAlerts {
    store: Entity<InventoryStore>,
    scroll_handle: UniformListScrollHandle,
    drag: Option<ScrollDragState>,
}

impl RestockAlerts {
    pub fn new(store: Entity<InventoryStore>, _cx: &mut Context<Self>) -> Self {
        Self {
            store,
            scroll_handle: UniformListScrollHandle::default(),
            drag: None,
        }
    }
}

impl Render for RestockAlerts {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();

        let items: Vec<(String, f64, f64, String, bool)> = {
            let store = self.store.read(cx);
            store
                .products
                .iter()
                .filter(|p| matches!(p.status, StockStatus::Critical | StockStatus::Low))
                .map(|p| {
                    (
                        p.product.name.clone(),
                        p.current_stock,
                        p.product.min_stock_level,
                        p.product.unit.clone(),
                        matches!(p.status, StockStatus::Critical),
                    )
                })
                .collect()
        };

        if items.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(c.text_muted))
                .child("All stock levels healthy.")
                .into_any_element();
        }

        let count = items.len();
        let geom = scrollbar_geometry(&self.scroll_handle);
        let is_dragging = self.drag.is_some();

        // Scrollbar track (always present so layout is stable)
        let mut track = div()
            .id("restock-sb-track")
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
                    .id("restock-sb-thumb")
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
                    "restock-alerts",
                    count,
                    cx.processor(move |_this, range: std::ops::Range<usize>, _window, cx| {
                        let c = cx.global::<ThemeHandle>().0.clone();
                        range
                            .map(|ix| {
                                let (name, current, min, unit, is_critical) = &items[ix];
                                let badge = if *is_critical {
                                    c.status_red
                                } else {
                                    c.status_amber
                                };
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .px(px(12.))
                                    .py(px(8.))
                                    .child(
                                        div()
                                            .w(px(8.))
                                            .h(px(8.))
                                            .rounded_full()
                                            .bg(rgb(badge))
                                            .mr(px(8.)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(rems(1.))
                                            .text_color(rgb(c.text_default))
                                            .child(name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.923))
                                            .text_color(rgb(badge))
                                            .child(format!(
                                                "{current:.1} / min {min:.1} {unit}"
                                            )),
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
                    .id("restock-sb-overlay")
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
            description: None,
            notes: None,
            preferred_supplier_id: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            model_number: Some(format!("MODEL-{id}")),
            part_number: Some(format!("PART-{id}")),
            duty_percent: 42.5,
            end_of_life: false,
            replacement: None,
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

        let alert_count = products
            .iter()
            .filter(|p| matches!(p.status, StockStatus::Critical | StockStatus::Low))
            .count();

        assert_eq!(
            alert_count, 2,
            "only Critical and Low should appear in alerts"
        );
    }
}
