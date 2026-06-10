use gpui::{
    Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px, rgb,
};
use vassl_ui::{ThemeHandle, tooltip_keyed, tooltip};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveModule {
    Inventory,
    Quotations,
    PriceBook,
    Suppliers,
    Settings,
}

pub struct Sidebar {
    pub active: ActiveModule,
}

impl Sidebar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { active: ActiveModule::Inventory }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active = self.active;

        // Platform modifier prefix shown in keybinding badges.
        #[cfg(target_os = "macos")]
        let mod_key = "⌘";
        #[cfg(not(target_os = "macos"))]
        let mod_key = "Ctrl+";

        // Returns a sidebar icon button, optionally carrying a tooltip with a keybinding badge.
        let make_btn = |module: ActiveModule,
                        label:   &'static str,
                        id:      &'static str,
                        tip:     &'static str,
                        key:     Option<&'static str>| {
            let is_active = active == module;
            let bg        = if is_active { rgb(c.surface_active) } else { rgb(c.surface_default) };
            let fg        = if is_active { rgb(c.text_on_active) } else { rgb(c.text_muted) };
            let hover_bg  = rgb(c.surface_hover);

            let btn = div()
                .id(id)
                .w(px(36.)).h(px(36.)).m(px(6.))
                .rounded(px(6.))
                .bg(bg).text_color(fg)
                .flex().items_center().justify_center()
                .cursor_pointer()
                .when(!is_active, |d| d.hover(move |s| s.bg(hover_bg)))
                .child(label)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, _window, cx| {
                    this.active = module;
                    cx.notify();
                }));

            match key {
                Some(k) => btn.tooltip(tooltip_keyed(tip, format!("{mod_key}{k}"))),
                None    => btn.tooltip(tooltip(tip)),
            }
        };

        div()
            .w(px(48.)).h_full()
            .bg(rgb(c.sidebar_bg))
            .flex().flex_col().justify_between()
            .child(
                div().flex().flex_col()
                    .child(make_btn(ActiveModule::Inventory,  "I", "btn-inventory",  "Inventory",  Some("1")))
                    .child(make_btn(ActiveModule::Quotations, "Q", "btn-quotations", "Quotations", Some("2")))
                    .child(make_btn(ActiveModule::PriceBook,  "P", "btn-pricebook",  "Price Book", Some("3")))
                    .child(make_btn(ActiveModule::Suppliers,  "S", "btn-suppliers",  "Suppliers",  Some("4"))),
            )
            .child(make_btn(ActiveModule::Settings, "⚙", "btn-settings", "Settings", Some(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_module_is_inventory() {
        assert_eq!(ActiveModule::Inventory, ActiveModule::Inventory);
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

    #[test]
    fn suppliers_module_is_distinct() {
        assert_ne!(ActiveModule::Suppliers, ActiveModule::Inventory);
        assert_ne!(ActiveModule::Suppliers, ActiveModule::PriceBook);
        assert_ne!(ActiveModule::Suppliers, ActiveModule::Settings);
    }
}
