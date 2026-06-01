use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::selling_price;
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::colors;

actions!(price_form, [EscapeForm, TabField, BackTabField]);
use crate::db::PriceBookDb;
use crate::store::PriceBookStore;

#[derive(Debug)]
pub enum PriceFormEvent { Submitted, Cancelled }

impl EventEmitter<PriceFormEvent> for PriceEntryForm {}

pub struct PriceEntryForm {
    store:        Entity<PriceBookStore>,
    product_id:   i64,
    product_name: String,
    pub cost:     Entity<TextInput>,
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
        let c = cx.global::<ThemeHandle>().0.clone();
        let selling      = self.computed_selling_price(cx);
        let cost_focused = self.cost.read(cx).focus_handle.is_focused(window);
        let duty_focused = self.duty.read(cx).focus_handle.is_focused(window);
        let mrkp_focused = self.markup.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("PriceEntryForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(PriceFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.cost.read(cx).focus_handle.clone(),
                    this.duty.read(cx).focus_handle.clone(),
                    this.markup.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.cost.read(cx).focus_handle.clone(),
                    this.duty.read(cx).focus_handle.clone(),
                    this.markup.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(560.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1()
                                .text_size(px(13.)).text_color(rgb(c.text_default))
                                .child(format!("New Price Entry — {}", self.product_name)))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Cost Price (USD)"))
                                    .child(div().flex_1().child(text_field("", self.cost.clone(), cost_focused, window)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Duty Cost (USD)"))
                                    .child(div().flex_1().child(text_field("", self.duty.clone(), duty_focused, window)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Markup %"))
                                    .child(div().flex_1().child(text_field("", self.markup.clone(), mrkp_focused, window)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Selling Price"))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(6.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(px(13.)).text_color(rgb(c.status_green)).child(selling))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(px(11.)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    // ── footer ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("pb-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(PriceFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("pb-btn-save").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save Entry"))
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
