use gpui::{
    actions, div, prelude::*, px, rems, rgb, rgba, Context, Entity, EventEmitter, FocusHandle,
    Focusable, IntoElement, Render, SharedString, Window,
};
use vassl_core::AcquisitionType;
use vassl_ui::{text_field, TextInput, ThemeHandle};

actions!(stock_form, [EscapeForm, TabField, BackTabField]);
use crate::db::InventoryDb;
use crate::store::InventoryStore;

#[derive(Debug)]
pub enum StockFormEvent {
    Submitted,
    Cancelled,
}

impl EventEmitter<StockFormEvent> for StockEntryForm {}

pub struct StockEntryForm {
    store: Entity<InventoryStore>,
    product_id: i64,
    product_name: String,
    pub quantity: Entity<TextInput>,
    unit_cost: Entity<TextInput>,
    invoice_ref: Entity<TextInput>,
    acquisition_type: AcquisitionType,
    cancel_focus: FocusHandle,
    save_focus: FocusHandle,
    error: Option<String>,
    qty_error: bool,
    cost_error: bool,
}

fn validate_entry(quantity: &str, unit_cost: &str) -> Result<(f64, f64), String> {
    let qty: f64 = quantity
        .trim()
        .parse()
        .map_err(|_| "Quantity must be a positive number".to_string())?;
    if qty <= 0.0 {
        return Err("Quantity must be > 0".to_string());
    }
    let cost: f64 = unit_cost
        .trim()
        .parse()
        .map_err(|_| "Unit cost must be a number ≥ 0".to_string())?;
    if cost < 0.0 {
        return Err("Unit cost must be ≥ 0".to_string());
    }
    Ok((qty, cost))
}

impl StockEntryForm {
    pub fn new(
        store: Entity<InventoryStore>,
        product_id: i64,
        product_name: String,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            store,
            product_id,
            product_name,
            quantity: cx.new(|cx| TextInput::with_placeholder("e.g. 10", cx)),
            unit_cost: cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            invoice_ref: cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            acquisition_type: AcquisitionType::Restock,
            cancel_focus: cx.focus_handle(),
            save_focus: cx.focus_handle(),
            error: None,
            qty_error: false,
            cost_error: false,
        }
    }

    fn validate(&self, cx: &Context<Self>) -> Result<(f64, f64), String> {
        let qty = self.quantity.read(cx).text().to_string();
        let cost = self.unit_cost.read(cx).text().to_string();
        validate_entry(&qty, &cost)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let qty_str = self.quantity.read(cx).text().to_string();
        let cost_str = self.unit_cost.read(cx).text().to_string();
        self.qty_error = qty_str.trim().parse::<f64>().map_or(true, |v| v <= 0.0);
        self.cost_error = cost_str.trim().parse::<f64>().map_or(true, |v| v < 0.0);

        match self.validate(cx) {
            Err(msg) => {
                self.error = Some(msg);
                cx.notify();
            }
            Ok((qty, cost)) => {
                let db = InventoryDb::global(&**cx);
                let pid = self.product_id;
                let invref = self.invoice_ref.read(cx).text().trim().to_string();
                let acq = self.acquisition_type.clone();
                let store = self.store.clone();
                let invref_opt: Option<String> = if invref.is_empty() {
                    None
                } else {
                    Some(invref)
                };

                cx.spawn(async move |this, cx| {
                    let result = db
                        .insert_stock_entry(
                            pid,
                            qty,
                            cost,
                            None,
                            acq,
                            None,
                            invref_opt.as_deref(),
                            None,
                        )
                        .await;
                    let _ = this.update(cx, |form, cx| match result {
                        Err(e) => {
                            tracing::error!("insert_stock_entry failed: {e:?}");
                            form.error = Some(format!("Save failed: {e}"));
                            cx.notify();
                        }
                        Ok(_) => {
                            let _ = store.update(cx, |s, cx| s.load_products(cx));
                            cx.emit(StockFormEvent::Submitted);
                        }
                    });
                    Ok::<(), anyhow::Error>(())
                })
                .detach();
            }
        }
    }
}

impl Focusable for StockEntryForm {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.quantity.read(cx).focus_handle.clone()
    }
}

impl Render for StockEntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let qty_focused = self.quantity.read(cx).focus_handle.is_focused(window);
        let cost_focused = self.unit_cost.read(cx).focus_handle.is_focused(window);
        let inv_focused = self.invoice_ref.read(cx).focus_handle.is_focused(window);
        let cancel_f = self.cancel_focus.is_focused(window);
        let save_f = self.save_focus.is_focused(window);

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000099))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .key_context("StockEntryForm")
            .on_action(cx.listener(|_, _: &EscapeForm, window, cx| {
                let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                window.focus(&root, cx);
                cx.emit(StockFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.quantity.read(cx).focus_handle.clone(),
                    this.unit_cost.read(cx).focus_handle.clone(),
                    this.invoice_ref.read(cx).focus_handle.clone(),
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
                    this.unit_cost.read(cx).focus_handle.clone(),
                    this.invoice_ref.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev =
                    handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(580.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .flex()
                    .flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.))
                            .py(px(14.))
                            .rounded_t(px(10.))
                            .bg(rgb(c.sidebar_bg))
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(rems(1.))
                                    .text_color(rgb(c.text_default))
                                    .child(format!("New Stock Entry — {}", self.product_name)),
                            )
                            .child(
                                div()
                                    .text_size(rems(0.846))
                                    .text_color(rgb(c.text_muted))
                                    .child("Esc to cancel"),
                            ),
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .px(px(20.))
                            .pt(px(8.))
                            .pb(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .py(px(10.))
                                    .child(
                                        div()
                                            .w(px(160.))
                                            .text_size(rems(0.923))
                                            .text_color(rgb(c.text_default))
                                            .child("Quantity"),
                                    )
                                    .child(div().flex_1().child(text_field(
                                        "",
                                        self.quantity.clone(),
                                        qty_focused,
                                        self.qty_error,
                                        cx,
                                    ))),
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .py(px(10.))
                                    .child(
                                        div()
                                            .w(px(160.))
                                            .text_size(rems(0.923))
                                            .text_color(rgb(c.text_default))
                                            .child("Unit Cost (USD)"),
                                    )
                                    .child(div().flex_1().child(text_field(
                                        "",
                                        self.unit_cost.clone(),
                                        cost_focused,
                                        self.cost_error,
                                        cx,
                                    ))),
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .py(px(10.))
                                    .child(
                                        div()
                                            .w(px(160.))
                                            .text_size(rems(0.923))
                                            .text_color(rgb(c.text_default))
                                            .child("Invoice Ref"),
                                    )
                                    .child(div().flex_1().child(text_field(
                                        "",
                                        self.invoice_ref.clone(),
                                        inv_focused,
                                        false,
                                        cx,
                                    ))),
                            )
                            .child(
                                div().h(px(18.)).flex().items_center().child(
                                    div()
                                        .text_size(rems(0.846))
                                        .text_color(rgb(c.status_red))
                                        .child(
                                            self.error
                                                .as_deref()
                                                .map(SharedString::from)
                                                .unwrap_or_default(),
                                        ),
                                ),
                            ),
                    )
                    // ── footer ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.))
                            .py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("btn-cancel")
                                    .track_focus(&self.cancel_focus)
                                    .px(px(18.))
                                    .py(px(7.))
                                    .rounded(px(5.))
                                    .bg(rgb(c.surface_default))
                                    .text_size(rems(0.923))
                                    .text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(cancel_f, |d| {
                                        d.border_2().border_color(rgb(c.surface_active))
                                    })
                                    .when(!cancel_f, |d| {
                                        d.border_1().border_color(rgb(c.surface_default))
                                    })
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|_, _, window, cx| {
                                            let root =
                                                cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                                            window.focus(&root, cx);
                                            cx.emit(StockFormEvent::Cancelled);
                                        }),
                                    )
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .id("btn-save")
                                    .track_focus(&self.save_focus)
                                    .px(px(18.))
                                    .py(px(7.))
                                    .rounded(px(5.))
                                    .bg(rgb(c.surface_active))
                                    .text_size(rems(0.923))
                                    .text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(save_f, |d| {
                                        d.border_2().border_color(rgb(c.text_default))
                                    })
                                    .when(!save_f, |d| {
                                        d.border_1().border_color(rgb(c.surface_active))
                                    })
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.submit(cx);
                                        }),
                                    )
                                    .child("Save Entry"),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_entry;

    #[test]
    fn validate_rejects_empty_quantity() {
        assert!(validate_entry("", "10.0").is_err());
    }
    #[test]
    fn validate_rejects_zero_quantity() {
        assert!(validate_entry("0", "10.0").is_err());
    }
    #[test]
    fn validate_rejects_negative_cost() {
        assert!(validate_entry("5", "-1").is_err());
    }
    #[test]
    fn validate_accepts_valid_input() {
        assert_eq!(validate_entry("10.5", "120.00").unwrap(), (10.5, 120.0));
    }
}
