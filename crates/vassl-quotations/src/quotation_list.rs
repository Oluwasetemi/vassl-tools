use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
           MouseUpEvent, Render, Window,
           div, prelude::*, px, rems, rgb, uniform_list, UniformListScrollHandle};
use vassl_ui::{ScrollDragState, ThemeColors, ThemeHandle, scrollbar_geometry};

use crate::db::QuotationRow;
use crate::store::{QuotationStore, status_badge_color};

const TRACK_W: f32 = 14.0;

pub struct QuotationList {
    store: Entity<QuotationStore>,
    pub scroll_handle: UniformListScrollHandle,
    drag: Option<ScrollDragState>,
}

impl QuotationList {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self { store, scroll_handle: UniformListScrollHandle::default(), drag: None }
    }
}

pub fn format_total(usd: f64) -> String {
    format!("${usd:.2}")
}

impl Render for QuotationList {
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

        if store.quotations.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_default))
                .child("No quotations yet — click \"+ New Quotation\" to create one.")
                .into_any_element();
        }

        let count = store.quotations.len();
        let store_entity = self.store.clone();

        let geom = scrollbar_geometry(&self.scroll_handle);
        let is_dragging = self.drag.is_some();

        let mut track = div()
            .id("quot-sb-track")
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
                    .id("quot-sb-thumb")
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
                    "quotation-list",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let store = this.store.read(cx);
                        let c = cx.global::<ThemeHandle>().0.clone();
                        let selected = store.selected_id;
                        range.map(|ix| {
                            let q = &store.quotations[ix];
                            quotation_row(q, selected == Some(q.id), store_entity.clone(), &c)
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
                    .id("quot-sb-overlay")
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

fn quotation_row(q: &QuotationRow, selected: bool, store: Entity<QuotationStore>, c: &ThemeColors) -> impl IntoElement {
    let id        = q.id;
    let row_bg    = if selected { c.surface_active } else { c.canvas_bg };
    let hover_bg  = rgb(c.surface_hover);
    let badge_col = status_badge_color(q.status.clone());
    let date_str  = q.created_at.get(..10).unwrap_or("").to_string();

    div()
        .id(format!("quot-row-{id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(6.))
        .bg(rgb(row_bg))
        .when(!selected, |d| d.hover(move |s| s.bg(hover_bg)))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_quotation(id, cx));
            },
        )
        // Status badge dot
        .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(badge_col)).mr(px(8.)))
        // Reference number
        .child(
            div().w(px(130.)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                .child(q.reference_number.clone())
        )
        // Project + client
        .child(
            div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_muted))
                .child(format!("{} / {}", q.project_name, q.client_name))
        )
        // Total
        .child(
            div().w(px(90.)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                .child(format_total(q.total_usd))
        )
        // Date
        .child(
            div().w(px(90.)).text_size(rems(0.846)).text_color(rgb(c.text_muted))
                .child(date_str)
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;
    use crate::db::QuotationRow;

    fn make_row(id: i64, ref_num: &str, total: f64) -> QuotationRow {
        QuotationRow {
            id,
            reference_number: ref_num.to_string(),
            status:       QuotationStatus::Draft,
            project_id:   1,
            project_name: "Test Project".to_string(),
            client_name:  "Client A".to_string(),
            total_usd:    total,
            created_at:   "2026-01-01T00:00:00Z".to_string(),
            notes:        None,
        }
    }

    #[test]
    fn format_total_two_decimal_places() {
        let row = make_row(1, "VASSL-2026-0001", 1234.5);
        let formatted = format_total(row.total_usd);
        assert_eq!(formatted, "$1234.50");
    }

    #[test]
    fn format_date_trims_to_10_chars() {
        let row = make_row(1, "VASSL-2026-0001", 0.0);
        let date = &row.created_at[..10];
        assert_eq!(date.len(), 10);
        assert_eq!(date, "2026-01-01");
    }
}
