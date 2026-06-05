use gpui::{Context, Entity, EventEmitter, IntoElement, MouseButton, MouseDownEvent,
           Render, Subscription, Window, div, prelude::*, px, rems, rgb};
use vassl_ui::NewRecord;
use vassl_ui::{TextInput, ThemeHandle, text_field};

use vassl_core::Product;
use crate::product_form::{ProductForm, ProductFormEvent};
use crate::product_list::ProductList;
use crate::restock::RestockAlerts;
use crate::stock_form::{StockEntryForm, StockFormEvent};
use crate::store::InventoryStore;
use crate::InventoryStoreHandle;

#[derive(Clone, PartialEq)]
pub enum InventoryPanelEvent {
    ShowPriceHistory   { product_id: i64, name: String },
    ShowPriceEntryForm { product_id: i64, name: String },
    ImportXlsxRequested,
}

impl EventEmitter<InventoryPanelEvent> for InventoryPanel {}

#[derive(Clone, Copy, PartialEq)]
enum Tab { Products, Restock }

pub struct InventoryPanel {
    store:          Entity<InventoryStore>,
    product_list:   Entity<ProductList>,
    restock_alerts: Entity<RestockAlerts>,
    active_tab:     Tab,
    stock_form:     Option<Entity<StockEntryForm>>,
    _form_sub:      Option<Subscription>,
    product_form:   Option<Entity<ProductForm>>,
    _prod_form_sub: Option<Subscription>,
    search_input:   Entity<TextInput>,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<InventoryStoreHandle>().0.clone();
        let product_list   = cx.new(|cx| ProductList::new(store.clone(), cx));
        let restock_alerts = cx.new(|cx| RestockAlerts::new(store.clone(), cx));

        store.update(cx, |s, cx| s.load_products(cx));
        let search_input = cx.new(|cx| TextInput::with_placeholder("Filter…", cx));

        cx.observe(&search_input, |this, input, cx| {
            let q = input.read(cx).text().to_string();
            this.store.update(cx, |s, cx| s.set_search_query(q, cx));
        }).detach();

        Self {
            store,
            product_list,
            restock_alerts,
            active_tab:     Tab::Products,
            stock_form:     None,
            _form_sub:      None,
            product_form:   None,
            _prod_form_sub: None,
            search_input,
        }
    }

    fn open_stock_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stock_form.is_some() { return; }
        let (product_id, product_name) = {
            let store = self.store.read(cx);
            let Some(pid) = store.selected_product_id else { return; };
            let name = store.products
                .iter()
                .find(|p| p.product.id == pid)
                .map(|p| p.product.name.clone())
                .unwrap_or_default();
            (pid, name)
        };

        let form  = cx.new(|cx| StockEntryForm::new(self.store.clone(), product_id, product_name, cx));
        let first = form.read(cx).quantity.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        let sub = cx.subscribe(&form, |this, _form, ev: &StockFormEvent, cx| {
            match ev {
                StockFormEvent::Submitted | StockFormEvent::Cancelled => {
                    this._form_sub  = None;
                    this.stock_form = None;
                    cx.notify();
                }
            }
        });
        self.stock_form = Some(form);
        self._form_sub  = Some(sub);
        cx.notify();
    }

    pub fn create_product_form(&mut self, cx: &mut Context<Self>) -> Option<gpui::FocusHandle> {
        if self.product_form.is_some() { return None; }
        let form  = cx.new(|cx| ProductForm::new(self.store.clone(), cx));
        let first = form.read(cx).sku.read(cx).focus_handle.clone();
        let sub  = cx.subscribe(&form, |this, _form, ev: &ProductFormEvent, cx| {
            match ev {
                ProductFormEvent::Submitted | ProductFormEvent::Cancelled => {
                    this._prod_form_sub = None;
                    this.product_form   = None;
                    cx.notify();
                }
            }
        });
        self.product_form   = Some(form);
        self._prod_form_sub = Some(sub);
        cx.notify();
        Some(first)
    }

    pub fn open_product_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fh) = self.create_product_form(cx) {
            window.focus(&fh, cx);
        }
    }

    fn open_edit_form(&mut self, product: Product, window: &mut Window, cx: &mut Context<Self>) {
        if self.product_form.is_some() { return; }
        let current_stock = self.store.read(cx).products.iter()
            .find(|p| p.product.id == product.id)
            .map(|p| p.current_stock)
            .unwrap_or(0.0);
        let form  = cx.new(|cx| ProductForm::new_edit(self.store.clone(), &product, current_stock, cx));
        let first = form.read(cx).sku.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        let sub  = cx.subscribe(&form, |this, _form, ev: &ProductFormEvent, cx| {
            match ev {
                ProductFormEvent::Submitted | ProductFormEvent::Cancelled => {
                    this._prod_form_sub = None;
                    this.product_form   = None;
                    cx.notify();
                }
            }
        });
        self.product_form   = Some(form);
        self._prod_form_sub = Some(sub);
        cx.notify();
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();
        let viewport      = window.viewport_size();

        let has_query = !self.search_input.read(cx).text().is_empty();

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Products => content.child(self.product_list.clone()),
            Tab::Restock  => content.child(self.restock_alerts.clone()),
        };

        let mut root = div()
            .key_context("InventoryPanel")
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| {
                this.open_product_form(window, cx);
            }))
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child({
                        let is_tab = active_tab == Tab::Products;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("tab-products")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if is_tab { c.surface_active } else { c.surface_default }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Products;
                                cx.notify();
                            }))
                            .child("Products")
                    })
                    .child({
                        let is_tab = active_tab == Tab::Restock;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("tab-restock")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if is_tab { c.surface_active } else { c.surface_default }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Restock;
                                cx.notify();
                            }))
                            .child("Restock Alerts")
                    })
                    .child(
                        div()
                            .flex().flex_row().items_center().gap(px(4.))
                            .child(
                                div()
                                    .w(px(160.))
                                    .child({
                                        let focused = self.search_input.read(cx).focus_handle.is_focused(window);
                                        text_field("", self.search_input.clone(), focused, cx)
                                    })
                            )
                            .child({
                                let mut clear = div()
                                    .id("inv-search-clear")
                                    .px(px(6.)).py(px(2.)).rounded(px(3.))
                                    .text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                    .child("×");
                                if has_query {
                                    let si = self.search_input.clone();
                                    clear = clear
                                        .cursor_pointer()
                                        .on_mouse_down(gpui::MouseButton::Left, move |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut gpui::App| {
                                            si.update(cx, |t, cx| t.reset(cx));
                                        });
                                }
                                clear
                            })
                    )
                    .child(div().flex_1())
                    .child({
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("btn-new-product")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .hover(move |s| s.bg(hover_bg))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_product_form(window, cx);
                            }))
                            .child("+ New Product")
                    })
                    .child({
                        let mut btn = div()
                            .id("btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .child("+ New Entry");

                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_stock_form(window, cx);
                                }));
                        }
                        btn
                    })
                    // .child({  // Import XLSX — disabled for alpha release
                    //     let hover_bg = rgb(c.surface_hover);
                    //     div()
                    //         .id("btn-import-xlsx")
                    //         .px(px(12.)).py(px(4.)).rounded(px(4.))
                    //         .bg(rgb(c.surface_default))
                    //         .hover(move |s| s.bg(hover_bg))
                    //         .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                    //         .cursor_pointer()
                    //         .on_mouse_down(MouseButton::Left, cx.listener(|_, _: &MouseDownEvent, _: &mut Window, cx| {
                    //             cx.emit(InventoryPanelEvent::ImportXlsxRequested);
                    //         }))
                    //         .child("↑ Import XLSX")
                    // })
            )
            .child(content);

        if let Some(form) = &self.stock_form {
            root = root.child(form.clone());
        }
        if let Some(form) = &self.product_form {
            root = root.child(form.clone());
        }

        // Context menu overlay
        let ctx_menu = self.store.read(cx).context_menu.clone();
        if let Some(target) = ctx_menu {
            let info_line = {
                let store = self.store.read(cx);
                store.products
                    .iter()
                    .find(|p| p.product.id == target.product_id)
                    .map(|p| format!(
                        "Stock: {:.1} {} (min {:.1})",
                        p.current_stock, p.product.unit, p.product.min_stock_level
                    ))
                    .unwrap_or_default()
            };

            let pid  = target.product_id;
            let name = target.product_name.clone();

            // Clamp so the menu stays fully within the window viewport.
            // Menu is 220px wide; height estimate covers all items (~200px).
            const MENU_W: f32 = 220.0;
            const MENU_H: f32 = 200.0;
            let menu_x = target.x.min((viewport.width.as_f32()  - MENU_W).max(0.0));
            let menu_y = target.y.min((viewport.height.as_f32() - MENU_H).max(0.0));

            root = root
                .child(
                    div()
                        .absolute().inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseDownEvent, _: &mut Window, cx| {
                                this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                            }),
                        )
                )
                .child(
                    div()
                        .absolute()
                        .left(px(menu_x))
                        .top(px(menu_y))
                        .w(px(220.))
                        .bg(rgb(c.surface_default))
                        .rounded(px(6.))
                        .shadow_md()
                        .child(
                            div()
                                .px(px(12.)).pt(px(10.)).pb(px(4.))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(target.product_name.clone())
                        )
                        .child(
                            div()
                                .px(px(12.)).pb(px(8.))
                                .text_size(rems(0.846))
                                .text_color(rgb(c.text_muted))
                                .child(info_line)
                        )
                        .child(
                            div()
                                .h(px(1.))
                                .bg(rgb(c.surface_default))
                        )
                        .child({
                            let product_for_edit = {
                                let store = self.store.read(cx);
                                store.products.iter().find(|p| p.product.id == pid).map(|p| p.product.clone())
                            };
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-inv-edit-product")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("Edit Product")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        if let Some(product) = product_for_edit.clone() {
                                            this.open_edit_form(product, window, cx);
                                        }
                                    }),
                                )
                        })
                        .child({
                            let n = name.clone();
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-inv-price-history")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("Price History")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        cx.emit(InventoryPanelEvent::ShowPriceHistory {
                                            product_id: pid,
                                            name:       n.clone(),
                                        });
                                    }),
                                )
                        })
                        .child({
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-inv-add-price")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("Add Price Entry")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        cx.emit(InventoryPanelEvent::ShowPriceEntryForm {
                                            product_id: pid,
                                            name:       name.clone(),
                                        });
                                    }),
                                )
                        })
                );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_panel_event_show_price_history_carries_data() {
        let ev = InventoryPanelEvent::ShowPriceHistory {
            product_id: 5,
            name:       "Lens 24mm".to_string(),
        };
        match ev {
            InventoryPanelEvent::ShowPriceHistory { product_id, name } => {
                assert_eq!(product_id, 5);
                assert_eq!(name, "Lens 24mm");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn inventory_panel_event_show_price_entry_form_carries_data() {
        let ev = InventoryPanelEvent::ShowPriceEntryForm {
            product_id: 12,
            name:       "NVR".to_string(),
        };
        match ev {
            InventoryPanelEvent::ShowPriceEntryForm { product_id, name } => {
                assert_eq!(product_id, 12);
                assert_eq!(name, "NVR");
            }
            _ => panic!("wrong variant"),
        }
    }
}
