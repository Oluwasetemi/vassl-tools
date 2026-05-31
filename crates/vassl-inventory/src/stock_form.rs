use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::AcquisitionType;

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

// ── Events ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum StockFormEvent {
    Submitted,
    Cancelled,
}

impl EventEmitter<StockFormEvent> for StockEntryForm {}

// ── Form struct ───────────────────────────────────────────────────────────────

pub struct StockEntryForm {
    store:            Entity<InventoryStore>,
    product_id:       i64,
    product_name:     String,
    quantity:         String,
    unit_cost:        String,
    supplier:         String,
    invoice_ref:      String,
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
            quantity: String::new(),
            unit_cost: String::new(),
            supplier: String::new(),
            invoice_ref: String::new(),
            // TODO(Plan 5): default Restock until TextInput + AcquisitionType picker added
            acquisition_type: AcquisitionType::Restock,
            error: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn validate(&self) -> Result<(f64, f64), String> {
        validate_entry(&self.quantity, &self.unit_cost)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.validate() {
            Err(msg) => {
                self.error = Some(msg);
                cx.notify();
            }
            Ok((qty, cost)) => {
                let db        = InventoryDb::global(&**cx);
                let pid       = self.product_id;
                let sup       = self.supplier.trim().to_string();
                let invref    = self.invoice_ref.trim().to_string();
                let acq       = self.acquisition_type.clone();
                let store     = self.store.clone();

                let sup_opt: Option<String> = if sup.is_empty() { None } else { Some(sup) };
                let invref_opt: Option<String> = if invref.is_empty() { None } else { Some(invref) };

                cx.spawn(async move |this, cx| {
                    let result = db.insert_stock_entry(
                        pid, qty, cost,
                        sup_opt.as_deref(),
                        acq,
                        None,
                        invref_opt.as_deref(),
                        None,
                    ).await;

                    if let Err(e) = result {
                        tracing::error!("insert_stock_entry failed: {e:?}");
                        return Ok(());
                    }

                    // Refresh products, then emit Submitted to close the form
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(StockFormEvent::Submitted))
                })
                .detach();
            }
        }
    }
}

impl Focusable for StockEntryForm {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for StockEntryForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Full-screen dark scrim + centered card
        div()
            .absolute()
            .top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(400.))
                    .bg(rgb(colors::CANVAS_BG))
                    .rounded(px(8.))
                    .p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    // Title
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .child(format!("New Stock Entry — {}", self.product_name))
                    )
                    // Quantity
                    .child(form_field("Quantity", &self.quantity, "e.g. 10"))
                    // Unit Cost
                    .child(form_field("Unit Cost (USD)", &self.unit_cost, "e.g. 120.00"))
                    // Supplier
                    .child(form_field("Supplier", &self.supplier, "optional"))
                    // Invoice Ref
                    .child(form_field("Invoice Ref", &self.invoice_ref, "optional"))
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
                                    .id("btn-cancel")
                                    .px(px(16.)).py(px(6.))
                                    .rounded(px(4.))
                                    .bg(rgb(colors::SURFACE_DEFAULT))
                                    .text_size(px(12.))
                                    .text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_this, _, _, cx| {
                                        cx.emit(StockFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div()
                                    .id("btn-save")
                                    .px(px(16.)).py(px(6.))
                                    .rounded(px(4.))
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

fn form_field(label: &str, value: &str, placeholder: &str) -> impl IntoElement {
    let display = if value.is_empty() { placeholder } else { value };
    let text_color = if value.is_empty() { colors::TEXT_MUTED } else { colors::TEXT_DEFAULT };

    div()
        .flex().flex_col().gap(px(4.))
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(colors::TEXT_MUTED))
                .child(label.to_string())
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

// ── Tests ─────────────────────────────────────────────────────────────────────

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
