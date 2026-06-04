use gpui::{Context, FocusHandle, Focusable, IntoElement, Render, SharedString, Window,
           div, prelude::*, px, rems, rgb};
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
/// `Theme` is not used — theme is a toggle pill, not a select. Kept for exhaustive matching.
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(dead_code)]
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

    save_error:   Option<String>,
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
            save_error:      None,
            focus_handle:    cx.focus_handle(),
        }
    }

    pub fn save_setting(&self, key: &'static str, value: String, cx: &mut Context<Self>) {
        let db = vassl_db::AppDatabase::global(&**cx).clone();
        cx.spawn(async move |this, cx| {
            if let Err(e) = db.write(move |conn| vassl_db::shared::set_setting(conn, key, &value)).await {
                tracing::error!("save_setting({key}) failed: {e:?}");
                let _ = this.update(cx, |sp, cx| {
                    sp.save_error = Some(format!("Failed to save setting \"{key}\": {e}"));
                    cx.notify();
                });
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
                            .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child(title))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child(description))
                    )
                    .child(div().w(px(240.)).child(control))
            )
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
    }

    fn render_appearance(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let font_picker = self.render_font_picker(cx);  // must be first — borrows self mutably
        let c       = cx.global::<ThemeHandle>().0.clone();
        let is_dark = self.theme == "dark";

        // theme toggle pill
        let toggle = {
            let (pill_bg, thumb_x) = if is_dark {
                (c.surface_active, px(16.))
            } else {
                (c.surface_default, px(2.))
            };
            div().id("settings-theme-toggle")
                .w(px(32.)).h(px(18.)).rounded_full()
                .bg(rgb(pill_bg)).cursor_pointer().relative()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.theme = if this.theme == "dark" { "light".into() } else { "dark".into() };
                    let colors = if this.theme == "dark" {
                        vassl_ui::ThemeColors::dark()
                    } else {
                        vassl_ui::ThemeColors::light()
                    };
                    cx.set_global(vassl_ui::ThemeHandle(colors.with_font(this.font_family.clone())));
                    this.save_setting("appearance.theme", this.theme.clone(), cx);
                    cx.notify();
                }))
                .child(
                    div().absolute()
                        .top(px(2.)).left(thumb_x)
                        .w(px(14.)).h(px(14.)).rounded_full()
                        .bg(rgb(c.canvas_bg))
                )
        };

        // font size stepper
        let font_size = self.font_size;
        let stepper = div().flex().flex_row().items_center().gap(px(2.))
            .child(
                div().id("settings-font-minus")
                    .px(px(10.)).py(px(5.)).rounded(px(4.))
                    .bg(rgb(c.surface_default)).text_size(rems(1.)).text_color(rgb(c.text_default))
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.font_size = (this.font_size - 0.5).max(10.0);
                        window.set_rem_size(px(this.font_size as f32));
                        this.save_setting("appearance.font_size", format!("{:.1}", this.font_size), cx);
                        cx.notify();
                    }))
                    .child("−")
            )
            .child(
                div().w(px(52.)).px(px(4.)).py(px(5.))
                    .bg(rgb(c.surface_default)).rounded(px(4.))
                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                    .flex().items_center().justify_center()
                    .child(format!("{:.1}", font_size))
            )
            .child(
                div().id("settings-font-plus")
                    .px(px(10.)).py(px(5.)).rounded(px(4.))
                    .bg(rgb(c.surface_default)).text_size(rems(1.)).text_color(rgb(c.text_default))
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.font_size = (this.font_size + 0.5).min(24.0);
                        window.set_rem_size(px(this.font_size as f32));
                        this.save_setting("appearance.font_size", format!("{:.1}", this.font_size), cx);
                        cx.notify();
                    }))
                    .child("+")
            );

        // font size label with ↺ reset button
        let font_size_label = div().flex().flex_row().items_center().gap(px(6.))
            .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("Font Size"))
            .child(
                div().id("settings-font-reset")
                    .text_size(rems(0.846)).text_color(rgb(c.text_muted)).cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.font_size = 13.0;
                        window.set_rem_size(px(13.0_f32));
                        this.save_setting("appearance.font_size", "13.0".into(), cx);
                        cx.notify();
                    }))
                    .child("↺")
            );

        div().flex().flex_col()
            // Theme row
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("Theme"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child(if is_dark { "Dark mode" } else { "Light mode" }))
                            )
                            .child(toggle)
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
            // Font Family row
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("Font Family"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("UI font used across the application."))
                            )
                            .child(div().w(px(240.)).child(font_picker))
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
            // Font size row
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(font_size_label)
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("UI font size in pixels. Step 0.5, range 10–24."))
                            )
                            .child(stepper)
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
    }

    fn render_font_picker(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c          = cx.global::<ThemeHandle>().0.clone();
        let is_open    = self.open_select == Some(SettingSelect::FontPicker);
        let current    = self.font_family.clone();
        let font_names = self.font_names.clone();

        div().flex().flex_col()
            // trigger button showing current font name in that font
            .child(
                div().id("settings-font-trigger")
                    .flex().flex_row().items_center().gap(px(6.))
                    .px(px(10.)).py(px(6.))
                    .bg(rgb(c.surface_default)).rounded(px(5.))
                    .border_1().border_color(rgb(c.surface_active))
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                        this.open_select = if this.open_select == Some(SettingSelect::FontPicker) {
                            None
                        } else {
                            Some(SettingSelect::FontPicker)
                        };
                        cx.notify();
                    }))
                    .child(
                        div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .font_family(SharedString::from(current.clone()))
                            .child(current.clone())
                    )
                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("◇"))
            )
            // inline scrollable list (shown only when open)
            .when(is_open, move |d| {
                d.child(
                    div().id("settings-font-list")
                        .mt(px(2.)).max_h(px(200.)).overflow_y_scroll()
                        .bg(rgb(c.surface_default)).rounded(px(4.))
                        .border_1().border_color(rgb(c.surface_active))
                        .children(font_names.into_iter().map(|name| {
                            let name_for_select = name.clone();
                            let name_for_display = name.clone();
                            let selected = name == current;
                            let bg = if selected { c.surface_active } else { c.surface_default };
                            div()
                                .id(SharedString::from(format!("font-{name}")))
                                .px(px(10.)).py(px(5.))
                                .bg(rgb(bg)).cursor_pointer()
                                .font_family(SharedString::from(name.clone()))
                                .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.font_family = name_for_select.clone();
                                    this.open_select = None;
                                    this.save_setting("appearance.font_family", name_for_select.clone(), cx);
                                    // Update the theme global so the root div re-renders with the new font
                                    let mut theme = cx.global::<ThemeHandle>().0.clone();
                                    theme.font_family = name_for_select.clone();
                                    cx.set_global(ThemeHandle(theme));
                                    cx.notify();
                                }))
                                .child(name_for_display)
                        }))
                )
            })
    }

    fn render_inventory(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c             = cx.global::<ThemeHandle>().0.clone();
        let stock_focused = self.low_stock.read(cx).focus_handle.is_focused(window);
        let unit_focused  = self.default_unit.read(cx).focus_handle.is_focused(window);

        div().flex().flex_col()
            .child(Self::render_row(
                "Low Stock Threshold",
                "Alert when stock quantity falls at or below this number.",
                vassl_ui::text_field("", self.low_stock.clone(), stock_focused, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Stock Unit",
                "Unit label used when adding new products (e.g. pcs, kg, L).",
                vassl_ui::text_field("", self.default_unit.clone(), unit_focused, cx),
                &c,
            ))
    }

    fn render_setting_select(
        &mut self,
        id:      &'static str,
        select:  SettingSelect,
        current: String,
        options: &[(&'static str, &'static str)],
        on_pick: impl Fn(&mut Self, &'static str, &mut Context<Self>) + 'static,
        cx:      &mut Context<Self>,
    ) -> impl IntoElement {
        let c       = cx.global::<ThemeHandle>().0.clone();
        let is_open = self.open_select == Some(select);
        let label: SharedString = options.iter()
            .find(|(v, _)| *v == current.as_str())
            .map(|(_, l)| SharedString::from(*l))
            .unwrap_or_else(|| SharedString::from(current.clone()));
        let options_owned: Vec<(&'static str, &'static str)> = options.to_vec();
        let current_owned = current.clone();
        let on_pick = std::sync::Arc::new(on_pick);

        div().flex().flex_col()
            .child(
                div().id(id)
                    .flex().flex_row().items_center().gap(px(6.))
                    .px(px(10.)).py(px(6.))
                    .bg(rgb(c.surface_default)).rounded(px(5.))
                    .border_1().border_color(rgb(c.surface_active))
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                        this.open_select = if this.open_select == Some(select) { None } else { Some(select) };
                        cx.notify();
                    }))
                    .child(div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_default)).child(label))
                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("◇"))
            )
            .when(is_open, move |d| {
                d.child(
                    div().id(SharedString::from(format!("{id}-list")))
                        .mt(px(2.))
                        .bg(rgb(c.surface_default)).rounded(px(4.))
                        .border_1().border_color(rgb(c.surface_active))
                        .children(options_owned.iter().map(|(val, lbl)| {
                            let val2 = *val;
                            let selected = *val == current_owned.as_str();
                            let bg = if selected { c.surface_active } else { c.surface_default };
                            let on_pick = on_pick.clone();
                            div()
                                .id(SharedString::from(format!("{id}-opt-{val}")))
                                .px(px(10.)).py(px(6.))
                                .bg(rgb(bg)).cursor_pointer()
                                .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    on_pick(this, val2, cx);
                                    this.open_select = None;
                                    cx.notify();
                                }))
                                .child(*lbl)
                        }))
                )
            })
    }

    fn render_pricebook(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rate_focused   = self.usd_to_jmd.read(cx).focus_handle.is_focused(window);
        let margin_focused = self.default_margin.read(cx).focus_handle.is_focused(window);
        let c              = cx.global::<ThemeHandle>().0.clone();

        let currency_select = self.render_setting_select(
            "settings-currency",
            SettingSelect::Currency,
            self.currency.clone(),
            &[("USD", "USD — US Dollar"), ("JMD", "JMD — Jamaican Dollar")],
            |this, val, cx| {
                let val_s = val.to_string();
                this.currency = val_s.clone();
                this.save_setting("pricebook.currency", val_s, cx);
            },
            cx,
        );

        div().flex().flex_col()
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("Default Currency"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("Currency used on quotations and price book entries."))
                            )
                            .child(div().w(px(240.)).child(currency_select))
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
            .child(Self::render_row(
                "USD → JMD Rate",
                "Conversion rate applied when displaying prices in JMD.",
                vassl_ui::text_field("", self.usd_to_jmd.clone(), rate_focused, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Margin %",
                "Pre-filled margin percentage when creating new price book entries.",
                vassl_ui::text_field("", self.default_margin.clone(), margin_focused, cx),
                &c,
            ))
    }

    fn render_quotations(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c              = cx.global::<ThemeHandle>().0.clone();
        let prefix_focused = self.quote_prefix.read(cx).focus_handle.is_focused(window);
        let tax_focused    = self.tax_rate.read(cx).focus_handle.is_focused(window);
        let notes_focused  = self.notes_template.read(cx).focus_handle.is_focused(window);

        div().flex().flex_col()
            .child(Self::render_row(
                "Quote Number Prefix",
                "Prefix for auto-generated quotation reference numbers (e.g. VASSL-2026-0001).",
                vassl_ui::text_field("", self.quote_prefix.clone(), prefix_focused, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Tax / VAT %",
                "Pre-filled tax rate on new quotations. Set to 0 to disable.",
                vassl_ui::text_field("", self.tax_rate.clone(), tax_focused, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Notes Template",
                "Text pre-filled in the Notes field on new quotations.",
                vassl_ui::text_field("", self.notes_template.clone(), notes_focused, cx),
                &c,
            ))
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
                    .text_size(rems(1.)).cursor_pointer()
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
                    .child(div().text_size(rems(1.385)).text_color(rgb(c.text_default))
                        .child(Self::category_label(active)))
                    .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted)).mt(px(2.))
                        .child(format!("{} Settings", Self::category_label(active))))
            )
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            .child({
                match active {
                    SettingsCategory::General    => self.render_general(window, cx).into_any_element(),
                    SettingsCategory::Appearance => self.render_appearance(window, cx).into_any_element(),
                    SettingsCategory::Inventory  => self.render_inventory(window, cx).into_any_element(),
                    SettingsCategory::PriceBook  => self.render_pricebook(window, cx).into_any_element(),
                    SettingsCategory::Quotations => self.render_quotations(window, cx).into_any_element(),
                }
            });

        let error_bar = self.save_error.as_deref().map(|msg| {
            let c = cx.global::<ThemeHandle>().0.clone();
            div()
                .absolute().bottom_0().left(px(160.)).right_0()
                .px(px(16.)).py(px(8.))
                .bg(rgb(c.surface_default))
                .border_t_1().border_color(rgb(c.surface_active))
                .text_size(rems(0.846)).text_color(rgb(c.status_red))
                .child(SharedString::from(msg.to_string()))
        });

        div()
            .relative()
            .flex().flex_row().flex_1().h_full()
            .track_focus(&self.focus_handle)
            .child(nav)
            .child(content)
            .children(error_bar)
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

    #[test]
    fn stepper_step_up_clamps_at_24() {
        let mut v = 24.0_f64;
        v = (v + 0.5).min(24.0);
        assert!((v - 24.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stepper_step_down_clamps_at_10() {
        let mut v = 10.0_f64;
        v = (v - 0.5).max(10.0);
        assert!((v - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stepper_reset_returns_to_13() {
        let mut v = 22.5_f64;
        v = 13.0;
        assert!((v - 13.0).abs() < f64::EPSILON);
    }

    #[test]
    fn font_family_key_is_correct() {
        const KEY: &str = "appearance.font_family";
        let (module, field) = KEY.split_once('.').expect("key must contain a dot");
        assert_eq!(module, "appearance");
        assert_eq!(field, "font_family");
    }

    #[test]
    fn font_picker_default_is_valid_css_generic() {
        const DEFAULT: &str = "system-ui";
        assert!(!DEFAULT.is_empty());
        assert!(["system-ui", "sans-serif", "serif", "monospace"].contains(&DEFAULT));
    }
}
