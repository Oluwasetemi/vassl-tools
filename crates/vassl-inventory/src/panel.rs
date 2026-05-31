use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px, rgb};

use crate::colors;
use crate::product_list::ProductList;
use crate::store::InventoryStore;
use crate::InventoryStoreHandle;

pub struct InventoryPanel {
    #[allow(dead_code)]
    store:        Entity<InventoryStore>,
    product_list: Entity<ProductList>,
}

impl InventoryPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<InventoryStoreHandle>().0.clone();
        let product_list = cx.new(|cx| ProductList::new(store.clone(), cx));

        // Kick off initial data load
        store.update(cx, |s, cx| s.load_products(cx));

        Self { store, product_list }
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            // Header bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(16.))
                    .py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(colors::TEXT_DEFAULT))
                            .child("Inventory")
                    )
            )
            // Product list
            .child(self.product_list.clone())
    }
}
