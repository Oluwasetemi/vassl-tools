use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           MouseButton, MouseDownEvent, Render, Window, actions, div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_pricebook::store::ProductPrice;
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::db::QuotationDb;
use crate::store::QuotationStore;

actions!(line_item_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum LineItemFormEvent { Submitted, Cancelled }

impl EventEmitter<LineItemFormEvent> for LineItemForm {}

pub struct LineItemForm {
    store:            Entity<QuotationStore>,
    quotation_id:     i64,
    products:         Vec<ProductPrice>,
    selected_product: Option<i64>,
    pub description:  Entity<TextInput>,
    pub quantity:     Entity<TextInput>,
    unit:             Entity<TextInput>,
    unit_price:       Entity<TextInput>,
    discount_percent: Entity<TextInput>,
    cancel_focus:     FocusHandle,
    save_focus:       FocusHandle,
    error:            Option<String>,
    focus_handle:     FocusHandle,
}


pub fn validate_line_item(description: &str, quantity: &str, unit_price: &str, discount_pct: &str) -> Result<(String, f64, f64, f64, f64), String> {
    let desc = description.trim();
    if desc.is_empty() {
        return Err("Description cannot be empty".to_string());
    }

    let qty: f64 = quantity.trim().parse().map_err(|_| "Quantity must be a valid number".to_string())?;

    if qty <= 0.0 {
        return Err("Quantity must be greater than 0".to_string());
    }

    let price: f64 = unit_price.trim().parse().map_err(|_| "Unit price must be a valid number".to_string())?;

    if price < 0.0 {
        return Err("Unit price must be ≥ 0".to_string());
    }

    let disc: f64 = discount_pct.trim().parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0);
    let total = (qty * price * 100.0).round() / 100.0;

    Ok((desc.to_string(), qty, price, disc, total))
}

impl LineItemForm {
    pub fn new(
        store:        Entity<QuotationStore>,
        quotation_id: i64,
        products:     Vec<ProductPrice>,
        cx:           &mut Context<Self>,
    ) -> Self {
        Self {
            store,
            quotation_id,
            products,
            selected_product: None,
            description:      cx.new(|cx| TextInput::with_placeholder("e.g. Paint supplies", cx)),
            quantity:         cx.new(|cx| TextInput::with_placeholder("e.g. 5", cx)),
            unit:             cx.new(|cx| TextInput::with_placeholder("ea / lot / set…", cx)),
            unit_price:       cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            discount_percent: cx.new(|cx| TextInput::with_text("0.0", cx)),
            cancel_focus:     cx.focus_handle(),
            save_focus:       cx.focus_handle(),
            error:            None,
            focus_handle:     cx.focus_handle(),
        }
    }

    fn computed_total(&self, cx: &Context<Self>) -> String {
        let q: f64 = self.quantity.read(cx).text().trim().parse().unwrap_or(0.0);
        let u: f64 = self.unit_price.read(cx).text().trim().parse().unwrap_or(0.0);
        if q > 0.0 && u >= 0.0 { format!("${:.2}", q * u) } else { "—".to_string() }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let desc  = self.description.read(cx).text().to_string();
        let qty_s = self.quantity.read(cx).text().to_string();
        let up_s  = self.unit_price.read(cx).text().to_string();
        let disc_s = self.discount_percent.read(cx).text().to_string();
        let unit_s = self.unit.read(cx).text().trim().to_string();
        let unit   = if unit_s.is_empty() { None } else { Some(unit_s) };
        match validate_line_item(&desc, &qty_s, &up_s, &disc_s) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((description, quantity, unit_price, disc, total)) => {
                let qid = self.quotation_id;
                let pid = self.selected_product;
                let db  = QuotationDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_item(qid, pid, description, quantity, unit, unit_price, disc, total).await;
                    let _ = this.update(cx, |form, cx| {
                        match result {
                            Err(e) => {
                                tracing::error!("insert_item failed: {e:?}");
                                form.error = Some(format!("Save failed: {e}"));
                                cx.notify();
                            }
                            Ok(_) => {
                                let _ = store.update(cx, |s, cx| s.load_line_items(qid, cx));
                                cx.emit(LineItemFormEvent::Submitted);
                            }
                        }
                    });
                    Ok::<(), anyhow::Error>(())
                }).detach();
            }
        }
    }
}

impl Focusable for LineItemForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for LineItemForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let desc_focused = self.description.read(cx).focus_handle.is_focused(window);
        let qty_focused  = self.quantity.read(cx).focus_handle.is_focused(window);
        let unit_focused = self.unit.read(cx).focus_handle.is_focused(window);
        let up_focused   = self.unit_price.read(cx).focus_handle.is_focused(window);
        let disc_focused = self.discount_percent.read(cx).focus_handle.is_focused(window);
        let cancel_f     = self.cancel_focus.is_focused(window);
        let save_f       = self.save_focus.is_focused(window);
        let total        = self.computed_total(cx);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("LineItemForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(LineItemFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.description.read(cx).focus_handle.clone(),
                    this.quantity.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.unit_price.read(cx).focus_handle.clone(),
                    this.discount_percent.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.description.read(cx).focus_handle.clone(),
                    this.quantity.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.unit_price.read(cx).focus_handle.clone(),
                    this.discount_percent.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(580.))
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
                                .child("Add Line Item"))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── product picker ──────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.))
                            .child(
                                div().flex().flex_row().py(px(10.))
                                    .child(div().w(px(160.)).pt(px(2.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Product (optional)"))
                                    .child(
                                        div().id("item-product-picker").flex_1().h(px(110.)).overflow_y_scroll()
                                            .bg(rgb(c.surface_default)).rounded(px(4.))
                                            .children(self.products.iter().filter_map(|p| {
                                                let entry = p.latest.as_ref()?;
                                                let pid      = p.product_id;
                                                let selected = self.selected_product == Some(pid);
                                                let bg       = if selected { c.surface_active } else { c.surface_default };
                                                let name     = p.name.clone();
                                                let price    = entry.selling_price_usd;
                                                Some(div()
                                                    .id(format!("pick-item-product-{pid}"))
                                                    .flex().flex_row().items_center()
                                                    .px(px(10.)).py(px(5.))
                                                    .bg(rgb(bg)).cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                                        this.selected_product = Some(pid);
                                                        let name2 = name.clone();
                                                        this.description.update(cx, |d, cx| d.set_text(name2, cx));
                                                        this.unit_price.update(cx, |u, cx| u.set_text(format!("{price:.2}"), cx));
                                                        this.error = None;
                                                        cx.notify();
                                                    }))
                                                    .child(div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_default)).child(p.name.clone()))
                                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.status_green)).child(format!("${price:.2}"))))
                                            })
                                            )
                                    )
                            )
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pb(px(4.))
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Description"))
                                    .child(div().flex_1().child(text_field("", self.description.clone(), desc_focused, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Quantity"))
                                    .child(div().flex_1().child(text_field("", self.quantity.clone(), qty_focused, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Unit"))
                                    .child(div().flex_1().child(text_field("", self.unit.clone(), unit_focused, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Unit Price (USD)"))
                                    .child(div().flex_1().child(text_field("", self.unit_price.clone(), up_focused, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Discount %"))
                                    .child(div().flex_1().child(text_field("", self.discount_percent.clone(), disc_focused, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Total"))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(6.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(rems(1.)).text_color(rgb(c.status_green)).child(total))
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
                                div().id("item-btn-cancel")
                                    .track_focus(&self.cancel_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_default)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(cancel_f, |d| d.border_2().border_color(rgb(c.surface_active)))
                                    .when(!cancel_f, |d| d.border_1().border_color(rgb(c.surface_default)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(LineItemFormEvent::Cancelled); }))
                                    .child("Cancel")
                            )
                            .child(
                                div().id("item-btn-add")
                                    .track_focus(&self.save_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(save_f, |d| d.border_2().border_color(rgb(c.text_default)))
                                    .when(!save_f, |d| d.border_1().border_color(rgb(c.surface_active)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                    .child("Add Item")
                            )
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_line_item;

    #[test] fn rejects_empty_description() { assert!(validate_line_item("", "1", "10.0", "0").is_err()); }
    #[test] fn rejects_zero_quantity()     { assert!(validate_line_item("Desc", "0", "10.0", "0").is_err()); }
    #[test] fn rejects_negative_quantity() { assert!(validate_line_item("Desc", "-1", "10.0", "0").is_err()); }
    #[test] fn rejects_negative_price()    { assert!(validate_line_item("Desc", "2", "-5", "0").is_err()); }
    #[test] fn accepts_zero_price()        { assert!(validate_line_item("Desc", "3", "0", "0").is_ok()); }
    #[test] fn computes_total_correctly()  {
        let (_, qty, up, disc, total) = validate_line_item("Item", "4", "25.0", "10").unwrap();
        assert_eq!(qty, 4.0);
        assert_eq!(up, 25.0);
        assert_eq!(disc, 10.0);
        assert!((total - 100.0).abs() < 1e-9);
    }
}
