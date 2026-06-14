use gpui::{Context, Entity, EventEmitter, Focusable, IntoElement, MouseButton, MouseDownEvent,
           Render, Subscription, Window, div, prelude::*, px, rems, rgb};
use vassl_ui::NewRecord;
use vassl_ui::{AppSettings, TextInput, ThemeHandle, text_field, tooltip, tooltip_keyed};

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
    detail_open:    bool,
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

        cx.observe(&store, |this, _, cx| {
            if this.store.read(cx).detail_requested {
                this.detail_open = true;
                this.store.update(cx, |s, _| s.detail_requested = false);
                cx.notify();
            }
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
            detail_open:    false,
        }
    }

    pub fn show_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_open = true;
        cx.notify();
    }

    pub fn hide_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_open = false;
        cx.notify();
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_next(cx)) {
            self.product_list.update(cx, |list, _| {
                list.scroll_handle.scroll_to_item(idx, gpui::ScrollStrategy::Top);
            });
        }
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_prev(cx)) {
            self.product_list.update(cx, |list, _| {
                list.scroll_handle.scroll_to_item(idx, gpui::ScrollStrategy::Top);
            });
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
        let fh = form.read(cx).focus_handle(cx);
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
        window.defer(cx, move |window, cx| { window.focus(&fh, cx); });
    }

    pub fn create_product_form(&mut self, cx: &mut Context<Self>) -> Option<gpui::FocusHandle> {
        if self.product_form.is_some() { return None; }
        let form  = cx.new(|cx| ProductForm::new(self.store.clone(), cx));
        let fh = form.read(cx).focus_handle(cx);
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
        Some(fh)
    }

    pub fn open_product_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fh) = self.create_product_form(cx) {
            window.defer(cx, move |window, cx| { window.focus(&fh, cx); });
        }
    }

    fn open_edit_form(&mut self, product: Product, window: &mut Window, cx: &mut Context<Self>) {
        if self.product_form.is_some() { return; }
        let current_stock = self.store.read(cx).products.iter()
            .find(|p| p.product.id == product.id)
            .map(|p| p.current_stock)
            .unwrap_or(0.0);
        let form  = cx.new(|cx| ProductForm::new_edit(self.store.clone(), &product, current_stock, cx));
        let fh = form.read(cx).focus_handle(cx);
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
        window.defer(cx, move |window, cx| { window.focus(&fh, cx); });
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();
        let viewport      = window.viewport_size();

        #[cfg(target_os = "macos")]
        let mod_key = "⌘";
        #[cfg(not(target_os = "macos"))]
        let mod_key = "Ctrl+";

        let has_query = !self.search_input.read(cx).text().is_empty();

        let detail_open = self.detail_open;

        // Build content area (list or restock)
        let list_content = div().flex_1().h_full().flex().flex_col();
        let list_content = match active_tab {
            Tab::Products => list_content.child(self.product_list.clone()),
            Tab::Restock  => list_content.child(self.restock_alerts.clone()),
        };

        // When detail open, wrap in a flex_row with the sidebar on the right
        let selected_product = if detail_open {
            let store = self.store.read(cx);
            store.selected_product_id.and_then(|pid| {
                store.products.iter().find(|p| p.product.id == pid).cloned()
            })
        } else {
            None
        };

        let content_area = if detail_open {
            let detail_sidebar = {
                let border_col = rgb(c.surface_default);
                let mut sidebar = div()
                    .w(px(300.))
                    .flex_shrink_0()
                    .border_l_1()
                    .border_color(border_col)
                    .flex().flex_col()
                    .bg(rgb(c.canvas_bg))
                    // header
                    .child(
                        div()
                            .flex().flex_row().items_center()
                            .px(px(12.)).py(px(10.))
                            .bg(rgb(c.sidebar_bg))
                            .border_b_1().border_color(rgb(c.surface_default))
                            .child(div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD).child("Product Details"))
                            .child(
                                div()
                                    .id("inv-detail-close")
                                    .px(px(8.)).py(px(4.)).rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(c.surface_hover)))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                    .child("×")
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                        this.hide_detail(cx);
                                    }))
                            )
                    );

                if let Some(pw) = selected_product {
                    let p = &pw.product;
                    sidebar = sidebar.child(
                        div().id("inv-detail-scroll").flex_1().min_h(px(0.)).overflow_y_scroll().pb(px(64.))
                            .flex().flex_col().gap(px(0.))
                            .child(detail_field("SKU", p.sku.clone(), &c))
                            .child(detail_field("Name", p.name.clone(), &c))
                            .child(detail_field("Category", p.category.clone().unwrap_or_else(|| "—".into()), &c))
                            .child(detail_field("Unit", p.unit.clone(), &c))
                            .child(detail_field("Min Stock", format!("{:.1}", p.min_stock_level), &c))
                            .child(detail_field("Current Stock", format!("{:.1}", pw.current_stock), &c))
                            .child(detail_field("Duty %", format!("{:.1}%", p.duty_percent), &c))
                            .child(detail_field("Model No.", p.model_number.clone().unwrap_or_else(|| "—".into()), &c))
                            .child(detail_field("Part No.", p.part_number.clone().unwrap_or_else(|| "—".into()), &c))
                            .child(detail_field("End of Life", if p.end_of_life { "Yes" } else { "No" }, &c))
                            .child(detail_field("Replacement", p.replacement.clone().unwrap_or_else(|| "—".into()), &c))
                            .child(detail_field("Description", p.description.clone().unwrap_or_else(|| "—".into()), &c))
                            .child(detail_field("Notes", p.notes.clone().unwrap_or_else(|| "—".into()), &c))
                    );
                } else {
                    sidebar = sidebar.child(
                        div().flex_1().flex().items_center().justify_center()
                            .text_color(rgb(c.text_muted)).text_size(rems(0.923))
                            .child("Select a product to view details")
                    );
                }

                sidebar
            };

            div().flex_1().h_full().flex().flex_row()
                .child(list_content)
                .child(detail_sidebar)
        } else {
            div().flex_1().h_full().flex().flex_row()
                .child(list_content)
        };

        let content = content_area;

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
                                        text_field("", self.search_input.clone(), focused, false, cx)
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
                            .tooltip(tooltip_keyed("New Product", format!("{mod_key}N")))
                            .child("+ New Product")
                    })
                    .child({
                        let mut btn = div()
                            .id("btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .tooltip(tooltip("New Stock Entry"))
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
        let allow_delete = cx.global::<AppSettings>().allow_delete;
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
            // Menu is 220px wide; height covers header + info + separator + 3 items.
            const MENU_W: f32 = 220.0;
            const MENU_H: f32 = 260.0;
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
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-inv-view-details")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("View Details")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                        this.store.update(cx, |s, cx| s.select_product(pid, cx));
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        this.show_detail(cx);
                                    }),
                                )
                        })
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
                        .when(allow_delete, |menu| {
                            let hover_bg = rgb(c.surface_hover);
                            menu.child(div().h(px(1.)).bg(rgb(c.surface_default)))
                                .child({
                                    div()
                                        .id("ctx-inv-delete")
                                        .px(px(12.)).py(px(8.))
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg))
                                        .text_size(rems(1.))
                                        .text_color(rgb(c.status_red))
                                        .child("Delete Product")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                                this.store.update(cx, |s, cx| {
                                                    s.clear_context_menu(cx);
                                                    s.delete_product(pid, cx);
                                                });
                                            }),
                                        )
                                })
                        })
                );
        }

        root
    }
}

fn detail_field(label: impl Into<String>, value: impl Into<String>, c: &vassl_ui::ThemeColors) -> impl gpui::IntoElement {
    div()
        .flex().flex_col().px(px(12.)).py(px(8.))
        .border_b_1().border_color(rgb(c.surface_default))
        .child(
            div().text_size(rems(0.769)).text_color(rgb(c.text_muted)).mb(px(2.))
                .child(label.into())
        )
        .child(
            div().text_size(rems(0.923)).text_color(rgb(c.text_default))
                .child(value.into())
        )
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
