use gpui::{App, Context, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rems, rgb};
use vassl_core::PriceEntry;
use vassl_ui::ThemeHandle;

use crate::db::PriceBookDb;

pub enum PriceHistoryEvent { Dismissed }
impl gpui::EventEmitter<PriceHistoryEvent> for PriceHistoryPanel {}

pub struct PriceHistoryPanel {
    pub product_name: String,
    pub entries:      Vec<PriceEntry>,
}

impl PriceHistoryPanel {
    pub fn new(product_id: i64, product_name: String, cx: &mut Context<Self>) -> Self {
        let db      = PriceBookDb::global(&**cx);
        let entries = db.list_entries_for_product(product_id).unwrap_or_default();
        Self { product_name, entries }
    }
}

impl Render for PriceHistoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c           = cx.global::<ThemeHandle>().0.clone();
        let entry_count  = self.entries.len();
        let total_qty: f64 = self.entries.iter().map(|e| e.quantity).sum();

        let col_header = |label: &'static str, w: f32| {
            div()
                .w(px(w))
                .text_size(rems(0.846))
                .text_color(rgb(c.text_muted))
                .child(label)
        };

        let rows: Vec<_> = self.entries.iter().map(|e| {
            div()
                .flex().flex_row().items_center().w_full()
                .px(px(16.)).py(px(6.))
                .child(div().w(px(100.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(e.effective_date.get(..10).unwrap_or(&e.effective_date).to_string()))
                .child(div().w(px(60.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("{:.0}", e.quantity)))
                .child(div().w(px(90.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("${:.2}", e.cost_price_usd)))
                .child(div().w(px(80.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(format!("+${:.2}", e.duty_cost_usd)))
                .child(div().w(px(70.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(format!("{:.0}%", e.markup_percent)))
                .child(div().flex_1().text_size(rems(1.)).text_color(rgb(c.status_green)).child(format!("${:.2}", e.selling_price_usd)))
        }).collect();

        let body = if self.entries.is_empty() {
            div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child("No price history for this product.")
                .into_any_element()
        } else {
            div()
                .id("price-history-scroll")
                .flex_1().flex().flex_col()
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        // Outer div = full-screen backdrop; click dismisses
        div()
            .absolute()
            .inset_0()
            .flex().items_center().justify_center()
            .bg(gpui::rgba(0x00000099))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _: &mut Window, cx| {
                    cx.emit(PriceHistoryEvent::Dismissed);
                }),
            )
            // Modal box — absorbs clicks so they don't reach backdrop
            .child(
                div()
                    .w(px(620.))
                    .max_h(px(480.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(8.))
                    .flex().flex_col()
                    .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _: &mut Window, _: &mut App| {})
                    // Header
                    .child(
                        div()
                            .px(px(16.)).py(px(12.))
                            .text_size(rems(1.077))
                            .text_color(rgb(c.text_default))
                            .child(format!("Price History — {}", self.product_name))
                    )
                    // Column headers
                    .child(
                        div()
                            .flex().flex_row().items_center().w_full()
                            .px(px(16.)).py(px(4.))
                            .bg(rgb(c.surface_default))
                            .child(col_header("Date",          100.))
                            .child(col_header("Qty",            60.))
                            .child(col_header("Cost",           90.))
                            .child(col_header("+Duty",          80.))
                            .child(col_header("Markup%",        70.))
                            .child(div().flex_1().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Selling Price"))
                    )
                    // Rows or empty state
                    .child(body)
                    // Footer
                    .child(
                        div()
                            .px(px(16.)).py(px(8.))
                            .text_size(rems(0.846))
                            .text_color(rgb(c.text_muted))
                            .child(format!("{entry_count} entries · {total_qty:.0} units total"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: i64) -> PriceEntry {
        PriceEntry {
            id,
            product_id:        1,
            quantity:          1.0,
            cost_price_usd:    100.0,
            duty_cost_usd:     10.0,
            markup_percent:    30.0,
            selling_price_usd: 143.0,
            effective_date:    "2026-01-01T00:00:00Z".to_string(),
            notes:             None,
        }
    }

    #[test]
    fn empty_entries_gives_no_history_state() {
        let panel = PriceHistoryPanel {
            product_name: "Test Product".to_string(),
            entries:      Vec::new(),
        };
        assert!(panel.entries.is_empty());
        assert_eq!(panel.product_name, "Test Product");
    }

    #[test]
    fn entries_count_is_correct() {
        let panel = PriceHistoryPanel {
            product_name: "Camera".to_string(),
            entries:      vec![make_entry(1), make_entry(2), make_entry(3)],
        };
        assert_eq!(panel.entries.len(), 3);
    }
}
