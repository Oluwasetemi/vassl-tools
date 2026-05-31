use gpui::{Context, Entity, IntoElement, Render, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::product_list::ProductList;
use crate::restock::RestockAlerts;
use crate::store::InventoryStore;
use crate::InventoryStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab { Products, Restock }

pub struct InventoryPanel {
    #[allow(dead_code)] // used by Task 6 form
    store:          Entity<InventoryStore>,
    product_list:   Entity<ProductList>,
    restock_alerts: Entity<RestockAlerts>,
    active_tab:     Tab,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<InventoryStoreHandle>().0.clone();
        let product_list   = cx.new(|cx| ProductList::new(store.clone(), cx));
        let restock_alerts = cx.new(|cx| RestockAlerts::new(store.clone(), cx));

        store.update(cx, |s, cx| s.load_products(cx));

        Self { store, product_list, restock_alerts, active_tab: Tab::Products }
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Products      => content.child(self.product_list.clone()),
            Tab::Restock => content.child(self.restock_alerts.clone()),
        };

        div()
            .flex_1().flex().flex_col().h_full()
            // Tab bar header
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
            )
            // Active tab content
            .child(content)
    }
}
