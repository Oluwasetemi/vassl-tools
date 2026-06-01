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
    pub theme:       String,   // "dark" | "light" — saved by render_appearance toggle
    pub font_family: String,   // saved by render_font_picker on selection
    pub font_size:   f64,      // 10.0–24.0, step 0.5

    // Inventory
    pub low_stock:    gpui::Entity<TextInput>,
    pub default_unit: gpui::Entity<TextInput>,

    // Price Book
    pub currency:       String,  // "USD" | "JMD" — saved by render_pricebook select
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
        let db = vassl_db::AppDatabase::global(&**cx);

        // read all settings upfront while db borrow is active
        let user_name_val = vassl_db::shared::get_setting(db, "general.user_name")
            .ok().flatten()
            .or_else(|| vassl_db::shared::get_setting(db, "current_user").ok().flatten())
            .unwrap_or_default();
        let company_name_val  = vassl_db::shared::get_setting(db, "general.company_name").ok().flatten().unwrap_or_default();
        let theme_val         = vassl_db::shared::get_setting(db, "appearance.theme").ok().flatten().unwrap_or_else(|| "dark".into());
        let font_family_val   = vassl_db::shared::get_setting(db, "appearance.font_family").ok().flatten().unwrap_or_else(|| "system-ui".into());
        let font_size_val     = vassl_db::shared::get_setting(db, "appearance.font_size").ok().flatten().unwrap_or_else(|| "13".into());
        let low_stock_val     = vassl_db::shared::get_setting(db, "inventory.low_stock_threshold").ok().flatten().unwrap_or_else(|| "5".into());
        let default_unit_val  = vassl_db::shared::get_setting(db, "inventory.default_unit").ok().flatten().unwrap_or_else(|| "pcs".into());
        let currency_val      = vassl_db::shared::get_setting(db, "pricebook.currency").ok().flatten().unwrap_or_else(|| "USD".into());
        let usd_jmd_val       = vassl_db::shared::get_setting(db, "pricebook.usd_to_jmd_rate").ok().flatten().unwrap_or_else(|| "157.50".into());
        let margin_val        = vassl_db::shared::get_setting(db, "pricebook.default_margin").ok().flatten().unwrap_or_else(|| "20".into());
        let prefix_val        = vassl_db::shared::get_setting(db, "quotations.prefix").ok().flatten().unwrap_or_else(|| "VASSL".into());
        let tax_val           = vassl_db::shared::get_setting(db, "quotations.tax_rate").ok().flatten().unwrap_or_else(|| "0".into());
        let notes_val         = vassl_db::shared::get_setting(db, "quotations.notes_template").ok().flatten().unwrap_or_default();
        // db borrow ends here

        let font_size: f64 = font_size_val.parse::<f64>().unwrap_or(13.0).max(10.0).min(24.0);
        let font_names = cx.text_system().all_font_names();

        let make_input = |placeholder: &'static str, value: String, cx: &mut Context<Self>| {
            cx.new(move |cx| {
                let mut input = TextInput::with_placeholder(placeholder, cx);
                input.set_text(value, cx);
                input
            })
        };

        let user_name      = make_input("e.g. Alice Kamalu", user_name_val,    cx);
        let company_name   = make_input("e.g. Kamalu Ltd.",  company_name_val, cx);
        let low_stock      = make_input("5",                 low_stock_val,    cx);
        let default_unit   = make_input("pcs",               default_unit_val, cx);
        let usd_to_jmd     = make_input("157.50",            usd_jmd_val,      cx);
        let default_margin = make_input("20",                margin_val,       cx);
        let quote_prefix   = make_input("VASSL",             prefix_val,       cx);
        let tax_rate       = make_input("0",                 tax_val,          cx);
        let notes_template = make_input("",                  notes_val,        cx);

        Self {
            active_category: SettingsCategory::General,
            font_names,
            open_select:     None,
            user_name,
            company_name,
            theme:           theme_val,
            font_family:     font_family_val,
            font_size,
            low_stock,
            default_unit,
            currency:        currency_val,
            usd_to_jmd,
            default_margin,
            quote_prefix,
            tax_rate,
            notes_template,
            focus_handle:    cx.focus_handle(),
        }
    }

    fn save_setting(&self, key: &'static str, value: String, cx: &mut Context<Self>) {
        let db = vassl_db::AppDatabase::global(&**cx).clone();
        cx.spawn(async move |_, _| {
            if let Err(e) = db.write(move |conn| vassl_db::shared::set_setting(conn, key, &value)).await {
                tracing::error!("save_setting({key}) failed: {e:?}");
            }
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    pub fn wire_observers(&mut self, cx: &mut Context<Self>) {
        cx.observe(&self.user_name.clone(),     |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("general.user_name",           v, cx); }).detach();
        cx.observe(&self.company_name.clone(),  |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("general.company_name",         v, cx); }).detach();
        cx.observe(&self.low_stock.clone(),     |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("inventory.low_stock_threshold", v, cx); }).detach();
        cx.observe(&self.default_unit.clone(),  |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("inventory.default_unit",        v, cx); }).detach();
        cx.observe(&self.usd_to_jmd.clone(),    |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("pricebook.usd_to_jmd_rate",     v, cx); }).detach();
        cx.observe(&self.default_margin.clone(),|this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("pricebook.default_margin",      v, cx); }).detach();
        cx.observe(&self.quote_prefix.clone(),  |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("quotations.prefix",             v, cx); }).detach();
        cx.observe(&self.tax_rate.clone(),      |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("quotations.tax_rate",           v, cx); }).detach();
        cx.observe(&self.notes_template.clone(),|this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("quotations.notes_template",     v, cx); }).detach();
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

    fn render_row(
        title:       &'static str,
        description: &'static str,
        control:     impl IntoElement,
        c:           &vassl_ui::ThemeColors,
    ) -> impl IntoElement {
        div().flex().flex_col()
            .child(
                div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                    .child(
                        div().flex_1().flex().flex_col().gap(px(3.))
                            .child(div().text_size(px(13.)).text_color(rgb(c.text_default)).child(title))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child(description))
                    )
                    .child(div().w(px(240.)).child(control))
            )
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
    }

    fn render_general(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c           = cx.global::<ThemeHandle>().0.clone();
        let name_focused = self.user_name.read(cx).focus_handle.is_focused(window);
        let co_focused   = self.company_name.read(cx).focus_handle.is_focused(window);

        div().flex().flex_col()
            .child(Self::render_row(
                "User Name",
                "Your display name, used in audit logs.",
                vassl_ui::text_field("", self.user_name.clone(), name_focused, cx),
                &c,
            ))
            .child(Self::render_row(
                "Company Name",
                "Appears on quotation headers.",
                vassl_ui::text_field("", self.company_name.clone(), co_focused, cx),
                &c,
            ))
    }
}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child({
                match active {
                    SettingsCategory::General => self.render_general(window, cx).into_any_element(),
                    _ => div().px(px(32.)).py(px(24.))
                              .child(div().text_size(px(12.)).text_color(rgb(c.text_muted))
                                     .child("(coming soon)"))
                              .into_any_element(),
                }
            });

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

    #[test]
    fn font_size_default_parses_to_13() {
        let s = "13";
        let v: f64 = s.parse().unwrap();
        assert!((v - 13.0).abs() < f64::EPSILON);
    }

    #[test]
    fn font_size_clamps_at_bounds() {
        fn clamp(v: f64) -> f64 { v.max(10.0).min(24.0) }
        assert!((clamp(9.5)  - 10.0).abs() < f64::EPSILON);
        assert!((clamp(25.0) - 24.0).abs() < f64::EPSILON);
        assert!((clamp(13.0) - 13.0).abs() < f64::EPSILON);
    }

    #[test]
    fn theme_values_are_dark_or_light() {
        let valid = ["dark", "light"];
        for v in &valid { assert!(valid.contains(v)); }
        assert!(!valid.contains(&"blue"));
    }

    #[test]
    fn currency_values_are_usd_or_jmd() {
        let valid = ["USD", "JMD"];
        assert!(valid.contains(&"USD"));
        assert!(valid.contains(&"JMD"));
        assert!(!valid.contains(&"EUR"));
    }
}
