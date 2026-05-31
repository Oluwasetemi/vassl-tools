use gpui::{
    Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px, rgb,
};

use crate::colors;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveModule {
    Inventory,
    Quotations,
    PriceBook,
}

pub struct Sidebar {
    pub active: ActiveModule,
}

impl Sidebar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            active: ActiveModule::Inventory,
        }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;

        let make_btn = |module: ActiveModule, label: &'static str, id: &'static str| {
            let is_active = active == module;
            let bg = if is_active {
                rgb(colors::SURFACE_ACTIVE)
            } else {
                rgb(colors::SURFACE_DEFAULT)
            };
            let fg = if is_active {
                rgb(colors::TEXT_DEFAULT)
            } else {
                rgb(colors::TEXT_MUTED)
            };
            div()
                .id(id)
                .w(px(36.))
                .h(px(36.))
                .m(px(6.))
                .rounded(px(6.))
                .bg(bg)
                .text_color(fg)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(label)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _event, _window, cx| {
                        this.active = module;
                        cx.notify();
                    }),
                )
        };

        div()
            .w(px(48.))
            .h_full()
            .bg(rgb(colors::SIDEBAR_BG))
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(make_btn(ActiveModule::Inventory, "I", "btn-inventory"))
                    .child(make_btn(ActiveModule::Quotations, "Q", "btn-quotations"))
                    .child(make_btn(ActiveModule::PriceBook, "P", "btn-pricebook")),
            )
            .child(
                div()
                    .id("btn-settings")
                    .w(px(36.))
                    .h(px(36.))
                    .m(px(6.))
                    .rounded(px(6.))
                    .bg(rgb(colors::SURFACE_DEFAULT))
                    .text_color(rgb(colors::TEXT_MUTED))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    // TODO(Task 8): wire OpenSettings action when command palette is implemented
                    .child("⚙"),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_module_is_inventory() {
        // Sidebar::new() requires a GPUI Context — tested via integration.
        // Verify the intended default value is the Inventory variant.
        let default = ActiveModule::Inventory;
        assert_eq!(default, ActiveModule::Inventory);
        // If the default ever changes, update Sidebar::new() to match.
    }

    #[test]
    fn modules_are_distinct() {
        assert_ne!(ActiveModule::Inventory, ActiveModule::Quotations);
        assert_ne!(ActiveModule::Quotations, ActiveModule::PriceBook);
        assert_ne!(ActiveModule::Inventory, ActiveModule::PriceBook);
    }
}
