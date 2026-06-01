use gpui::{Context, Entity, EventEmitter, IntoElement, MouseButton, MouseDownEvent,
           Render, Subscription, Window, div, prelude::*, px, rgb};
use vassl_ui::ThemeHandle;

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
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<InventoryStoreHandle>().0.clone();
        let product_list   = cx.new(|cx| ProductList::new(store.clone(), cx));
        let restock_alerts = cx.new(|cx| RestockAlerts::new(store.clone(), cx));

        store.update(cx, |s, cx| s.load_products(cx));

        Self {
            store,
            product_list,
            restock_alerts,
            active_tab:     Tab::Products,
            stock_form:     None,
            _form_sub:      None,
            product_form:   None,
            _prod_form_sub: None,
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

    fn open_product_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.product_form.is_some() { return; }
        let form  = cx.new(|cx| ProductForm::new(self.store.clone(), cx));
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Products => content.child(self.product_list.clone()),
            Tab::Restock  => content.child(self.restock_alerts.clone()),
        };

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child(
                        div()
                            .id("tab-products")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Products { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Products;
                                cx.notify();
                            }))
                            .child("Products")
                    )
                    .child(
                        div()
                            .id("tab-restock")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Restock { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Restock;
                                cx.notify();
                            }))
                            .child("Restock Alerts")
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("btn-new-product")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_product_form(window, cx);
                            }))
                            .child("+ New Product")
                    )
                    .child({
                        let mut btn = div()
                            .id("btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
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
                        .left(px(target.x))
                        .top(px(target.y))
                        .w(px(220.))
                        .bg(rgb(c.surface_default))
                        .rounded(px(6.))
                        .shadow_md()
                        .child(
                            div()
                                .px(px(12.)).pt(px(10.)).pb(px(4.))
                                .text_size(px(13.))
                                .text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(target.product_name.clone())
                        )
                        .child(
                            div()
                                .px(px(12.)).pb(px(8.))
                                .text_size(px(11.))
                                .text_color(rgb(c.text_muted))
                                .child(info_line)
                        )
                        .child(
                            div()
                                .h(px(1.))
                                .bg(rgb(c.surface_default))
                        )
                        .child({
                            let n = name.clone();
                            div()
                                .id("ctx-inv-price-history")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .text_size(px(13.))
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
                        .child(
                            div()
                                .id("ctx-inv-add-price")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .text_size(px(13.))
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
                        )
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
