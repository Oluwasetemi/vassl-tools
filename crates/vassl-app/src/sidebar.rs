use gpui::{
    Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px, rgb,
};
use vassl_ui::ThemeHandle;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveModule {
    Inventory,
    Quotations,
    PriceBook,
    Settings,
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
        let c = cx.global::<ThemeHandle>().0.clone();
        let active = self.active;

        let make_btn = |module: ActiveModule, label: &'static str, id: &'static str| {
            let is_active = active == module;
            let bg = if is_active { rgb(c.surface_active) } else { rgb(c.surface_default) };
            let fg = if is_active { rgb(c.text_default)   } else { rgb(c.text_muted) };
            div()
                .id(id)
                .w(px(36.)).h(px(36.)).m(px(6.))
                .rounded(px(6.))
                .bg(bg).text_color(fg)
                .flex().items_center().justify_center()
                .cursor_pointer()
                .child(label)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, _window, cx| {
                    this.active = module;
                    cx.notify();
                }))
        };

        div()
            .w(px(48.)).h_full()
            .bg(rgb(c.sidebar_bg))
            .flex().flex_col().justify_between()
            .child(
                div().flex().flex_col()
                    .child(make_btn(ActiveModule::Inventory,  "I", "btn-inventory"))
                    .child(make_btn(ActiveModule::Quotations, "Q", "btn-quotations"))
                    .child(make_btn(ActiveModule::PriceBook,  "P", "btn-pricebook")),
            )
            .child(make_btn(ActiveModule::Settings, "⚙", "btn-settings"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_module_is_inventory() {
        let default = ActiveModule::Inventory;
        assert_eq!(default, ActiveModule::Inventory);
    }

    #[test]
    fn modules_are_distinct() {
        assert_ne!(ActiveModule::Inventory,  ActiveModule::Quotations);
        assert_ne!(ActiveModule::Quotations, ActiveModule::PriceBook);
        assert_ne!(ActiveModule::Inventory,  ActiveModule::PriceBook);
    }

    #[test]
    fn settings_module_is_distinct() {
        assert_ne!(ActiveModule::Settings, ActiveModule::Inventory);
        assert_ne!(ActiveModule::Settings, ActiveModule::Quotations);
        assert_ne!(ActiveModule::Settings, ActiveModule::PriceBook);
    }
}
