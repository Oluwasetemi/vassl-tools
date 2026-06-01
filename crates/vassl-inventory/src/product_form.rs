use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

actions!(product_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum ProductFormEvent { Submitted, Cancelled }

impl EventEmitter<ProductFormEvent> for ProductForm {}

pub struct ProductForm {
    store:        Entity<InventoryStore>,
    pub sku:      Entity<TextInput>,
    name:         Entity<TextInput>,
    category:     Entity<TextInput>,
    unit:         Entity<TextInput>,
    min_stock:    Entity<TextInput>,
    description:  Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

fn validate_product(sku: &str, name: &str, unit: &str, min_stock: &str) -> Result<(String, String, String, f64), String> {
    let sku = sku.trim().to_string();
    if sku.is_empty()  { return Err("SKU is required.".to_string()); }
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Name is required.".to_string()); }
    let unit = unit.trim().to_string();
    if unit.is_empty() { return Err("Unit is required (e.g. 'pcs', 'meters').".to_string()); }
    let min: f64 = min_stock.trim().parse().unwrap_or(0.0);
    if min < 0.0  { return Err("Min stock must be ≥ 0.".to_string()); }
    Ok((sku, name, unit, min))
}

impl ProductForm {
    pub fn new(store: Entity<InventoryStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            sku:          cx.new(|cx| TextInput::with_placeholder("e.g. CAM-IP-2MP", cx)),
            name:         cx.new(|cx| TextInput::with_placeholder("e.g. IP Camera 2MP", cx)),
            category:     cx.new(|cx| TextInput::with_placeholder("optional: Cameras, Cabling…", cx)),
            unit:         cx.new(|cx| TextInput::with_placeholder("pcs, meters, rolls…", cx)),
            min_stock:    cx.new(|cx| TextInput::with_placeholder("0", cx)),
            description:  cx.new(|cx| TextInput::with_placeholder(
                "e.g. Wide-angle camera lens, 24mm, F/1.8, compatible with Sony E-mount", cx
            )),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let sku       = self.sku.read(cx).text().to_string();
        let name      = self.name.read(cx).text().to_string();
        let unit      = self.unit.read(cx).text().to_string();
        let min_s     = self.min_stock.read(cx).text().to_string();
        let category  = self.category.read(cx).text().trim().to_string();
        let cat_opt   = if category.is_empty() { None } else { Some(category) };
        let desc      = self.description.read(cx).text().trim().to_string();
        let desc_opt  = if desc.is_empty() { None } else { Some(desc) };

        match validate_product(&sku, &name, &unit, &min_s) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((sku, name, unit, min)) => {
                let db    = InventoryDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_product(
                        &sku, &name, cat_opt.as_deref(), &unit, min,
                        desc_opt.as_deref(), None,
                    ).await;
                    if let Err(e) = result { tracing::error!("insert_product failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(ProductFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for ProductForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for ProductForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c      = cx.global::<ThemeHandle>().0.clone();
        let sku_f  = self.sku.read(cx).focus_handle.is_focused(window);
        let name_f = self.name.read(cx).focus_handle.is_focused(window);
        let cat_f  = self.category.read(cx).focus_handle.is_focused(window);
        let unit_f = self.unit.read(cx).focus_handle.is_focused(window);
        let min_f  = self.min_stock.read(cx).focus_handle.is_focused(window);
        let desc_f = self.description.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("ProductForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(ProductFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.sku.read(cx).focus_handle.clone(),
                    this.name.read(cx).focus_handle.clone(),
                    this.category.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.min_stock.read(cx).focus_handle.clone(),
                    this.description.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.sku.read(cx).focus_handle.clone(),
                    this.name.read(cx).focus_handle.clone(),
                    this.category.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.min_stock.read(cx).focus_handle.clone(),
                    this.description.read(cx).focus_handle.clone(),
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
                                .child("New Product"))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("SKU"))
                                    .child(div().flex_1().child(text_field("", self.sku.clone(), sku_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Name"))
                                    .child(div().flex_1().child(text_field("", self.name.clone(), name_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Category"))
                                    .child(div().flex_1().child(text_field("", self.category.clone(), cat_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Unit"))
                                    .child(div().flex_1().child(text_field("", self.unit.clone(), unit_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Min Stock Level"))
                                    .child(div().flex_1().child(text_field("", self.min_stock.clone(), min_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_start().py(px(10.))
                                    .child(div().w(px(160.)).pt(px(6.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Description"))
                                    .child(div().flex_1().h(px(64.)).child(text_field("", self.description.clone(), desc_f, cx)))
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
                            .child(div().id("prod-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(ProductFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("prod-btn-save").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save Product"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_product;
    #[test] fn rejects_empty_sku()  { assert!(validate_product("", "Camera", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_name() { assert!(validate_product("CAM-001", "", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_unit() { assert!(validate_product("CAM-001", "Camera", "", "5").is_err()); }
    #[test] fn accepts_zero_min()   { assert!(validate_product("CAM-001", "Camera", "pcs", "0").is_ok()); }
    #[test] fn accepts_valid()      { assert!(validate_product("CAM-001", "IP Camera", "pcs", "5.0").is_ok()); }
}
