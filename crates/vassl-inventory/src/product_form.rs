use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_ui::{TextInput, text_field};

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

#[derive(Debug)]
pub enum ProductFormEvent { Submitted, Cancelled }

impl EventEmitter<ProductFormEvent> for ProductForm {}

pub struct ProductForm {
    store:        Entity<InventoryStore>,
    sku:          Entity<TextInput>,
    name:         Entity<TextInput>,
    category:     Entity<TextInput>,
    unit:         Entity<TextInput>,
    min_stock:    Entity<TextInput>,
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

        match validate_product(&sku, &name, &unit, &min_s) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((sku, name, unit, min)) => {
                let db    = InventoryDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_product(&sku, &name, cat_opt.as_deref(), &unit, min, None).await;
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
        let sku_f  = self.sku.read(cx).focus_handle.is_focused(window);
        let name_f = self.name.read(cx).focus_handle.is_focused(window);
        let cat_f  = self.category.read(cx).focus_handle.is_focused(window);
        let unit_f = self.unit.read(cx).focus_handle.is_focused(window);
        let min_f  = self.min_stock.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(400.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT)).child("New Product"))
                    .child(text_field("SKU",                   self.sku.clone(),       sku_f,  window))
                    .child(text_field("Name",                  self.name.clone(),      name_f, window))
                    .child(text_field("Category (optional)",   self.category.clone(),  cat_f,  window))
                    .child(text_field("Unit",                  self.unit.clone(),      unit_f, window))
                    .child(text_field("Min Stock Level",       self.min_stock.clone(), min_f,  window))
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("prod-btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(ProductFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("prod-btn-save").px(px(16.)).py(px(6.)).rounded(px(4.))
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
    use super::validate_product;
    #[test] fn rejects_empty_sku()  { assert!(validate_product("", "Camera", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_name() { assert!(validate_product("CAM-001", "", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_unit() { assert!(validate_product("CAM-001", "Camera", "", "5").is_err()); }
    #[test] fn accepts_zero_min()   { assert!(validate_product("CAM-001", "Camera", "pcs", "0").is_ok()); }
    #[test] fn accepts_valid()      { assert!(validate_product("CAM-001", "IP Camera", "pcs", "5.0").is_ok()); }
}
