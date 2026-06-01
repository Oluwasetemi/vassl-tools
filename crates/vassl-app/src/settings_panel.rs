use gpui::{Context, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb};
use vassl_ui::{TextInput, ThemeHandle};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SettingsCategory {
    #[default]
    General,
    Appearance,
    Inventory,
    PriceBook,
    Quotations,
}

/// Identifies which inline select/picker is currently open.
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(dead_code)] // Theme reserved for future inline theme select
pub enum SettingSelect { Theme, Currency, FontPicker }

pub struct SettingsPanel {
    pub active_category: SettingsCategory,
    font_names:          Vec<String>,
    open_select:         Option<SettingSelect>,

    // General
    pub user_name:    gpui::Entity<TextInput>,
    pub company_name: gpui::Entity<TextInput>,

    // Appearance
    pub theme:       String,   // "dark" | "light"
    pub font_family: String,
    pub font_size:   f64,      // 10.0–24.0, step 0.5

    // Inventory
    pub low_stock:    gpui::Entity<TextInput>,
    pub default_unit: gpui::Entity<TextInput>,

    // Price Book
    pub currency:       String,  // "USD" | "JMD"
    pub usd_to_jmd:     gpui::Entity<TextInput>,
    pub default_margin: gpui::Entity<TextInput>,

    // Quotations
    pub quote_prefix:   gpui::Entity<TextInput>,
    pub tax_rate:       gpui::Entity<TextInput>,
    pub notes_template: gpui::Entity<TextInput>,

    focus_handle: FocusHandle,
}

impl SettingsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            active_category: SettingsCategory::General,
            font_names:      Vec::new(),
            open_select:     None,
            user_name:       cx.new(|cx| TextInput::with_placeholder("e.g. Alice Kamalu", cx)),
            company_name:    cx.new(|cx| TextInput::with_placeholder("e.g. Kamalu Ltd.", cx)),
            theme:           "dark".into(),
            font_family:     "system-ui".into(),
            font_size:       13.0,
            low_stock:       cx.new(|cx| TextInput::with_placeholder("5", cx)),
            default_unit:    cx.new(|cx| TextInput::with_placeholder("pcs", cx)),
            currency:        "USD".into(),
            usd_to_jmd:      cx.new(|cx| TextInput::with_placeholder("157.50", cx)),
            default_margin:  cx.new(|cx| TextInput::with_placeholder("20", cx)),
            quote_prefix:    cx.new(|cx| TextInput::with_placeholder("VASSL", cx)),
            tax_rate:        cx.new(|cx| TextInput::with_placeholder("0", cx)),
            notes_template:  cx.new(|cx| TextInput::with_placeholder("", cx)),
            focus_handle:    cx.focus_handle(),
        }
    }

    fn category_label(cat: SettingsCategory) -> &'static str {
        match cat {
            SettingsCategory::General    => "General",
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Inventory  => "Inventory",
            SettingsCategory::PriceBook  => "Price Book",
            SettingsCategory::Quotations => "Quotations",
        }
    }
}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for SettingsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active = self.active_category;

        let categories = [
            SettingsCategory::General,
            SettingsCategory::Appearance,
            SettingsCategory::Inventory,
            SettingsCategory::PriceBook,
            SettingsCategory::Quotations,
        ];

        // ── category nav (160px) ──────────────────────────────────────
        let nav = div()
            .w(px(160.)).h_full()
            .bg(rgb(c.sidebar_bg))
            .border_r_1().border_color(rgb(c.surface_default))
            .flex().flex_col().pt(px(8.))
            .children(categories.iter().map(|&cat| {
                let is_active = active == cat;
                let bg = if is_active { c.surface_active } else { c.sidebar_bg };
                let fg = if is_active { c.text_default   } else { c.text_muted };
                div()
                    .id(format!("settings-cat-{}", Self::category_label(cat)))
                    .px(px(16.)).py(px(9.))
                    .bg(rgb(bg)).text_color(rgb(fg))
                    .text_size(px(13.)).cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.active_category = cat;
                        cx.notify();
                    }))
                    .child(Self::category_label(cat))
            }));

        // ── content area ─────────────────────────────────────────────
        let content = div()
            .id("settings-content-scroll")
            .flex_1().h_full().overflow_y_scroll()
            .bg(rgb(c.canvas_bg))
            .flex().flex_col()
            .child(
                div().px(px(32.)).pt(px(24.)).pb(px(8.))
                    .child(div().text_size(px(18.)).text_color(rgb(c.text_default))
                        .child(Self::category_label(active)))
                    .child(div().text_size(px(12.)).text_color(rgb(c.text_muted)).mt(px(2.))
                        .child(format!("{} Settings", Self::category_label(active))))
            )
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            .child(
                div().px(px(32.)).py(px(8.))
                    .child(div().text_size(px(12.)).text_color(rgb(c.text_muted))
                        .child("(settings rows will appear here)"))
            );

        div()
            .flex().flex_row().flex_1().h_full()
            .track_focus(&self.focus_handle)
            .child(nav)
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_categories_are_distinct() {
        assert_ne!(SettingsCategory::General,    SettingsCategory::Appearance);
        assert_ne!(SettingsCategory::Appearance, SettingsCategory::Inventory);
        assert_ne!(SettingsCategory::Inventory,  SettingsCategory::PriceBook);
        assert_ne!(SettingsCategory::PriceBook,  SettingsCategory::Quotations);
    }

    #[test]
    fn default_category_is_general() {
        assert_eq!(SettingsCategory::default(), SettingsCategory::General);
    }

    #[test]
    fn setting_keys_follow_module_dot_name_convention() {
        let keys = [
            "general.user_name", "general.company_name",
            "appearance.theme", "appearance.font_family", "appearance.font_size",
            "inventory.low_stock_threshold", "inventory.default_unit",
            "pricebook.currency", "pricebook.usd_to_jmd_rate", "pricebook.default_margin",
            "quotations.prefix", "quotations.tax_rate", "quotations.notes_template",
        ];
        for key in &keys {
            assert!(key.contains('.'), "key '{key}' must contain a dot separator");
            let parts: Vec<&str> = key.splitn(2, '.').collect();
            assert_eq!(parts.len(), 2);
            assert!(!parts[0].is_empty());
            assert!(!parts[1].is_empty());
        }
    }
}
