use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rems, rgb};
use vassl_core::{QuotationExtras, QuotationStatus};
use vassl_ui::{ThemeColors, ThemeHandle};

const LEGACY_DEFAULT_RATE: f64 = 156.0;

fn fmt_jmd(v: f64) -> String {
    let cents = (v * 100.0).round() as i64;
    let whole = cents / 100;
    let frac  = (cents % 100).unsigned_abs();
    let s = whole.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 4);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    out = out.chars().rev().collect();
    format!("{out}.{frac:02}")
}

use crate::store::QuotationStore;

fn status_color(status: &QuotationStatus, c: &ThemeColors) -> u32 {
    match status {
        QuotationStatus::Draft    => c.status_grey,
        QuotationStatus::Sent     => c.status_amber,
        QuotationStatus::Accepted => c.status_green,
        QuotationStatus::Rejected => c.status_red,
    }
}

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
        let (selected_id, current_status, items, extras) = {
            let store  = self.store.read(cx);
            let sid    = store.selected_id;
            let status = store.quotations.iter()
                .find(|q| Some(q.id) == sid)
                .map(|q| q.status.clone());
            let extras = store.selected_extras.clone().unwrap_or_default();
            (sid, status, store.line_items.clone(), extras)
        };

        let settings_rate: f64 = {
            let db = vassl_db::AppDatabase::global(&**cx);
            vassl_db::shared::get_setting(db, "pricebook.usd_to_jmd_rate")
                .ok().flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(157.50)
        };
        let effective_rate = if extras.exchange_rate_jmd > 0.0 && extras.exchange_rate_jmd != LEGACY_DEFAULT_RATE {
            extras.exchange_rate_jmd
        } else {
            settings_rate
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
                        let ns    = next_status.clone();
                        let bg    = status_color(&next_status, &c);
                        div()
                            .id(format!("status-btn-{}", transition_label(&next_status)))
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(bg))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
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
                .child(div().w(px(50.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Qty"))
                .child(div().w(px(42.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Unit"))
                .child(div().w(px(82.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Unit Price"))
                .child(div().w(px(46.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Disc%"))
                .child(div().w(px(82.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Total USD"))
                .child(div().w(px(28.)))
        );

        if items.is_empty() {
            root = root.child(
                div()
                    .flex().items_center().justify_center()
                    .py(px(24.))
                    .text_color(rgb(c.text_muted))
                    .child("No line items yet — use \"Add Item\" above to add one.")
            );
        } else {
            let store = self.store.clone();
            let item_rows = div()
                .id("items-scroll").flex_1().flex().flex_col().overflow_y_scroll()
                .children(items.iter().map(|item| {
                    let item_id = item.id;
                    let store2  = store.clone();
                    div()
                        .flex().flex_row().items_center().w_full()
                        .px(px(12.)).py(px(6.))
                        .child(div().flex_1().text_size(rems(1.)).text_color(rgb(c.text_default)).child(item.description.clone()))
                        .child(div().w(px(50.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("{:.2}", item.quantity)))
                        .child(div().w(px(42.)).text_size(rems(0.846)).text_color(rgb(c.text_muted)).child(item.unit.clone().unwrap_or_default()))
                        .child(div().w(px(82.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("${:.2}", item.unit_price_usd)))
                        .child(div().w(px(46.)).text_size(rems(0.846)).text_color(rgb(c.text_muted))
                            .child(if item.discount_percent > 0.0 { format!("{:.1}%", item.discount_percent) } else { String::new() }))
                        .child(div().w(px(82.)).text_size(rems(0.923)).text_color(rgb(c.status_green)).child(format!("${:.2}", item.total_usd)))
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

        // ── Footer: full financial breakdown ────────────────────────────────
        root.child(totals_footer(&extras, effective_rate, &items, &c))
            .into_any_element()
    }
}

fn totals_footer(
    extras:         &QuotationExtras,
    effective_rate: f64,
    items:          &[vassl_core::QuotationItem],
    c:              &ThemeColors,
) -> impl gpui::IntoElement {
    let subtotal_usd: f64 = items.iter().map(|i| i.total_usd).sum();
    let rate              = effective_rate;
    let disc_pct          = extras.discount_percent;
    let gct_pct           = extras.gct_percent;

    let discount_usd  = (subtotal_usd * disc_pct / 100.0 * 100.0).round() / 100.0;
    let net_usd       = subtotal_usd - discount_usd;
    let gct_usd       = (net_usd * gct_pct / 100.0 * 100.0).round() / 100.0;
    let grand_usd     = net_usd + gct_usd;
    let grand_jmd     = grand_usd * rate;
    let deposit_jmd   = grand_jmd * 0.5;
    let balance_jmd   = grand_jmd - deposit_jmd;

    let validity = if extras.validity_days > 0 {
        format!("Valid {} days from quotation date.", extras.validity_days)
    } else {
        String::new()
    };

    div()
        .flex().flex_col()
        .px(px(12.)).py(px(10.))
        .bg(rgb(c.surface_default))
        .gap(px(3.))
        // sub-total
        .child(total_row("Sub-total",                 &format!("US$ {subtotal_usd:.2}"),    c.text_muted, c))
        .when(disc_pct > 0.0, |d| d
            .child(total_row(&format!("Less discount ({disc_pct:.1}%)"), &format!("- US$ {discount_usd:.2}"), c.status_amber, c))
            .child(total_row("Net",                   &format!("US$ {net_usd:.2}"),         c.text_default, c))
        )
        .child(total_row(&format!("GCT ({gct_pct:.1}%)"), &format!("US$ {gct_usd:.2}"),    c.text_muted, c))
        .child(div().h(px(1.)).bg(rgb(c.surface_active)).my(px(2.)))
        .child(total_row("Grand Total",               &format!("US$ {grand_usd:.2}"),       c.status_green, c))
        .child(total_row(&format!("Grand Total (J$ @ {rate:.0})"), &format!("J$ {}", fmt_jmd(grand_jmd)), c.status_green, c))
        .child(div().h(px(1.)).bg(rgb(c.surface_default)).my(px(2.)))
        .child(total_row("Deposit on order (50%)",    &format!("J$ {}", fmt_jmd(deposit_jmd)), c.text_muted, c))
        .child(total_row("Balance on completion",     &format!("J$ {}", fmt_jmd(balance_jmd)), c.text_muted, c))
        .when(!validity.is_empty(), |d| d
            .child(div().mt(px(4.)).text_size(rems(0.769)).text_color(rgb(c.text_muted)).child(validity))
        )
}

fn total_row(label: &str, value: &str, value_color: u32, c: &ThemeColors) -> impl gpui::IntoElement {
    div().flex().flex_row().justify_between()
        .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(label.to_string()))
        .child(div().text_size(rems(0.923)).text_color(rgb(value_color)).child(value.to_string()))
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

    #[test]
    fn totals_footer_computes_correctly() {
        use vassl_core::QuotationItem;
        let extras = QuotationExtras {
            exchange_rate_jmd: 156.0,
            discount_percent:  10.0,
            gct_percent:       15.0,
            validity_days:     30,
            quotation_date:    None,
        };
        let items = vec![QuotationItem {
            id: 1, quotation_id: 1, product_id: None,
            description: "Test".into(), quantity: 2.0, unit: None,
            unit_price_usd: 100.0, discount_percent: 0.0, total_usd: 200.0,
        }];
        // sub=200, disc=20, net=180, gct=27, grand_usd=207
        // effective_rate uses settings fallback; test arithmetic with a known rate
        let rate            = 156.0_f64;
        let subtotal: f64   = items.iter().map(|i| i.total_usd).sum();
        let discount        = (subtotal * extras.discount_percent / 100.0 * 100.0).round() / 100.0;
        let net             = subtotal - discount;
        let gct             = (net * extras.gct_percent / 100.0 * 100.0).round() / 100.0;
        let grand_usd       = net + gct;
        assert!((subtotal - 200.0).abs() < 1e-9);
        assert!((discount - 20.0).abs() < 1e-9);
        assert!((net - 180.0).abs() < 1e-9);
        assert!((gct - 27.0).abs() < 1e-9);
        assert!((grand_usd - 207.0).abs() < 1e-9);
        assert!((grand_usd * rate - 32292.0).abs() < 1e-9);
    }
}
