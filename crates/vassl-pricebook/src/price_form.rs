use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::selling_price;

use crate::colors;
use crate::db::PriceBookDb;
use crate::store::PriceBookStore;

#[derive(Debug)]
pub enum PriceFormEvent {
    Submitted,
    Cancelled,
}

impl EventEmitter<PriceFormEvent> for PriceEntryForm {}

pub struct PriceEntryForm {
    store:          Entity<PriceBookStore>,
    product_id:     i64,
    product_name:   String,
    cost:           String,
    duty:           String,
    markup:         String,
    error:          Option<String>,
    focus_handle:   FocusHandle,
}

fn validate_price_entry(cost: &str, duty: &str, markup: &str) -> Result<(f64, f64, f64), String> {
    let cost_val: f64 = cost.trim().parse()
        .map_err(|_| "Cost must be a number ≥ 0".to_string())?;
    if cost_val < 0.0 { return Err("Cost must be ≥ 0".to_string()); }

    let duty_val: f64 = duty.trim().parse()
        .map_err(|_| "Duty must be a number ≥ 0".to_string())?;
    if duty_val < 0.0 { return Err("Duty must be ≥ 0".to_string()); }

    let markup_val: f64 = markup.trim().parse()
        .map_err(|_| "Markup % must be > 0".to_string())?;
    if markup_val <= 0.0 { return Err("Markup % must be > 0".to_string()); }

    Ok((cost_val, duty_val, markup_val))
}

impl PriceEntryForm {
    pub fn new(
        store:        Entity<PriceBookStore>,
        product_id:   i64,
        product_name: String,
        cx:           &mut Context<Self>,
    ) -> Self {
        Self {
            store,
            product_id,
            product_name,
            cost:         String::new(),
            duty:         String::new(),
            markup:       "30".to_string(),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn computed_selling_price(&self) -> String {
        match validate_price_entry(&self.cost, &self.duty, &self.markup) {
            Ok((c, d, m)) => match selling_price(c, d, m) {
                Ok(s)  => format!("${s:.2}"),
                Err(_) => "—".to_string(),
            },
            Err(_) => "—".to_string(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match validate_price_entry(&self.cost, &self.duty, &self.markup) {
            Err(msg) => {
                self.error = Some(msg);
                cx.notify();
            }
            Ok((cost_val, duty_val, markup_val)) => {
                let sell = selling_price(cost_val, duty_val, markup_val).unwrap_or(0.0);
                let db    = PriceBookDb::global(&**cx);
                let pid   = self.product_id;
                let store = self.store.clone();

                cx.spawn(async move |this, cx| {
                    let result = db.insert_entry(pid, cost_val, duty_val, markup_val, sell, None).await;
                    if let Err(e) = result {
                        tracing::error!("insert_entry failed: {e:?}");
                        return Ok(());
                    }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(PriceFormEvent::Submitted))
                })
                .detach();
            }
        }
    }
}

impl Focusable for PriceEntryForm {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PriceEntryForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selling = self.computed_selling_price();

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(420.))
                    .bg(rgb(colors::CANVAS_BG))
                    .rounded(px(8.))
                    .p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    // Title
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .child(format!("New Price Entry — {}", self.product_name))
                    )
                    .child(price_field("Cost Price (USD)", &self.cost, "e.g. 120.00"))
                    .child(price_field("Duty Cost (USD)",  &self.duty, "e.g. 15.00"))
                    .child(price_field("Markup %",         &self.markup, "e.g. 30"))
                    // Computed selling price preview
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(
                                div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                                    .child("Selling Price (computed)")
                            )
                            .child(
                                div()
                                    .px(px(8.)).py(px(6.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .rounded(px(4.))
                                    .text_size(px(13.))
                                    .text_color(rgb(colors::STATUS_GREEN))
                                    .child(selling)
                            )
                    )
                    // Error message
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(colors::STATUS_RED))
                            .child(
                                self.error.as_deref()
                                    .map(SharedString::from)
                                    .unwrap_or_default()
                            )
                    )
                    // Buttons
                    .child(
                        div()
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(
                                div()
                                    .id("pb-btn-cancel")
                                    .px(px(16.)).py(px(6.)).rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .text_size(px(12.))
                                    .text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
                                        cx.emit(PriceFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div()
                                    .id("pb-btn-save")
                                    .px(px(16.)).py(px(6.)).rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_ACTIVE))
                                    .text_size(px(12.))
                                    .text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.submit(cx);
                                    }))
                                    .child("Save")
                            )
                    )
            )
    }
}

fn price_field(label: &str, value: &str, placeholder: &str) -> impl IntoElement {
    let display    = if value.is_empty() { placeholder } else { value };
    let text_color = if value.is_empty() { colors::TEXT_MUTED } else { colors::TEXT_DEFAULT };

    div().flex().flex_col().gap(px(4.))
        .child(
            div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child(label.to_string())
        )
        .child(
            div()
                .px(px(8.)).py(px(6.))
                .bg(rgb(colors::SURFACE_DEFAULT))
                .rounded(px(4.))
                .text_size(px(13.))
                .text_color(rgb(text_color))
                .child(display.to_string())
        )
}

#[cfg(test)]
mod tests {
    use super::validate_price_entry;

    #[test]
    fn validate_rejects_empty_cost() {
        assert!(validate_price_entry("", "0", "30").is_err());
    }

    #[test]
    fn validate_rejects_negative_cost() {
        assert!(validate_price_entry("-1", "0", "30").is_err());
    }

    #[test]
    fn validate_rejects_zero_markup() {
        assert!(validate_price_entry("100", "0", "0").is_err());
    }

    #[test]
    fn validate_rejects_negative_markup() {
        assert!(validate_price_entry("100", "0", "-5").is_err());
    }

    #[test]
    fn validate_accepts_valid_input() {
        let result = validate_price_entry("100.0", "10.0", "30.0");
        assert!(result.is_ok());
        let (cost, duty, markup) = result.unwrap();
        assert_eq!(cost, 100.0);
        assert_eq!(duty, 10.0);
        assert_eq!(markup, 30.0);
    }

    #[test]
    fn validate_accepts_zero_duty() {
        assert!(validate_price_entry("200.0", "0.0", "25.0").is_ok());
    }
}
