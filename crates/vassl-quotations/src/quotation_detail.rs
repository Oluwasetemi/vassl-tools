use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rems, rgb};
use vassl_core::QuotationStatus;
use vassl_ui::ThemeHandle;

use crate::store::{QuotationStore, status_badge_color};

pub struct QuotationDetail {
    store: Entity<QuotationStore>,
}

impl QuotationDetail {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

pub fn next_transitions(status: QuotationStatus) -> Vec<QuotationStatus> {
    match status {
        QuotationStatus::Draft    => vec![QuotationStatus::Sent],
        QuotationStatus::Sent     => vec![QuotationStatus::Accepted, QuotationStatus::Rejected],
        QuotationStatus::Accepted => vec![],
        QuotationStatus::Rejected => vec![],
    }
}

pub fn transition_label(status: &QuotationStatus) -> &'static str {
    match status {
        QuotationStatus::Draft    => "Mark as Draft",
        QuotationStatus::Sent     => "Mark as Sent",
        QuotationStatus::Accepted => "Mark as Accepted",
        QuotationStatus::Rejected => "Mark as Rejected",
    }
}

impl Render for QuotationDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let (selected_id, current_status, items, total) = {
            let store = self.store.read(cx);
            let sid   = store.selected_id;
            let status = store.quotations.iter()
                .find(|q| Some(q.id) == sid)
                .map(|q| q.status.clone());
            let total: f64 = store.line_items.iter().map(|i| i.total_usd).sum();
            (sid, status, store.line_items.clone(), total)
        };

        if selected_id.is_none() {
            return div()
                .flex_1().flex().items_center().justify_center()
                .text_color(rgb(c.text_muted))
                .child("Select a quotation to view its line items.")
                .into_any_element();
        }

        let mut root = div().flex_1().flex().flex_col();

        // Status transition buttons
        if let Some(ref status) = current_status {
            let transitions = next_transitions(status.clone());
            if !transitions.is_empty() {
                let id = selected_id.unwrap();
                let btn_row = div()
                    .flex().flex_row().gap(px(8.))
                    .px(px(12.)).py(px(8.))
                    .children(transitions.into_iter().map(|next_status| {
                        let store = self.store.clone();
                        let ns = next_status.clone();
                        div()
                            .id(format!("status-btn-{}", transition_label(&next_status)))
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(status_badge_color(next_status.clone())))
                            .text_size(rems(0.923)).text_color(rgb(c.canvas_bg))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left,
                                move |_, _, cx: &mut gpui::App| {
                                    store.update(cx, |s, cx| s.transition_status(id, ns.clone(), cx));
                                })
                            .child(transition_label(&next_status).to_string())
                    }));
                root = root.child(btn_row);
            }
        }

        let can_delete = matches!(current_status, Some(QuotationStatus::Draft) | Some(QuotationStatus::Sent) | Some(QuotationStatus::Accepted));

        // Line items header
        root = root.child(
            div()
                .flex().flex_row().items_center()
                .px(px(12.)).py(px(4.))
                .bg(rgb(c.surface_default))
                .child(div().flex_1().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Description"))
                .child(div().w(px(70.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Qty"))
                .child(div().w(px(90.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Unit Price"))
                .child(div().w(px(90.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Total"))
                .child(div().w(px(28.)))  // column for delete button
        );

        if items.is_empty() {
            root = root.child(
                div()
                    .flex_1().flex().items_center().justify_center()
                    .text_color(rgb(c.text_muted))
                    .child("No line items yet — use \"Add Item\" above to add one.")
            );
        } else {
            let store = self.store.clone();
            let item_rows = div()
                .id("items-scroll").flex_1().flex().flex_col().overflow_y_scroll()
                .children(items.iter().map(|item| {
                    let item_id  = item.id;
                    let store2   = store.clone();
                    div()
                        .flex().flex_row().items_center().w_full()
                        .px(px(12.)).py(px(6.))
                        .child(div().flex_1().text_size(rems(1.)).text_color(rgb(c.text_default)).child(item.description.clone()))
                        .child(div().w(px(70.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("{:.2}", item.quantity)))
                        .child(div().w(px(90.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("${:.2}", item.unit_price_usd)))
                        .child(div().w(px(90.)).text_size(rems(0.923)).text_color(rgb(c.status_green)).child(format!("${:.2}", item.total_usd)))
                        .child(
                            div()
                                .id(format!("del-item-{item_id}"))
                                .w(px(28.)).flex().items_center().justify_center()
                                .when(can_delete, move |d| {
                                    d.px(px(6.)).py(px(2.)).rounded(px(3.))
                                        .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(rgb(c.status_red)))
                                        .on_mouse_down(gpui::MouseButton::Left,
                                            move |_, _, cx: &mut gpui::App| {
                                                store2.update(cx, |s, cx| s.delete_item(item_id, cx));
                                            })
                                        .child("×")
                                })
                        )
                }));
            root = root.child(item_rows);
        }

        // Total footer
        root.child(
            div()
                .flex().flex_row().justify_end()
                .px(px(12.)).py(px(8.))
                .bg(rgb(c.surface_default))
                .child(
                    div().text_size(rems(1.)).text_color(rgb(c.status_green))
                        .child(format!("Total: ${total:.2}"))
                )
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;

    #[test]
    fn next_status_transitions_are_correct() {
        assert_eq!(next_transitions(QuotationStatus::Draft),    vec![QuotationStatus::Sent]);
        assert_eq!(next_transitions(QuotationStatus::Sent),     vec![QuotationStatus::Accepted, QuotationStatus::Rejected]);
        assert!(next_transitions(QuotationStatus::Accepted).is_empty());
        assert!(next_transitions(QuotationStatus::Rejected).is_empty());
    }

    #[test]
    fn transition_label_is_human_readable() {
        assert_eq!(transition_label(&QuotationStatus::Sent),     "Mark as Sent");
        assert_eq!(transition_label(&QuotationStatus::Accepted), "Mark as Accepted");
        assert_eq!(transition_label(&QuotationStatus::Rejected), "Mark as Rejected");
    }
}
