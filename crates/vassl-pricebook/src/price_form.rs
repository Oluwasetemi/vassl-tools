use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_core::selling_price;
use vassl_inventory::InventoryStoreHandle;
use vassl_ui::{TextInput, ThemeHandle, text_field};


actions!(price_form, [EscapeForm, TabField, BackTabField]);
use crate::db::PriceBookDb;
use crate::store::PriceBookStore;

#[derive(Debug)]
pub enum PriceFormEvent { Submitted, Cancelled }

impl EventEmitter<PriceFormEvent> for PriceEntryForm {}

pub struct PriceEntryForm {
    store:         Entity<PriceBookStore>,
    product_id:    i64,
    product_name:  String,
    duty_percent:  f64,
    currency:      String,
    pub cost:      Entity<TextInput>,
    quantity:      Entity<TextInput>,
    markup:        Entity<TextInput>,
    cancel_focus:  FocusHandle,
    save_focus:    FocusHandle,
    error:         Option<String>,
    qty_error:     bool,
    cost_error:    bool,
    markup_error:  bool,
    edit_entry_id: Option<i64>,
}

/// Returns (qty, cost, markup_pct).
fn validate_price_entry(qty: &str, cost: &str, markup: &str) -> Result<(f64, f64, f64), String> {
    let q: f64 = qty.trim().parse().map_err(|_| "Quantity must be a number > 0".to_string())?;
    if q <= 0.0 { return Err("Quantity must be > 0".to_string()); }
    let c: f64 = cost.trim().parse().map_err(|_| "Cost must be a number ≥ 0".to_string())?;
    if c < 0.0 { return Err("Cost must be ≥ 0".to_string()); }
    let m: f64 = markup.trim().parse().map_err(|_| "Markup % must be > 0".to_string())?;
    if m <= 0.0 { return Err("Markup % must be > 0".to_string()); }
    Ok((q, c, m))
}

impl PriceEntryForm {
    pub fn new(store: Entity<PriceBookStore>, product_id: i64, product_name: String, duty_percent: f64, current_stock: f64, cx: &mut Context<Self>) -> Self {
        let markup_field = cx.new(|cx| {
            let db = vassl_db::AppDatabase::global(&**cx);
            let default = vassl_db::shared::get_setting(db, "pricebook.default_margin")
                .ok().flatten().unwrap_or_else(|| "30".into());
            let mut f = TextInput::with_placeholder("e.g. 30", cx);
            f.set_text(default, cx);
            f
        });
        let quantity_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 10", cx);
            if current_stock > 0.0 {
                f.set_text(format!("{:.0}", current_stock), cx);
            }
            f
        });
        let db       = vassl_db::AppDatabase::global(&**cx);
        let currency = vassl_db::shared::get_setting(db, "pricebook.currency")
            .ok().flatten().unwrap_or_else(|| "USD".into());
        Self {
            store,
            product_id,
            product_name,
            duty_percent,
            currency,
            cost:         cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            quantity:     quantity_field,
            markup:       markup_field,
            cancel_focus:  cx.focus_handle(),
            save_focus:    cx.focus_handle(),
            error:         None,
            qty_error:     false,
            cost_error:    false,
            markup_error:  false,
            edit_entry_id: None,
        }
    }

    pub fn edit_for(
        store:         Entity<PriceBookStore>,
        product_id:    i64,
        product_name:  String,
        current_stock: f64,
        duty_percent:  f64,
        entry:         vassl_core::PriceEntry,
        cx:            &mut Context<Self>,
    ) -> Self {
        let entry_cost    = entry.cost_price_usd.to_string();
        let entry_qty     = entry.quantity.to_string();
        let entry_markup  = entry.markup_percent.to_string();
        let entry_currency = entry.currency.clone();
        let entry_id      = entry.id;

        let quantity_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 10", cx);
            f.set_text(entry_qty, cx);
            f
        });
        let markup_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 30", cx);
            f.set_text(entry_markup, cx);
            f
        });
        let cost_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 120.00", cx);
            f.set_text(entry_cost, cx);
            f
        });
        let _ = current_stock;

        Self {
            store,
            product_id,
            product_name,
            duty_percent,
            currency:      entry_currency,
            cost:          cost_field,
            quantity:      quantity_field,
            markup:        markup_field,
            cancel_focus:  cx.focus_handle(),
            save_focus:    cx.focus_handle(),
            error:         None,
            qty_error:     false,
            cost_error:    false,
            markup_error:  false,
            edit_entry_id: Some(entry_id),
        }
    }

    fn computed_selling_price(&self, cx: &Context<Self>) -> String {
        let q = self.quantity.read(cx).text().to_string();
        let c = self.cost.read(cx).text().to_string();
        let m = self.markup.read(cx).text().to_string();
        match validate_price_entry(&q, &c, &m) {
            Ok((_qv, cv, mv)) => {
                let duty_usd = cv * self.duty_percent / 100.0;
                match selling_price(cv, duty_usd, mv) {
                    Ok(s)  => format!("${s:.2}"),
                    Err(_) => "—".to_string(),
                }
            }
            Err(_) => "—".to_string(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let q = self.quantity.read(cx).text().to_string();
        let c = self.cost.read(cx).text().to_string();
        let m = self.markup.read(cx).text().to_string();
        self.qty_error    = q.trim().parse::<f64>().map_or(true, |v| v <= 0.0);
        self.cost_error   = c.trim().parse::<f64>().map_or(true, |v| v < 0.0);
        self.markup_error = m.trim().parse::<f64>().map_or(true, |v| v <= 0.0);
        match validate_price_entry(&q, &c, &m) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((qv, cv, mv)) => {
                let duty_usd = cv * self.duty_percent / 100.0;
                let sell     = selling_price(cv, duty_usd, mv).unwrap_or(0.0);
                let db        = PriceBookDb::global(&**cx);
                let pid       = self.product_id;
                let store     = self.store.clone();
                let inv_store = cx.global::<InventoryStoreHandle>().0.clone();
                let currency  = self.currency.clone();
                let edit_id   = self.edit_entry_id;
                cx.spawn(async move |this, cx| {
                    let result = if let Some(entry_id) = edit_id {
                        db.update_price_entry(entry_id, qv, cv, duty_usd, mv, sell, None, &currency).await
                            .map(|_| entry_id)
                    } else {
                        db.insert_entry(pid, qv, cv, duty_usd, mv, sell, None, &currency).await
                    };
                    let _ = this.update(cx, |form, cx| {
                        match result {
                            Err(e) => {
                                tracing::error!("price entry save failed: {e:?}");
                                form.error = Some(format!("Save failed: {e}"));
                                cx.notify();
                            }
                            Ok(_) => {
                                let _ = store.update(cx, |s, cx| s.load_products(cx));
                                if edit_id.is_none() {
                                    let _ = inv_store.update(cx, |s, cx| s.load_products(cx));
                                }
                                cx.emit(PriceFormEvent::Submitted);
                            }
                        }
                    });
                    Ok::<(), anyhow::Error>(())
                }).detach();
            }
        }
    }
}

impl Focusable for PriceEntryForm {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle { self.cost.read(cx).focus_handle.clone() }
}

impl Render for PriceEntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let selling      = self.computed_selling_price(cx);
        let qty_focused  = self.quantity.read(cx).focus_handle.is_focused(window);
        let cost_focused = self.cost.read(cx).focus_handle.is_focused(window);
        let mrkp_focused = self.markup.read(cx).focus_handle.is_focused(window);
        let cancel_f     = self.cancel_focus.is_focused(window);
        let save_f       = self.save_focus.is_focused(window);
        let is_jmd       = self.currency == "JMD";
        let duty_pct     = self.duty_percent;

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .key_context("PriceEntryForm")
            .on_action(cx.listener(|_, _: &EscapeForm, window, cx| {
                let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                window.focus(&root, cx);
                cx.emit(PriceFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.quantity.read(cx).focus_handle.clone(),
                    this.cost.read(cx).focus_handle.clone(),
                    this.markup.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.quantity.read(cx).focus_handle.clone(),
                    this.cost.read(cx).focus_handle.clone(),
                    this.markup.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
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
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .rounded_t(px(10.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1()
                                .text_size(rems(1.)).text_color(rgb(c.text_default))
                                .child(if self.edit_entry_id.is_some() {
                                    format!("Edit Price Entry — {}", self.product_name)
                                } else {
                                    format!("New Price Entry — {}", self.product_name)
                                }))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Quantity"))
                                    .child(div().flex_1().child(text_field("", self.quantity.clone(), qty_focused, self.qty_error, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                        .child(if is_jmd { "Cost Price (JMD)" } else { "Cost Price (USD)" }))
                                    .child(div().flex().flex_row().items_center().gap(px(8.)).flex_1()
                                        .child(div().flex_1().child(text_field("", self.cost.clone(), cost_focused, self.cost_error, cx)))
                                        // Currency toggle — USD / JMD
                                        .child({
                                            let usd_bg = if !is_jmd { c.surface_active } else { c.surface_default };
                                            let jmd_bg = if is_jmd  { c.surface_active } else { c.surface_default };
                                            div().flex().flex_row().items_center().gap(px(2.))
                                                .child(
                                                    div().id("pb-cur-usd")
                                                        .px(px(8.)).py(px(4.)).rounded(px(4.))
                                                        .bg(rgb(usd_bg)).cursor_pointer()
                                                        .text_size(rems(0.846)).text_color(rgb(c.text_default))
                                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.currency = "USD".into();
                                                            cx.notify();
                                                        }))
                                                        .child("USD")
                                                )
                                                .child(
                                                    div().id("pb-cur-jmd")
                                                        .px(px(8.)).py(px(4.)).rounded(px(4.))
                                                        .bg(rgb(jmd_bg)).cursor_pointer()
                                                        .text_size(rems(0.846)).text_color(rgb(c.text_default))
                                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.currency = "JMD".into();
                                                            cx.notify();
                                                        }))
                                                        .child("JMD")
                                                )
                                        })
                                    )
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // Duty % — read-only from product
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Duty %"))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(4.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                        .child(format!("{duty_pct:.1}% (from product)")))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Markup %"))
                                    .child(div().flex_1().child(text_field("", self.markup.clone(), mrkp_focused, self.markup_error, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Selling Price"))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(4.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(rems(1.)).text_color(rgb(c.status_green)).child(selling))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.status_red))
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
                            .child(
                                div().id("pb-btn-cancel")
                                    .track_focus(&self.cancel_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_default)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(cancel_f, |d| d.border_2().border_color(rgb(c.surface_active)))
                                    .when(!cancel_f, |d| d.border_1().border_color(rgb(c.surface_default)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, window, cx| {
                                        let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                                        window.focus(&root, cx);
                                        cx.emit(PriceFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div().id("pb-btn-save")
                                    .track_focus(&self.save_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(save_f, |d| d.border_2().border_color(rgb(c.text_default)))
                                    .when(!save_f, |d| d.border_1().border_color(rgb(c.surface_active)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                    .child(if self.edit_entry_id.is_some() { "Update Entry" } else { "Save Entry" })
                            )
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_price_entry;
    #[test] fn rejects_zero_quantity()      { assert!(validate_price_entry("0", "100", "30").is_err()); }
    #[test] fn rejects_empty_cost()         { assert!(validate_price_entry("1", "", "30").is_err()); }
    #[test] fn rejects_negative_cost()      { assert!(validate_price_entry("1", "-1", "30").is_err()); }
    #[test] fn rejects_zero_markup()        { assert!(validate_price_entry("1", "100", "0").is_err()); }
    #[test] fn rejects_negative_markup()    { assert!(validate_price_entry("1", "100", "-5").is_err()); }
    #[test] fn accepts_valid()              { assert!(validate_price_entry("10", "100.0", "30.0").is_ok()); }
    #[test] fn forty_pct_duty_calculation() {
        // product duty_percent=40, cost=100 → duty_usd=40, selling = (100+40) * 1.30 = 182
        let (_qv, cv, mv) = validate_price_entry("10", "100.0", "30.0").unwrap();
        let duty_pct = 40.0_f64;
        let duty_usd = cv * duty_pct / 100.0;
        let sell = vassl_core::selling_price(cv, duty_usd, mv).unwrap();
        assert!((sell - 182.0).abs() < 1e-9);
    }
}
