use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
           Subscription, Window, actions, div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_core::{AcquisitionType, Product};
use vassl_suppliers::store::SupplierStoreHandle;
use vassl_ui::{Dropdown, DropdownItem, TextInput, ThemeHandle, text_field};

use crate::db::InventoryDb;
use crate::store::InventoryStore;

actions!(product_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum ProductFormEvent { Submitted, Cancelled }

impl EventEmitter<ProductFormEvent> for ProductForm {}

enum FormMode {
    Create,
    Edit { product_id: i64 },
}

pub struct ProductForm {
    mode:               FormMode,
    store:              Entity<InventoryStore>,
    sku_display:        String,      // read-only in Edit mode
    original_stock:     Option<f64>, // snapshot at open time; used to compute delta on save
    pub sku:            Entity<TextInput>,
    name:               Entity<TextInput>,
    category:           Entity<TextInput>,
    unit:               Entity<TextInput>,
    model_number:       Entity<TextInput>,
    part_number:        Entity<TextInput>,
    duty_percent:       Entity<TextInput>,
    initial_qty:        Entity<TextInput>,  // create mode only
    edit_stock:         Entity<TextInput>,  // edit mode only; pre-filled with current stock
    min_stock:          Entity<TextInput>,
    description:        Entity<TextInput>,
    supplier_dropdown:  Entity<Dropdown>,
    _supplier_sub:      Subscription,
    cancel_focus:       FocusHandle,
    save_focus:         FocusHandle,
    error:              Option<String>,
    sku_error:          bool,
    name_error:         bool,
    unit_error:         bool,
    focus_handle:       FocusHandle,
}

fn validate_product(sku: &str, name: &str, unit: &str, min_stock: &str) -> Result<(String, String, String, f64), String> {
    let sku = sku.trim().to_string();
    if sku.is_empty()  { return Err("SKU is required.".to_string()); }
    validate_product_fields(name, unit, min_stock).map(|(n, u, m)| (sku, n, u, m))
}

fn validate_product_fields(name: &str, unit: &str, min_stock: &str) -> Result<(String, String, f64), String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Name is required.".to_string()); }
    let unit = unit.trim().to_string();
    if unit.is_empty() { return Err("Unit is required (e.g. 'pcs', 'meters').".to_string()); }
    let min: f64 = min_stock.trim().parse().unwrap_or(0.0);
    if min < 0.0  { return Err("Min stock must be ≥ 0.".to_string()); }
    Ok((name, unit, min))
}

impl ProductForm {
    fn make_supplier_dropdown(preselect: Option<i64>, cx: &mut Context<Self>) -> (Entity<Dropdown>, Subscription) {
        let sup_store = cx.global::<SupplierStoreHandle>().0.clone();
        let suppliers = sup_store.read(cx).suppliers.clone();
        let dropdown  = cx.new(|cx| {
            let mut d = Dropdown::new("No preferred supplier", "No suppliers yet — add one in Suppliers.", cx);
            d.items = Self::suppliers_to_items(&suppliers);
            d.loading = false;
            d.selected_id = preselect;
            d
        });
        let sub = cx.observe(&sup_store, {
            let dropdown = dropdown.clone();
            move |_this, store_ent, cx| {
                let suppliers = store_ent.read(cx).suppliers.clone();
                dropdown.update(cx, |d, cx| {
                    d.items   = ProductForm::suppliers_to_items(&suppliers);
                    d.loading = false;
                    cx.notify();
                });
            }
        });
        (dropdown, sub)
    }

    fn suppliers_to_items(suppliers: &[vassl_core::Supplier]) -> Vec<DropdownItem> {
        let mut items = vec![DropdownItem { id: -1, label: "(None)".into(), sublabel: None }];
        items.extend(suppliers.iter().map(|s| DropdownItem {
            id:       s.id,
            label:    s.name.clone(),
            sublabel: s.contact_person.clone(),
        }));
        items
    }


    pub fn new(store: Entity<InventoryStore>, cx: &mut Context<Self>) -> Self {
        let (supplier_dropdown, _supplier_sub) = Self::make_supplier_dropdown(None, cx);
        let description = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("", cx);
            f.suppress_placeholder = true;
            f
        });
        let db            = vassl_db::AppDatabase::global(&**cx);
        let default_unit  = vassl_db::shared::get_setting(db, "inventory.default_unit").ok().flatten().unwrap_or_default();
        let default_min   = vassl_db::shared::get_setting(db, "inventory.low_stock_threshold").ok().flatten().unwrap_or_default();
        Self {
            mode:           FormMode::Create,
            store,
            sku_display:    String::new(),
            original_stock: None,
            sku:          cx.new(|cx| TextInput::with_placeholder("e.g. CAM-IP-2MP", cx)),
            name:         cx.new(|cx| TextInput::with_placeholder("e.g. IP Camera 2MP", cx)),
            category:     cx.new(|cx| TextInput::with_placeholder("optional: Cameras, Cabling…", cx)),
            unit:         cx.new(move |cx| {
                let mut f = TextInput::with_placeholder("pcs, meters, rolls…", cx);
                if !default_unit.is_empty() { f.set_text(&default_unit, cx); }
                f
            }),
            model_number: cx.new(|cx| TextInput::with_placeholder("e.g. DS-2CD2143G2-I", cx)),
            part_number:  cx.new(|cx| TextInput::with_placeholder("e.g. PN-001", cx)),
            duty_percent: cx.new(|cx| TextInput::with_placeholder("e.g. 42.5", cx)),
            initial_qty:  cx.new(|cx| TextInput::with_placeholder("e.g. 10  (optional)", cx)),
            edit_stock:   cx.new(|cx| TextInput::with_placeholder("", cx)),
            min_stock:    cx.new(move |cx| {
                let mut f = TextInput::with_placeholder("0", cx);
                if !default_min.is_empty() { f.set_text(&default_min, cx); }
                f
            }),
            description,
            supplier_dropdown,
            _supplier_sub,
            cancel_focus: cx.focus_handle(),
            save_focus:   cx.focus_handle(),
            error:        None,
            sku_error:    false,
            name_error:   false,
            unit_error:   false,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn new_edit(store: Entity<InventoryStore>, product: &Product, current_stock: f64, cx: &mut Context<Self>) -> Self {

        let name_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. IP Camera 2MP", cx);
            f.set_text(&product.name, cx);
            f
        });
        let cat_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("optional", cx);
            if let Some(c) = &product.category { f.set_text(c, cx); }
            f
        });
        let unit_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("pcs, meters, rolls…", cx);
            f.set_text(&product.unit, cx);
            f
        });
        let min_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("0", cx);
            f.set_text(&format!("{}", product.min_stock_level), cx);
            f
        });
        let desc_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("optional", cx);
            f.suppress_placeholder = true;
            if let Some(d) = &product.description { f.set_text(d, cx); }
            f
        });
        let stock_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("0", cx);
            f.set_text(&format!("{current_stock:.2}"), cx);
            f
        });
        let model_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. DS-2CD2143G2-I", cx);
            if let Some(m) = &product.model_number { f.set_text(m, cx); }
            f
        });
        let part_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. PN-001", cx);
            if let Some(p) = &product.part_number { f.set_text(p, cx); }
            f
        });
        let duty_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 42.5", cx);
            if product.duty_percent > 0.0 {
                f.set_text(&format!("{:.1}", product.duty_percent), cx);
            }
            f
        });
        let preselect = product.preferred_supplier_id;
        let (supplier_dropdown, _supplier_sub) = Self::make_supplier_dropdown(preselect, cx);
        Self {
            mode:           FormMode::Edit { product_id: product.id },
            store,
            sku_display:    product.sku.clone(),
            original_stock: Some(current_stock),
            sku:          cx.new(|cx| TextInput::with_placeholder("", cx)), // unused in edit
            name:         name_field,
            category:     cat_field,
            unit:         unit_field,
            model_number: model_field,
            part_number:  part_field,
            duty_percent: duty_field,
            initial_qty:  cx.new(|cx| TextInput::with_placeholder("", cx)), // unused in edit
            edit_stock:   stock_field,
            min_stock:    min_field,
            description:  desc_field,
            supplier_dropdown,
            _supplier_sub,
            cancel_focus: cx.focus_handle(),
            save_focus:   cx.focus_handle(),
            error:        None,
            sku_error:    false,
            name_error:   false,
            unit_error:   false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name      = self.name.read(cx).text().to_string();
        let unit      = self.unit.read(cx).text().to_string();
        let min_s     = self.min_stock.read(cx).text().to_string();
        let category  = self.category.read(cx).text().trim().to_string();
        let cat_opt   = if category.is_empty() { None } else { Some(category) };
        let desc      = self.description.read(cx).text().trim().to_string();
        let desc_opt  = if desc.is_empty() { None } else { Some(desc) };
        let model     = self.model_number.read(cx).text().trim().to_string();
        let model_opt = if model.is_empty() { None } else { Some(model) };
        let part      = self.part_number.read(cx).text().trim().to_string();
        let part_opt  = if part.is_empty() { None } else { Some(part) };
        let duty: f64 = self.duty_percent.read(cx).text().trim().parse::<f64>().unwrap_or(0.0).max(0.0);
        // -1 is the sentinel "(None)" item; treat as no supplier
        let sup_id    = self.supplier_dropdown.read(cx).selected_id.filter(|&id| id > 0);

        self.name_error = name.trim().is_empty();
        self.unit_error = unit.trim().is_empty();

        match &self.mode {
            FormMode::Create => {
                let sku = self.sku.read(cx).text().to_string();
                self.sku_error = sku.trim().is_empty();
                let qty_s   = self.initial_qty.read(cx).text().trim().to_string();
                let init_qty: Option<f64> = if qty_s.is_empty() {
                    None
                } else {
                    match qty_s.parse::<f64>() {
                        Ok(q) if q > 0.0 => Some(q),
                        Ok(_) => { self.error = Some("Initial quantity must be > 0.".to_string()); cx.notify(); return; }
                        Err(_) => { self.error = Some("Initial quantity must be a number.".to_string()); cx.notify(); return; }
                    }
                };
                match validate_product(&sku, &name, &unit, &min_s) {
                    Err(msg) => { self.error = Some(msg); cx.notify(); }
                    Ok((sku, name, unit, min)) => {
                        let db    = InventoryDb::global(&**cx);
                        let store = self.store.clone();
                        cx.spawn(async move |this, cx| {
                            let insert_result = db.insert_product(
                                &sku, &name, cat_opt.as_deref(), &unit, min,
                                desc_opt.as_deref(), None, sup_id,
                                model_opt.as_deref(), part_opt.as_deref(), duty,
                            ).await;
                            match insert_result {
                                Err(e) => {
                                    tracing::error!("insert_product failed: {e:?}");
                                    let msg = if e.to_string().contains("UNIQUE") {
                                        "A product with this SKU already exists.".to_string()
                                    } else {
                                        format!("Save failed: {e}")
                                    };
                                    let _ = this.update(cx, |form, cx| { form.error = Some(msg); cx.notify(); });
                                    return Ok(());
                                }
                                Ok(new_id) => {
                                    if let Some(qty) = init_qty {
                                        if let Err(e) = db.insert_stock_entry(
                                            new_id, qty, 0.0, None,
                                            AcquisitionType::Restock, None, None, None,
                                        ).await {
                                            tracing::error!("insert initial stock entry failed: {e:?}");
                                        }
                                    }
                                }
                            }
                            let _ = store.update(cx, |s, cx| s.load_products(cx));
                            this.update(cx, |_, cx| cx.emit(ProductFormEvent::Submitted))
                        }).detach();
                    }
                }
            }
            FormMode::Edit { product_id } => {
                let pid = *product_id;
                let stock_s = self.edit_stock.read(cx).text().trim().to_string();
                let new_stock: f64 = match stock_s.parse() {
                    Ok(v) if v >= 0.0 => v,
                    Ok(_) => { self.error = Some("Stock must be ≥ 0.".to_string()); cx.notify(); return; }
                    Err(_) => { self.error = Some("Stock must be a number.".to_string()); cx.notify(); return; }
                };
                let delta = new_stock - self.original_stock.unwrap_or(0.0);
                match validate_product_fields(&name, &unit, &min_s) {
                    Err(msg) => { self.error = Some(msg); cx.notify(); }
                    Ok((name, unit, min)) => {
                        let db    = InventoryDb::global(&**cx);
                        let store = self.store.clone();
                        cx.spawn(async move |this, cx| {
                            let update_result = db.update_product(
                                pid, &name, cat_opt.as_deref(), &unit, min,
                                desc_opt.as_deref(), sup_id,
                                model_opt.as_deref(), part_opt.as_deref(), duty,
                            ).await;
                            if let Err(e) = update_result {
                                tracing::error!("update_product failed: {e:?}");
                                let _ = this.update(cx, |form, cx| {
                                    form.error = Some(format!("Save failed: {e}"));
                                    cx.notify();
                                });
                                return Ok(());
                            }
                            if delta.abs() > f64::EPSILON {
                                if let Err(e) = db.insert_stock_entry(
                                    pid, delta, 0.0, None,
                                    AcquisitionType::Adjustment, None, Some("manual adjustment"), None,
                                ).await {
                                    tracing::error!("stock adjustment failed: {e:?}");
                                }
                            }
                            let _ = store.update(cx, |s, cx| s.load_products(cx));
                            this.update(cx, |_, cx| cx.emit(ProductFormEvent::Submitted))
                        }).detach();
                    }
                }
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
        let is_edit = matches!(self.mode, FormMode::Edit { .. });

        let name_f      = self.name.read(cx).focus_handle.is_focused(window);
        let cat_f       = self.category.read(cx).focus_handle.is_focused(window);
        let unit_f      = self.unit.read(cx).focus_handle.is_focused(window);
        let model_f     = self.model_number.read(cx).focus_handle.is_focused(window);
        let part_f      = self.part_number.read(cx).focus_handle.is_focused(window);
        let duty_f      = self.duty_percent.read(cx).focus_handle.is_focused(window);
        let qty_f       = self.initial_qty.read(cx).focus_handle.is_focused(window);
        let stock_f     = self.edit_stock.read(cx).focus_handle.is_focused(window);
        let min_f       = self.min_stock.read(cx).focus_handle.is_focused(window);
        let desc_f      = self.description.read(cx).focus_handle.is_focused(window);
        let cancel_f    = self.cancel_focus.is_focused(window);
        let save_f      = self.save_focus.is_focused(window);
        let has_desc    = !self.description.read(cx).content.is_empty();

        let (title, save_label) = if is_edit {
            (format!("Edit Product — {}", self.sku_display), "Update Product")
        } else {
            ("New Product".to_string(), "Save Product")
        };

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("ProductForm")
            .on_action(cx.listener(|this, _: &EscapeForm, window, cx| {
                // Close an open dropdown first; only cancel the form on the second Esc
                if this.supplier_dropdown.read(cx).is_open {
                    this.supplier_dropdown.update(cx, |d, cx| { d.is_open = false; cx.notify(); });
                } else {
                    let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                    window.focus(&root, cx);
                    cx.emit(ProductFormEvent::Cancelled);
                }
            }))
            .on_action(cx.listener(move |this, _: &TabField, window, cx| {
                let mut handles = vec![
                    this.name.read(cx).focus_handle.clone(),
                    this.category.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.model_number.read(cx).focus_handle.clone(),
                    this.part_number.read(cx).focus_handle.clone(),
                    this.duty_percent.read(cx).focus_handle.clone(),
                    this.min_stock.read(cx).focus_handle.clone(),
                    this.description.read(cx).focus_handle.clone(),
                    this.supplier_dropdown.read(cx).trigger_focus.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                if !is_edit {
                    handles.insert(0, this.sku.read(cx).focus_handle.clone());
                    // initial_qty sits after duty_percent (index 6 after sku insert → insert at 7)
                    handles.insert(7, this.initial_qty.read(cx).focus_handle.clone());
                } else {
                    // edit_stock is the first focusable field (SKU is read-only in edit)
                    handles.insert(0, this.edit_stock.read(cx).focus_handle.clone());
                }
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(move |this, _: &BackTabField, window, cx| {
                let mut handles = vec![
                    this.name.read(cx).focus_handle.clone(),
                    this.category.read(cx).focus_handle.clone(),
                    this.unit.read(cx).focus_handle.clone(),
                    this.model_number.read(cx).focus_handle.clone(),
                    this.part_number.read(cx).focus_handle.clone(),
                    this.duty_percent.read(cx).focus_handle.clone(),
                    this.min_stock.read(cx).focus_handle.clone(),
                    this.description.read(cx).focus_handle.clone(),
                    this.supplier_dropdown.read(cx).trigger_focus.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                if !is_edit {
                    handles.insert(0, this.sku.read(cx).focus_handle.clone());
                    handles.insert(7, this.initial_qty.read(cx).focus_handle.clone());
                } else {
                    handles.insert(0, this.edit_stock.read(cx).focus_handle.clone());
                }
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(580.))
                    .max_h(px(680.))
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
                            .flex_shrink_0()
                            .child(div().flex_1()
                                .text_size(rems(1.)).text_color(rgb(c.text_default))
                                .child(title))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields (scrollable) ──────────────────────────────
                    .child(
                        div()
                            .id("prod-form-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            // SKU — editable only in Create mode
                            .child(if is_edit {
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("SKU"))
                                    .child(div().flex_1().px(px(8.)).py(px(4.))
                                        .bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                        .child(self.sku_display.clone()))
                            } else {
                                let sku_f = self.sku.read(cx).focus_handle.is_focused(window);
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("SKU"))
                                    .child(div().flex_1().child(text_field("", self.sku.clone(), sku_f, self.sku_error, cx)))
                            })
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // Current Stock — editable in Edit mode
                            .when(is_edit, |d| d
                                .child(
                                    div().flex().flex_row().items_center().py(px(10.))
                                        .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Current Stock"))
                                        .child(div().flex_1().child(text_field("", self.edit_stock.clone(), stock_f, false, cx)))
                                )
                                .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            )
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Name"))
                                    .child(div().flex_1().child(text_field("", self.name.clone(), name_f, self.name_error, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Category"))
                                    .child(div().flex_1().child(text_field("", self.category.clone(), cat_f, false, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Unit"))
                                    .child(div().flex_1().child(text_field("", self.unit.clone(), unit_f, self.unit_error, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Model #"))
                                    .child(div().flex_1().child(text_field("", self.model_number.clone(), model_f, false, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Part #"))
                                    .child(div().flex_1().child(text_field("", self.part_number.clone(), part_f, false, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Duty %"))
                                    .child(div().flex_1().child(text_field("", self.duty_percent.clone(), duty_f, false, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .when(!is_edit, |d| d
                                .child(
                                    div().flex().flex_row().items_center().py(px(10.))
                                        .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Initial Stock"))
                                        .child(div().flex_1().child(text_field("", self.initial_qty.clone(), qty_f, false, cx)))
                                )
                                .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            )
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Min Stock Level"))
                                    .child(div().flex_1().child(text_field("", self.min_stock.clone(), min_f, false, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_start().py(px(10.))
                                    .child(div().w(px(160.)).pt(px(6.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Description"))
                                    .child(
                                        div().flex_1().h(px(80.)).relative()
                                            .border_1().border_color(rgb(if desc_f { c.surface_active } else { c.surface_default }))
                                            .when(desc_f, |d| d.border_2())
                                            .rounded(px(4.))
                                            .bg(rgb(if desc_f { c.canvas_bg } else { c.surface_default }))
                                            .overflow_hidden()
                                            // Wrapping placeholder — shows behind the TextInput when empty
                                            .when(!has_desc, |d| d.child(
                                                div()
                                                    .absolute().top(px(4.)).left(px(8.)).right(px(8.))
                                                    .text_size(rems(0.923))
                                                    .text_color(rgba(((c.text_muted as u64) << 8 | 0x99) as u32))
                                                    .child("e.g. Wide-angle camera lens, 24mm, F/1.8, compatible with Sony E-mount")
                                            ))
                                            .child(
                                                div().px(px(8.)).py(px(4.))
                                                    .text_size(rems(1.)).text_color(rgb(c.text_default))
                                                    .child(self.description.clone())
                                            )
                                    )
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // ── supplier dropdown ──────────────────────────
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child("Preferred Supplier"))
                                    .child(div().flex_1().child(self.supplier_dropdown.clone()))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                        ) // close inner content div
                    )   // close scroll wrapper
                    // ── footer ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(
                                div().id("prod-btn-cancel")
                                    .track_focus(&self.cancel_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_default))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(cancel_f, |d| d.border_2().border_color(rgb(c.surface_active)))
                                    .when(!cancel_f, |d| d.border_1().border_color(rgb(c.surface_default)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, window, cx| {
                                        let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                                        window.focus(&root, cx);
                                        cx.emit(ProductFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div().id("prod-btn-save")
                                    .track_focus(&self.save_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(save_f, |d| d.border_2().border_color(rgb(c.text_default)))
                                    .when(!save_f, |d| d.border_1().border_color(rgb(c.surface_active)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.submit(cx);
                                    }))
                                    .child(save_label)
                            )
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
