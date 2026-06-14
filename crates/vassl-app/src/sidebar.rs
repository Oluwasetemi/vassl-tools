use gpui::{
    Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px, rgb,
};
use vassl_ui::{AppSettings, ThemeHandle, tooltip_keyed, tooltip};
use crate::actions::{Logout, OpenAuditLog};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveModule {
    Inventory,
    Quotations,
    PriceBook,
    Suppliers,
    Projects,
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
        let c      = cx.global::<ThemeHandle>().0.clone();
        let perms  = cx.global::<AppSettings>().clone();
        let active = self.active;

        #[cfg(target_os = "macos")]
        let mod_key = "⌘";
        #[cfg(not(target_os = "macos"))]
        let mod_key = "Ctrl+";

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

        let show_inventory  = perms.is_admin || perms.can_inventory;
        let show_quotations = perms.is_admin || perms.can_quotations;
        let show_pricebook  = perms.is_admin || perms.can_pricebook;
        // Suppliers are tied to inventory access.
        let show_suppliers  = show_inventory;

        div()
            .w(px(48.)).h_full()
            .bg(rgb(c.sidebar_bg))
            .flex().flex_col().justify_between()
            .child(
                div().flex().flex_col()
                    .when(show_inventory,  |d| d.child(make_btn(ActiveModule::Inventory,  "I", "btn-inventory",  "Inventory",  Some("1"))))
                    .when(show_quotations, |d| d.child(make_btn(ActiveModule::Quotations, "Q", "btn-quotations", "Quotations", Some("2"))))
                    .when(show_pricebook,  |d| d.child(make_btn(ActiveModule::PriceBook,  "P", "btn-pricebook",  "Price Book", Some("3"))))
                    .when(show_suppliers,  |d| d.child(make_btn(ActiveModule::Suppliers,  "S", "btn-suppliers",  "Suppliers",  Some("4"))))
            )
            .child(
                div().flex().flex_col()
                    .when(perms.is_admin, |d| {
                        let hover_bg = rgb(c.surface_hover);
                        d.child(
                            div()
                                .id("btn-auditlog")
                                .w(px(36.)).h(px(36.)).m(px(6.))
                                .rounded(px(6.))
                                .bg(rgb(c.surface_default)).text_color(rgb(c.text_muted))
                                .flex().items_center().justify_center()
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .child("A")
                                .tooltip(tooltip_keyed("Audit Log", format!("{mod_key}⇧A")))
                                .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, window, cx| {
                                    window.dispatch_action(Box::new(OpenAuditLog), cx);
                                }))
                        )
                    })
                    .child(make_btn(ActiveModule::Settings, "⚙", "btn-settings", "Settings", Some(",")))
                    .child({
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("btn-logout")
                            .w(px(36.)).h(px(36.)).m(px(6.))
                            .rounded(px(6.))
                            .bg(rgb(c.surface_default)).text_color(rgb(c.text_muted))
                            .flex().items_center().justify_center()
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover_bg))
                            .child("↩")
                            .tooltip(tooltip("Sign Out"))
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, window, cx| {
                                window.dispatch_action(Box::new(Logout), cx);
                            }))
                    })
            )
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
