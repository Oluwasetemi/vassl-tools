use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::selling_price;
use vassl_ui::{TextInput, text_field};

use crate::colors;
use crate::db::PriceBookDb;
use crate::store::PriceBookStore;

#[derive(Debug)]
pub enum PriceFormEvent { Submitted, Cancelled }

impl EventEmitter<PriceFormEvent> for PriceEntryForm {}

pub struct PriceEntryForm {
    store:        Entity<PriceBookStore>,
    product_id:   i64,
    product_name: String,
    cost:         Entity<TextInput>,
    duty:         Entity<TextInput>,
    markup:       Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

fn validate_price_entry(cost: &str, duty: &str, markup: &str) -> Result<(f64, f64, f64), String> {
    let c: f64 = cost.trim().parse().map_err(|_| "Cost must be a number ≥ 0".to_string())?;
    if c < 0.0 { return Err("Cost must be ≥ 0".to_string()); }
    let d: f64 = duty.trim().parse().map_err(|_| "Duty must be a number ≥ 0".to_string())?;
    if d < 0.0 { return Err("Duty must be ≥ 0".to_string()); }
    let m: f64 = markup.trim().parse().map_err(|_| "Markup % must be > 0".to_string())?;
    if m <= 0.0 { return Err("Markup % must be > 0".to_string()); }
    Ok((c, d, m))
}

impl PriceEntryForm {
    pub fn new(store: Entity<PriceBookStore>, product_id: i64, product_name: String, cx: &mut Context<Self>) -> Self {
        let markup_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 30", cx);
            f.set_text("30", cx);
            f
        });
        Self {
            store,
            product_id,
            product_name,
            cost:         cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            duty:         cx.new(|cx| TextInput::with_placeholder("e.g. 15.00", cx)),
            markup:       markup_field,
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn computed_selling_price(&self, cx: &Context<Self>) -> String {
        let c = self.cost.read(cx).text().to_string();
        let d = self.duty.read(cx).text().to_string();
        let m = self.markup.read(cx).text().to_string();
        match validate_price_entry(&c, &d, &m) {
            Ok((cv, dv, mv)) => match selling_price(cv, dv, mv) {
                Ok(s)  => format!("${s:.2}"),
                Err(_) => "—".to_string(),
            },
            Err(_) => "—".to_string(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let c = self.cost.read(cx).text().to_string();
        let d = self.duty.read(cx).text().to_string();
        let m = self.markup.read(cx).text().to_string();
        match validate_price_entry(&c, &d, &m) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((cv, dv, mv)) => {
                let sell  = selling_price(cv, dv, mv).unwrap_or(0.0);
                let db    = PriceBookDb::global(&**cx);
                let pid   = self.product_id;
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_entry(pid, cv, dv, mv, sell, None).await;
                    if let Err(e) = result { tracing::error!("insert_entry failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(PriceFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for PriceEntryForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for PriceEntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selling      = self.computed_selling_price(cx);
        let cost_focused = self.cost.read(cx).focus_handle.is_focused(window);
        let duty_focused = self.duty.read(cx).focus_handle.is_focused(window);
        let mrkp_focused = self.markup.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(420.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT))
                        .child(format!("New Price Entry — {}", self.product_name)))
                    .child(text_field("Cost Price (USD)", self.cost.clone(),   cost_focused, window))
                    .child(text_field("Duty Cost (USD)",  self.duty.clone(),   duty_focused, window))
                    .child(text_field("Markup %",         self.markup.clone(), mrkp_focused, window))
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Selling Price (computed)"))
                            .child(div().px(px(8.)).py(px(6.)).bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                .text_size(px(13.)).text_color(rgb(colors::STATUS_GREEN)).child(selling))
                    )
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("pb-btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(PriceFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("pb-btn-save").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_price_entry;
    #[test] fn rejects_empty_cost()      { assert!(validate_price_entry("", "0", "30").is_err()); }
    #[test] fn rejects_negative_cost()   { assert!(validate_price_entry("-1", "0", "30").is_err()); }
    #[test] fn rejects_zero_markup()     { assert!(validate_price_entry("100", "0", "0").is_err()); }
    #[test] fn rejects_negative_markup() { assert!(validate_price_entry("100", "0", "-5").is_err()); }
    #[test] fn accepts_valid()           { assert!(validate_price_entry("100.0", "10.0", "30.0").is_ok()); }
    #[test] fn accepts_zero_duty()       { assert!(validate_price_entry("200.0", "0.0", "25.0").is_ok()); }
}
