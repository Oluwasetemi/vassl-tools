use gpui::{App, Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb};
use vassl_core::Supplier;
use vassl_ui::{ThemeColors, ThemeHandle};

use crate::store::SupplierStore;

pub struct SupplierList {
    store: Entity<SupplierStore>,
}

impl SupplierList {
    pub fn new(store: Entity<SupplierStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
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

        let selected = store.selected_supplier_id;
        let rows: Vec<_> = store.suppliers.iter().map(|s| {
            supplier_row(s, selected == Some(s.id), self.store.clone(), &c)
        }).collect();

        div()
            .id("supplier-list-scroll")
            .flex_1().flex().flex_col()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }
}

fn supplier_row(s: &Supplier, selected: bool, store: Entity<SupplierStore>, c: &ThemeColors) -> impl IntoElement {
    let id     = s.id;
    let row_bg = if selected { c.surface_active } else { c.canvas_bg };

    div()
        .id(format!("supplier-{id}"))
        .flex().flex_row().items_center().w_full()
        .px(px(12.)).py(px(7.))
        .bg(rgb(row_bg))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |s, cx| s.select_supplier(id, cx));
            },
        )
        .child(format_supplier_row(s))
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
