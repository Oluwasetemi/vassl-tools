use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::product_list::ProductList;
use crate::restock::RestockAlerts;
use crate::stock_form::{StockEntryForm, StockFormEvent};
use crate::store::InventoryStore;
use crate::InventoryStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab { Products, Restock }

pub struct InventoryPanel {
    store:          Entity<InventoryStore>,
    product_list:   Entity<ProductList>,
    restock_alerts: Entity<RestockAlerts>,
    active_tab:     Tab,
    stock_form:     Option<Entity<StockEntryForm>>,
    _form_sub:      Option<Subscription>,
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
            active_tab: Tab::Products,
            stock_form: None,
            _form_sub:  None,
        }
    }

    fn open_stock_form(&mut self, cx: &mut Context<Self>) {
        // Read needed data from store — borrow scoped
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

        let form = cx.new(|cx| StockEntryForm::new(self.store.clone(), product_id, product_name, cx));

        let sub = cx.subscribe(&form, |this, _form, ev: &StockFormEvent, cx| {
            match ev {
                StockFormEvent::Submitted | StockFormEvent::Cancelled => {
                    this.stock_form = None;
                    this._form_sub  = None;
                    cx.notify();
                }
            }
        });

        self.stock_form = Some(form);
        self._form_sub  = Some(sub);
        cx.notify();
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab    = self.active_tab;
        let has_selection = { self.store.read(cx).selected_product_id.is_some() };

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Products => content.child(self.product_list.clone()),
            Tab::Restock  => content.child(self.restock_alerts.clone()),
        };

        let mut root = div()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    // Products tab
                    .child(
                        div()
                            .id("tab-products")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Products { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Products;
                                cx.notify();
                            }))
                            .child("Products")
                    )
                    // Restock Alerts tab
                    .child(
                        div()
                            .id("tab-restock")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Restock { colors::SURFACE_ACTIVE } else { colors::CANVAS_BG }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Restock;
                                cx.notify();
                            }))
                            .child("Restock Alerts")
                    )
                    // Spacer
                    .child(div().flex_1())
                    // New Entry button
                    .child(
                        div()
                            .id("btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.open_stock_form(cx);
                            }))
                            .child("+ New Entry")
                    )
            )
            .child(content);

        // Overlay modal if open
        if let Some(form) = &self.stock_form {
            root = root.child(form.clone());
        }

        root
    }
}
