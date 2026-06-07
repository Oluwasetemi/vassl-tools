use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
           MouseUpEvent, Render, Window,
           div, prelude::*, px, rems, rgb, uniform_list, UniformListScrollHandle};
use vassl_core::Supplier;
use vassl_ui::{ScrollDragState, ThemeColors, ThemeHandle, scrollbar_geometry};

use crate::store::SupplierStore;

const TRACK_W: f32 = 14.0;

pub struct SupplierList {
    store: Entity<SupplierStore>,
    pub scroll_handle: UniformListScrollHandle,
    drag: Option<ScrollDragState>,
}

impl SupplierList {
    pub fn new(store: Entity<SupplierStore>, _cx: &mut Context<Self>) -> Self {
        Self { store, scroll_handle: UniformListScrollHandle::default(), drag: None }
    }
}

pub fn format_supplier_row(s: &Supplier) -> String {
    let extra = match (&s.email, &s.phone) {
        (Some(e), Some(p)) => format!("  {e}  {p}"),
        (Some(e), None)    => format!("  {e}"),
        (None, Some(p))    => format!("  {p}"),
        (None, None)       => String::new(),
    };
    format!("{}{extra}", s.name)
}

impl Render for SupplierList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c     = cx.global::<ThemeHandle>().0.clone();
        let store = self.store.read(cx);

        if store.loading {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child("Loading…")
                .into_any_element();
        }

        if store.suppliers.is_empty() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_default))
                .child("No suppliers — add one to get started.")
                .into_any_element();
        }

        let filtered = store.filtered_suppliers();
        if filtered.is_empty() && !store.suppliers.is_empty() {
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
            .id("supplier-sb-track")
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
                    .id("supplier-sb-thumb")
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
                    "supplier-list",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let store = this.store.read(cx);
                        let filtered = store.filtered_suppliers();
                        let c = cx.global::<ThemeHandle>().0.clone();
                        let selected = store.selected_supplier_id;
                        range.map(|ix| {
                            let s = &filtered[ix];
                            supplier_row(s, selected == Some(s.id), store_entity.clone(), &c)
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
                    .id("supplier-sb-overlay")
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

fn supplier_row(s: &Supplier, selected: bool, store: Entity<SupplierStore>, c: &ThemeColors) -> impl IntoElement {
    let id       = s.id;
    let row_bg   = if selected { c.surface_active } else { c.canvas_bg };
    let hover_bg = rgb(c.surface_hover);
    let contact = match (&s.email, &s.phone) {
        (Some(e), Some(p)) => format!("{e}  {p}"),
        (Some(e), None)    => e.clone(),
        (None, Some(p))    => p.clone(),
        (None, None)       => String::new(),
    };

    div()
        .id(format!("supplier-{id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(7.))
        .bg(rgb(row_bg))
        .when(!selected, |d| d.hover(move |s| s.bg(hover_bg)))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_supplier(id, cx));
            },
        )
        .child(
            div()
                .flex_1()
                .text_size(rems(1.))
                .text_color(rgb(c.text_default))
                .child(s.name.clone())
        )
        .when(!contact.is_empty(), |el| {
            el.child(
                div()
                    .text_size(rems(0.846))
                    .text_color(rgb(c.text_muted))
                    .child(contact)
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_supplier(name: &str, email: Option<&str>, phone: Option<&str>) -> Supplier {
        Supplier {
            id: 1, name: name.to_string(),
            contact_person: None,
            email: email.map(String::from),
            phone: phone.map(String::from),
            notes: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn supplier_row_shows_name_and_email() {
        let s = make_supplier("Acme Ltd", Some("a@acme.com"), None);
        let row = format_supplier_row(&s);
        assert!(row.contains("Acme Ltd"));
        assert!(row.contains("a@acme.com"));
    }

    #[test]
    fn supplier_row_no_email_shows_name_only() {
        let s = make_supplier("Beta Corp", None, None);
        let row = format_supplier_row(&s);
        assert!(row.contains("Beta Corp"));
    }
}
