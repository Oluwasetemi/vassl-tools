use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::AcquisitionType;
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::colors;

actions!(stock_form, [EscapeForm, TabField, BackTabField]);
use crate::db::InventoryDb;
use crate::store::InventoryStore;

#[derive(Debug)]
pub enum StockFormEvent { Submitted, Cancelled }

impl EventEmitter<StockFormEvent> for StockEntryForm {}

pub struct StockEntryForm {
    store:            Entity<InventoryStore>,
    product_id:       i64,
    product_name:     String,
    quantity:         Entity<TextInput>,
    unit_cost:        Entity<TextInput>,
    supplier:         Entity<TextInput>,
    invoice_ref:      Entity<TextInput>,
    acquisition_type: AcquisitionType,
    error:            Option<String>,
    focus_handle:     FocusHandle,
}

fn validate_entry(quantity: &str, unit_cost: &str) -> Result<(f64, f64), String> {
    let qty: f64 = quantity.trim().parse()
        .map_err(|_| "Quantity must be a positive number".to_string())?;
    if qty <= 0.0 { return Err("Quantity must be > 0".to_string()); }
    let cost: f64 = unit_cost.trim().parse()
        .map_err(|_| "Unit cost must be a number ≥ 0".to_string())?;
    if cost < 0.0 { return Err("Unit cost must be ≥ 0".to_string()); }
    Ok((qty, cost))
}

impl StockEntryForm {
    pub fn new(store: Entity<InventoryStore>, product_id: i64, product_name: String, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            product_id,
            product_name,
            quantity:    cx.new(|cx| TextInput::with_placeholder("e.g. 10", cx)),
            unit_cost:   cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            supplier:    cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            invoice_ref: cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            acquisition_type: AcquisitionType::Restock,
            error:       None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn validate(&self, cx: &Context<Self>) -> Result<(f64, f64), String> {
        let qty  = self.quantity.read(cx).text().to_string();
        let cost = self.unit_cost.read(cx).text().to_string();
        validate_entry(&qty, &cost)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.validate(cx) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((qty, cost)) => {
                let db       = InventoryDb::global(&**cx);
                let pid      = self.product_id;
                let sup      = self.supplier.read(cx).text().trim().to_string();
                let invref   = self.invoice_ref.read(cx).text().trim().to_string();
                let acq      = self.acquisition_type.clone();
                let store    = self.store.clone();
                let sup_opt: Option<String>    = if sup.is_empty()    { None } else { Some(sup) };
                let invref_opt: Option<String> = if invref.is_empty() { None } else { Some(invref) };

                cx.spawn(async move |this, cx| {
                    let result = db.insert_stock_entry(pid, qty, cost, sup_opt.as_deref(), acq, None, invref_opt.as_deref(), None).await;
                    if let Err(e) = result { tracing::error!("insert_stock_entry failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(StockFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for StockEntryForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for StockEntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let qty_focused  = self.quantity.read(cx).focus_handle.is_focused(window);
        let cost_focused = self.unit_cost.read(cx).focus_handle.is_focused(window);
        let sup_focused  = self.supplier.read(cx).focus_handle.is_focused(window);
        let inv_focused  = self.invoice_ref.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("StockEntryForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(StockFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.quantity.read(cx).focus_handle.clone(),
                    this.unit_cost.read(cx).focus_handle.clone(),
                    this.supplier.read(cx).focus_handle.clone(),
                    this.invoice_ref.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.quantity.read(cx).focus_handle.clone(),
                    this.unit_cost.read(cx).focus_handle.clone(),
                    this.supplier.read(cx).focus_handle.clone(),
                    this.invoice_ref.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(400.))
                    .bg(rgb(c.canvas_bg)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(c.text_default))
                        .child(format!("New Stock Entry — {}", self.product_name)))
                    .child(text_field("Quantity",         self.quantity.clone(),    qty_focused,  window))
                    .child(text_field("Unit Cost (USD)",  self.unit_cost.clone(),   cost_focused, window))
                    .child(text_field("Supplier",         self.supplier.clone(),    sup_focused,  window))
                    .child(text_field("Invoice Ref",      self.invoice_ref.clone(), inv_focused,  window))
                    .child(div().text_size(px(11.)).text_color(rgb(c.status_red))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(StockFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("btn-save").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_entry;

    #[test]
    fn validate_rejects_empty_quantity()    { assert!(validate_entry("", "10.0").is_err()); }
    #[test]
    fn validate_rejects_zero_quantity()     { assert!(validate_entry("0", "10.0").is_err()); }
    #[test]
    fn validate_rejects_negative_cost()     { assert!(validate_entry("5", "-1").is_err()); }
    #[test]
    fn validate_accepts_valid_input()       { assert_eq!(validate_entry("10.5", "120.00").unwrap(), (10.5, 120.0)); }
}
