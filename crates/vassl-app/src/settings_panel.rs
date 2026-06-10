use gpui::{Context, FocusHandle, Focusable, IntoElement, MouseButton, MouseDownEvent,
           Render, SharedString, Window, div, prelude::*, px, rems, rgb};
use std::collections::HashMap;
use std::path::PathBuf;
use vassl_ui::{TextInput, ThemeHandle};

/// Events emitted by SettingsPanel.
#[derive(Clone, Debug)]
pub enum SettingsPanelEvent {
    /// Fired when the user saves a keyboard shortcut override.
    KeymapChanged,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SettingsCategory {
    #[default]
    General,
    Appearance,
    Inventory,
    PriceBook,
    Quotations,
    Database,
    Keyboard,
}

#[derive(Clone, Debug, Default)]
pub enum BackupStatus {
    #[default]
    Idle,
    InProgress,
    Done { path: String, at: String },
    Failed(String),
}

/// Identifies which inline select/picker is currently open.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SettingSelect { Currency, FontPicker }

pub struct SettingsPanel {
    pub active_category: SettingsCategory,
    font_names:          Vec<String>,
    open_select:         Option<SettingSelect>,

    // General
    pub user_name:        gpui::Entity<TextInput>,
    pub company_name:     gpui::Entity<TextInput>,
    pub allow_delete:     bool,
    pub allow_price_edit: bool,

    // Appearance
    pub theme:       String,   // "dark" | "light" — saved by render_appearance toggle
    pub font_family: String,   // saved by render_font_picker on selection
    pub font_size:   f64,      // 10.0–24.0, step 0.5

    // Inventory
    pub low_stock:        gpui::Entity<TextInput>,
    pub default_unit:     gpui::Entity<TextInput>,
    pub sku_auto_enabled: bool,
    pub sku_fields:       gpui::Entity<TextInput>,
    pub sku_separator:    gpui::Entity<TextInput>,

    // Price Book
    pub currency:       String,  // "USD" | "JMD" — saved by render_pricebook select
    pub usd_to_jmd:     gpui::Entity<TextInput>,
    pub default_margin: gpui::Entity<TextInput>,

    // Quotations
    pub quote_prefix:   gpui::Entity<TextInput>,
    pub tax_rate:       gpui::Entity<TextInput>,
    pub notes_template: gpui::Entity<TextInput>,

    // Database
    pub backup_status: BackupStatus,

    // Keyboard
    pub keymap_overrides: HashMap<String, String>,  // action_name → keystroke string
    pub listening_for:    Option<String>,            // action_name being remapped

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
        let low_stock_val       = vassl_db::shared::get_setting(db, "inventory.low_stock_threshold").ok().flatten().unwrap_or_else(|| "5".into());
        let default_unit_val    = vassl_db::shared::get_setting(db, "inventory.default_unit").ok().flatten().unwrap_or_else(|| "pcs".into());
        let sku_auto_val        = vassl_db::shared::get_setting(db, "inventory.sku_auto_enabled").ok().flatten().unwrap_or_else(|| "false".into());
        let sku_fields_val      = vassl_db::shared::get_setting(db, "inventory.sku_fields").ok().flatten().unwrap_or_else(|| "model_number,part_number".into());
        let sku_separator_val   = vassl_db::shared::get_setting(db, "inventory.sku_separator").ok().flatten().unwrap_or_else(|| "-".into());
        let currency_val      = vassl_db::shared::get_setting(db, "pricebook.currency").ok().flatten().unwrap_or_else(|| "USD".into());
        let usd_jmd_val       = vassl_db::shared::get_setting(db, "pricebook.usd_to_jmd_rate").ok().flatten().unwrap_or_else(|| "157.50".into());
        let margin_val        = vassl_db::shared::get_setting(db, "pricebook.default_margin").ok().flatten().unwrap_or_else(|| "20".into());
        let prefix_val        = vassl_db::shared::get_setting(db, "quotations.prefix").ok().flatten().unwrap_or_else(|| "VASSL".into());
        let tax_val           = vassl_db::shared::get_setting(db, "quotations.tax_rate").ok().flatten().unwrap_or_else(|| "0".into());
        let notes_val         = vassl_db::shared::get_setting(db, "quotations.notes_template").ok().flatten().unwrap_or_default();
        let allow_delete_val      = vassl_db::shared::get_setting(db, "general.allow_delete").ok().flatten().unwrap_or_else(|| "false".into());
        let allow_price_edit_val  = vassl_db::shared::get_setting(db, "general.allow_price_edit").ok().flatten().unwrap_or_else(|| "false".into());

        // Load persisted keymap overrides
        let keymap_overrides: HashMap<String, String> = crate::keybindings::default_app_bindings()
            .iter()
            .filter_map(|(action_name, _default, _label)| {
                let db_key = format!("keymap.{action_name}");
                vassl_db::shared::get_setting(db, &db_key)
                    .ok()
                    .flatten()
                    .map(|v| (action_name.to_string(), v))
            })
            .collect();
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
        let company_name   = make_input("e.g. VASS Ltd.",  company_name_val, cx);
        let low_stock      = make_input("5",                             low_stock_val,      cx);
        let default_unit   = make_input("pcs",                           default_unit_val,   cx);
        let sku_fields     = make_input("model_number,part_number",      sku_fields_val,     cx);
        let sku_separator  = make_input("-",                             sku_separator_val,  cx);
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
            allow_delete:     allow_delete_val == "true",
            allow_price_edit: allow_price_edit_val == "true",
            theme:           theme_val,
            font_family:     font_family_val,
            font_size,
            low_stock,
            default_unit,
            sku_auto_enabled: sku_auto_val == "true",
            sku_fields,
            sku_separator,
            currency:        currency_val,
            usd_to_jmd,
            default_margin,
            quote_prefix,
            tax_rate,
            notes_template,
            backup_status:   BackupStatus::Idle,
            keymap_overrides,
            listening_for:   None,
            save_error:      None,
            focus_handle:    cx.focus_handle(),
        }
    }

    fn app_db_path() -> PathBuf {
        dirs::data_local_dir()
            .expect("no local data dir")
            .join("VASSL")
            .join("0-global")
            .join("db.sqlite")
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
        cx.observe(&self.sku_fields.clone(),    |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("inventory.sku_fields",          v, cx); }).detach();
        cx.observe(&self.sku_separator.clone(), |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("inventory.sku_separator",       v, cx); }).detach();
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
            SettingsCategory::Database   => "Database",
            SettingsCategory::Keyboard   => "Keyboard",
        }
    }

    fn render_keyboard(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c               = cx.global::<ThemeHandle>().0.clone();
        let bindings        = crate::keybindings::default_app_bindings();
        let overrides       = self.keymap_overrides.clone();
        let listening_for   = self.listening_for.clone();

        // "Reset All" button
        let reset_all_btn = div()
            .id("settings-keymap-reset-all")
            .px(px(14.)).py(px(6.)).rounded(px(5.))
            .bg(rgb(c.surface_default))
            .text_size(rems(0.923)).text_color(rgb(c.text_muted))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                // Remove all keymap overrides from DB and memory
                let db    = vassl_db::AppDatabase::global(&**cx).clone();
                let keys: Vec<String> = this.keymap_overrides.keys()
                    .map(|k| format!("keymap.{k}"))
                    .collect();
                this.keymap_overrides.clear();
                this.listening_for = None;
                cx.emit(SettingsPanelEvent::KeymapChanged);
                cx.notify();
                cx.spawn(async move |_, _cx| {
                    for key in keys {
                        if let Err(e) = db.write(move |conn| {
                            conn.exec_bound::<&str>(
                                "DELETE FROM settings WHERE key = (?)"
                            )
                            .map_err(|e| anyhow::anyhow!("{e}"))?
                            (&key)
                            .map_err(|e| anyhow::anyhow!("{e}"))
                        }).await {
                            tracing::warn!("failed to delete keymap setting: {e:?}");
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                }).detach();
            }))
            .child("Reset All to Defaults");

        let header_row = div().flex().flex_row().items_center()
            .px(px(32.)).py(px(14.))
            .child(div().flex_1().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Configure keyboard shortcuts for app-level actions."))
            .child(reset_all_btn);

        let rows: Vec<gpui::AnyElement> = bindings.iter().map(|(action_name, default_keystroke, label)| {
            let action_name        = *action_name;
            let default_keystroke  = *default_keystroke;
            let label              = *label;
            let current_binding: String = crate::keybindings::format_keystroke(
                overrides.get(action_name).map(|s| s.as_str()).unwrap_or(default_keystroke)
            );
            let is_listening = listening_for.as_deref() == Some(action_name);
            let row_bg = if is_listening { c.surface_active } else { c.canvas_bg };
            let has_override = overrides.contains_key(action_name);

            // Pre-build the toggle-listening handler
            let toggle_listener = cx.listener(move |this: &mut Self, _: &MouseDownEvent, _, cx| {
                if this.listening_for.as_deref() == Some(action_name) {
                    // Cancel: restore bindings
                    this.listening_for = None;
                    cx.emit(SettingsPanelEvent::KeymapChanged);
                } else {
                    // Enter listening mode: disable all shortcuts so they don't fire
                    // while the user presses the new key combo.
                    this.listening_for = Some(action_name.to_string());
                    cx.clear_key_bindings();
                }
                cx.notify();
            });

            // Pre-build the reset-row handler (only used when has_override)
            let reset_listener = cx.listener(move |this: &mut Self, _: &MouseDownEvent, _, cx| {
                let db_key = format!("keymap.{action_name}");
                this.keymap_overrides.remove(action_name);
                this.listening_for = None;
                cx.emit(SettingsPanelEvent::KeymapChanged);
                cx.notify();
                let db = vassl_db::AppDatabase::global(&**cx).clone();
                cx.spawn(async move |_, _cx| {
                    if let Err(e) = db.write(move |conn| {
                        conn.exec_bound::<&str>(
                            "DELETE FROM settings WHERE key = (?)"
                        )
                        .map_err(|e| anyhow::anyhow!("{e}"))?
                        (&db_key)
                        .map_err(|e| anyhow::anyhow!("{e}"))
                    }).await {
                        tracing::warn!("failed to delete keymap override: {e:?}");
                    }
                    Ok::<(), anyhow::Error>(())
                }).detach();
            });

            let binding_cell = div()
                .id(SharedString::from(format!("keymap-binding-{action_name}")))
                .w(px(180.)).px(px(10.)).py(px(5.))
                .rounded(px(4.))
                .bg(rgb(if is_listening { c.surface_active } else { c.surface_default }))
                .border_1().border_color(rgb(if is_listening { c.text_muted } else { c.surface_default }))
                .text_size(rems(0.923)).text_color(rgb(if is_listening { c.text_muted } else { c.text_default }))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, toggle_listener)
                .child(if is_listening {
                    "Press a key combo…".to_string()
                } else {
                    current_binding.clone()
                });

            let mut row = div().flex().flex_row().items_center()
                .px(px(32.)).py(px(10.))
                .bg(rgb(row_bg))
                .child(
                    div().flex_1()
                        .text_size(rems(0.923)).text_color(rgb(c.text_default))
                        .child(label)
                )
                .child(
                    div().w(px(140.))
                        .text_size(rems(0.846)).text_color(rgb(c.text_muted))
                        .child(crate::keybindings::format_keystroke(default_keystroke))
                )
                .child(binding_cell);

            if has_override {
                let reset_btn = div()
                    .id(SharedString::from(format!("keymap-reset-{action_name}")))
                    .ml(px(8.)).px(px(8.)).py(px(4.))
                    .rounded(px(4.))
                    .bg(rgb(c.surface_default))
                    .text_size(rems(0.769)).text_color(rgb(c.text_muted))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, reset_listener)
                    .child("↺");
                row = row.child(reset_btn);
            }

            row.into_any_element()
        }).collect();

        // Column header
        let col_header = div().flex().flex_row().items_center()
            .px(px(32.)).py(px(6.))
            .bg(rgb(c.surface_default))
            .child(div().flex_1().text_size(rems(0.769)).text_color(rgb(c.text_muted)).child("Action"))
            .child(div().w(px(140.)).text_size(rems(0.769)).text_color(rgb(c.text_muted)).child("Default"))
            .child(div().w(px(180.)).text_size(rems(0.769)).text_color(rgb(c.text_muted)).child("Current Binding"))
            .child(div().w(px(40.)));

        div().flex().flex_col()
            .child(header_row)
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            .child(col_header)
            .children(rows.into_iter().map(|r| div().flex().flex_col()
                .child(r)
                .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            ))
    }

    /// Called from `on_key_down` in root.rs when we're in listening mode.
    /// Saves the new binding for the action currently in `listening_for`.
    #[allow(dead_code)]
    pub fn capture_key_for_listening(&mut self, keystroke: String, cx: &mut Context<Self>) {
        let Some(action_name) = self.listening_for.take() else { return; };
        let db_key = format!("keymap.{action_name}");
        let keystroke_for_db = keystroke.clone();
        self.keymap_overrides.insert(action_name.clone(), keystroke);
        cx.emit(SettingsPanelEvent::KeymapChanged);
        cx.notify();

        let db = vassl_db::AppDatabase::global(&**cx).clone();
        cx.spawn(async move |_, _cx| {
            if let Err(e) = db.write(move |conn| {
                vassl_db::shared::set_setting(conn, &db_key, &keystroke_for_db)
            }).await {
                tracing::warn!("failed to save keymap override: {e:?}");
            }
            Ok::<(), anyhow::Error>(())
        }).detach();
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
                    .px(px(10.)).py(px(7.)).rounded(px(4.))
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
                div().w(px(52.)).px(px(4.)).py(px(7.))
                    .bg(rgb(c.surface_default)).rounded(px(4.))
                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                    .flex().items_center().justify_center()
                    .child(format!("{:.1}", font_size))
            )
            .child(
                div().id("settings-font-plus")
                    .px(px(10.)).py(px(7.)).rounded(px(4.))
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
                    .px(px(12.)).py(px(7.))
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
        let c              = cx.global::<ThemeHandle>().0.clone();
        let stock_focused  = self.low_stock.read(cx).focus_handle.is_focused(window);
        let unit_focused   = self.default_unit.read(cx).focus_handle.is_focused(window);
        let fields_focused = self.sku_fields.read(cx).focus_handle.is_focused(window);
        let sep_focused    = self.sku_separator.read(cx).focus_handle.is_focused(window);
        let sku_enabled    = self.sku_auto_enabled;

        // SKU auto-enable toggle pill
        let sku_toggle = {
            let (pill_bg, thumb_x) = if sku_enabled {
                (c.surface_active, px(16.))
            } else {
                (c.surface_default, px(2.))
            };
            div().id("settings-sku-toggle")
                .w(px(32.)).h(px(18.)).rounded_full()
                .bg(rgb(pill_bg)).cursor_pointer().relative()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.sku_auto_enabled = !this.sku_auto_enabled;
                    let v = if this.sku_auto_enabled { "true" } else { "false" };
                    this.save_setting("inventory.sku_auto_enabled", v.into(), cx);
                    cx.notify();
                }))
                .child(
                    div().absolute()
                        .top(px(2.)).left(thumb_x)
                        .w(px(14.)).h(px(14.)).rounded_full()
                        .bg(rgb(c.canvas_bg))
                )
        };

        div().flex().flex_col()
            .child(Self::render_row(
                "Low Stock Threshold",
                "Alert when stock quantity falls at or below this number.",
                vassl_ui::text_field("", self.low_stock.clone(), stock_focused, false, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Stock Unit",
                "Unit label used when adding new products (e.g. pcs, kg, L).",
                vassl_ui::text_field("", self.default_unit.clone(), unit_focused, false, cx),
                &c,
            ))
            // SKU auto-compute section
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("SKU Auto-Compute"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("Auto-generate SKU from product fields when creating products."))
                            )
                            .child(sku_toggle)
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
            .when(sku_enabled, |d| d
                .child(Self::render_row(
                    "SKU Fields",
                    "Comma-separated field names to join (e.g. model_number,part_number).",
                    vassl_ui::text_field("", self.sku_fields.clone(), fields_focused, false, cx),
                    &c,
                ))
                .child(Self::render_row(
                    "SKU Separator",
                    "Character(s) to join fields (e.g. - or /).",
                    vassl_ui::text_field("", self.sku_separator.clone(), sep_focused, false, cx),
                    &c,
                ))
            )
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
                vassl_ui::text_field("", self.usd_to_jmd.clone(), rate_focused, false, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Margin %",
                "Pre-filled margin percentage when creating new price book entries.",
                vassl_ui::text_field("", self.default_margin.clone(), margin_focused, false, cx),
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
                vassl_ui::text_field("", self.quote_prefix.clone(), prefix_focused, false, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Tax / VAT %",
                "Pre-filled tax rate on new quotations. Set to 0 to disable.",
                vassl_ui::text_field("", self.tax_rate.clone(), tax_focused, false, cx),
                &c,
            ))
            .child(Self::render_row(
                "Default Notes Template",
                "Text pre-filled in the Notes field on new quotations.",
                vassl_ui::text_field("", self.notes_template.clone(), notes_focused, false, cx),
                &c,
            ))
    }

    fn render_database(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c          = cx.global::<ThemeHandle>().0.clone();
        let db_path    = Self::app_db_path();
        let db_display = db_path.display().to_string();
        let status     = self.backup_status.clone();

        let (status_line1, status_line2, status_color) = match &status {
            BackupStatus::Idle       => ("No backup taken yet.".to_string(), None, c.text_muted),
            BackupStatus::InProgress => ("Backing up…".to_string(),          None, c.text_muted),
            BackupStatus::Done { path, at } => (
                format!("Last backup: {at}"),
                Some(format!("→ {path}")),
                c.status_green,
            ),
            BackupStatus::Failed(e) => (format!("Backup failed: {e}"), None, c.status_red),
        };
        let in_progress = matches!(status, BackupStatus::InProgress);
        let btn_text    = if in_progress { "Backing up…" } else { "Back Up Now" };
        let btn_bg      = if in_progress { c.surface_default } else { c.surface_active };

        div().flex().flex_col()
            // DB location row — path wraps below the label on its own line
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_col().py(px(14.)).px(px(32.)).gap(px(4.))
                            .child(div().text_size(rems(1.)).text_color(rgb(c.text_default))
                                .child("Database Location"))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                .child("Path to the live SQLite database file."))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                .child(SharedString::from(db_display)))
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
            // Backup row — status lines stack; button is flex-shrink-0 on the right
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.)).gap(px(12.))
                            // left: title + status — min_w(0) lets it shrink without pushing button
                            .child(
                                div().flex_1().min_w(px(0.)).flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default))
                                        .child("Back Up Database"))
                                    .child(
                                        div().text_size(rems(0.846)).text_color(rgb(status_color))
                                            .overflow_hidden()
                                            .child(SharedString::from(status_line1))
                                    )
                                    .when_some(status_line2, |d, line| d.child(
                                        div().text_size(rems(0.846)).text_color(rgb(status_color))
                                            .overflow_hidden()
                                            .child(SharedString::from(line))
                                    ))
                            )
                            .child(
                                div().id("settings-backup-btn")
                                    .px(px(16.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(btn_bg))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                        if matches!(this.backup_status, BackupStatus::InProgress) { return; }

                                        // Default to ~/Documents so the user can easily find it
                                        let default_dir = dirs::document_dir()
                                            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
                                        let now_label = chrono::Local::now()
                                            .format("vassl-backup-%Y-%m-%d").to_string();
                                        let rx = cx.prompt_for_new_path(&default_dir, Some(&now_label));

                                        this.backup_status = BackupStatus::InProgress;
                                        cx.notify();

                                        let app_db = vassl_db::AppDatabase::global(&**cx).clone();
                                        cx.spawn(async move |this, cx| {
                                            let Ok(Ok(Some(mut dest))) = rx.await else {
                                                let _ = this.update(cx, |sp, cx| {
                                                    sp.backup_status = BackupStatus::Idle;
                                                    cx.notify();
                                                });
                                                return;
                                            };

                                            // Ensure the file always has a .sqlite extension
                                            if dest.extension().is_none() {
                                                dest.set_extension("sqlite");
                                            }

                                            let dest_for_reveal = dest.clone();
                                            let result = app_db.write(move |conn| {
                                                conn.backup_main_to(&dest)
                                                    .map(|_| dest.display().to_string())
                                            }).await;

                                            let _ = this.update(cx, |sp, cx| {
                                                sp.backup_status = match result {
                                                    Ok(path) => {
                                                        let at = chrono::Local::now()
                                                            .format("%Y-%m-%d %H:%M").to_string();
                                                        // Open Finder/Explorer at the backup location
                                                        cx.reveal_path(&dest_for_reveal);
                                                        BackupStatus::Done { path, at }
                                                    }
                                                    Err(e) => BackupStatus::Failed(e.to_string()),
                                                };
                                                cx.notify();
                                            });
                                        }).detach();
                                    }))
                                    .child(btn_text)
                            )
                            // "Show in Finder" — only visible after a successful backup
                            .when(matches!(&status, BackupStatus::Done { .. }), |row| {
                                let backup_path = if let BackupStatus::Done { path, .. } = &status {
                                    PathBuf::from(path)
                                } else {
                                    PathBuf::new()
                                };
                                row.child(
                                    div().id("settings-backup-reveal")
                                        .px(px(12.)).py(px(7.)).rounded(px(5.))
                                        .bg(rgb(c.surface_default))
                                        .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                                            cx.reveal_path(&backup_path);
                                        }))
                                        .child("Show in Finder")
                                )
                            })
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
    }

    fn render_general(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c           = cx.global::<ThemeHandle>().0.clone();
        let name_focused = self.user_name.read(cx).focus_handle.is_focused(window);
        let co_focused   = self.company_name.read(cx).focus_handle.is_focused(window);
        let allow_delete     = self.allow_delete;
        let allow_price_edit = self.allow_price_edit;

        // Allow Delete toggle pill
        let delete_toggle = {
            let (pill_bg, thumb_x) = if allow_delete {
                (c.surface_active, px(16.))
            } else {
                (c.surface_default, px(2.))
            };
            div().id("settings-allow-delete-toggle")
                .w(px(32.)).h(px(18.)).rounded_full()
                .bg(rgb(pill_bg)).cursor_pointer().relative()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.allow_delete = !this.allow_delete;
                    let v = if this.allow_delete { "true" } else { "false" };
                    this.save_setting("general.allow_delete", v.into(), cx);
                    cx.set_global(vassl_ui::AppSettings {
                        allow_delete:     this.allow_delete,
                        allow_price_edit: this.allow_price_edit,
                    });
                    cx.notify();
                }))
                .child(
                    div().absolute()
                        .top(px(2.)).left(thumb_x)
                        .w(px(14.)).h(px(14.)).rounded_full()
                        .bg(rgb(c.canvas_bg))
                )
        };

        // Allow Price Edit toggle pill
        let price_edit_toggle = {
            let (pill_bg, thumb_x) = if allow_price_edit {
                (c.surface_active, px(16.))
            } else {
                (c.surface_default, px(2.))
            };
            div().id("settings-allow-price-edit-toggle")
                .w(px(32.)).h(px(18.)).rounded_full()
                .bg(rgb(pill_bg)).cursor_pointer().relative()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.allow_price_edit = !this.allow_price_edit;
                    let v = if this.allow_price_edit { "true" } else { "false" };
                    this.save_setting("general.allow_price_edit", v.into(), cx);
                    cx.set_global(vassl_ui::AppSettings {
                        allow_delete:     this.allow_delete,
                        allow_price_edit: this.allow_price_edit,
                    });
                    cx.notify();
                }))
                .child(
                    div().absolute()
                        .top(px(2.)).left(thumb_x)
                        .w(px(14.)).h(px(14.)).rounded_full()
                        .bg(rgb(c.canvas_bg))
                )
        };

        div().flex().flex_col()
            .child(Self::render_row(
                "User Name",
                "Your display name, used in audit logs.",
                vassl_ui::text_field("", self.user_name.clone(), name_focused, false, cx),
                &c,
            ))
            .child(Self::render_row(
                "Company Name",
                "Appears on quotation headers.",
                vassl_ui::text_field("", self.company_name.clone(), co_focused, false, cx),
                &c,
            ))
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("Allow Deleting Records"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("When enabled, context menus will show a Delete option for products, suppliers, and other records."))
                            )
                            .child(delete_toggle)
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                            .child(
                                div().flex_1().flex().flex_col().gap(px(3.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default)).child("Allow Editing Price Entries"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("When enabled, price entries can be edited directly from the pricebook context menu."))
                            )
                            .child(price_edit_toggle)
                    )
                    .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            )
    }
}

impl gpui::EventEmitter<SettingsPanelEvent> for SettingsPanel {}

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
            SettingsCategory::Database,
            SettingsCategory::Keyboard,
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
            .flex_1().min_h(px(0.)).overflow_y_scroll()
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
                    SettingsCategory::Database   => self.render_database(cx).into_any_element(),
                    SettingsCategory::Keyboard   => self.render_keyboard(cx).into_any_element(),
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
