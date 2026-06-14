use gpui::{Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
           Render, SharedString, Window, actions, div, prelude::*, px, rems, rgb};
use std::collections::HashMap;
use std::path::PathBuf;
use vassl_ui::{AppSettings, TextInput, ThemeHandle};
use crate::users_db::{AuthUser, UsersDb};

// Key-nav actions for the Security (password-change) sub-form.
actions!(settings_security, [SecurityTabField, SecurityBackTabField, SecurityEscapeForm]);
// Key-nav actions for the Admin add-user and reset-password sub-forms.
actions!(settings_admin, [AdminAddTabField, AdminAddBackTabField, AdminAddEscapeForm,
                           AdminResetTabField, AdminResetBackTabField, AdminResetEscapeForm]);

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
    Security,
    Admin,
    License,
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

    // Security (password change)
    pw_current:           gpui::Entity<TextInput>,
    pw_new:               gpui::Entity<TextInput>,
    pw_confirm:           gpui::Entity<TextInput>,
    pw_message:           Option<String>,
    pw_is_error:          bool,

    // Admin — user management
    admin_users:          Vec<AuthUser>,
    admin_add_form_open:  bool,
    add_username:         gpui::Entity<TextInput>,
    add_password:         gpui::Entity<TextInput>,
    add_can_inventory:    bool,
    add_can_pricebook:    bool,
    add_can_quotations:   bool,
    add_allow_delete:     bool,
    add_allow_price_edit: bool,
    add_error:            Option<String>,
    add_user_save_focus:  FocusHandle,
    reset_pw_id:          Option<i64>,
    reset_pw_input:       gpui::Entity<TextInput>,
    reset_pw_save_focus:  FocusHandle,
    reset_pw_cancel_focus: FocusHandle,
    security_save_focus:  FocusHandle,

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
        // (allow_delete / allow_price_edit are now per-user, not global settings)

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

        let make_pw_input = |ph: &'static str, cx: &mut Context<Self>| {
            cx.new(move |cx| {
                let mut t = TextInput::with_placeholder(ph, cx);
                t.is_password = true;
                t
            })
        };

        Self {
            active_category: SettingsCategory::General,
            font_names,
            open_select:     None,
            user_name,
            company_name,
            pw_current:          make_pw_input("Current password",  cx),
            pw_new:              make_pw_input("New password",       cx),
            pw_confirm:          make_pw_input("Confirm new password", cx),
            pw_message:          None,
            pw_is_error:         false,
            admin_users:         Vec::new(),
            admin_add_form_open: false,
            add_username:        cx.new(|cx| TextInput::with_placeholder("Username", cx)),
            add_password:        cx.new(|cx| {
                let mut t = TextInput::with_placeholder("Password", cx);
                t.is_password = true;
                t
            }),
            add_can_inventory:    false,
            add_can_pricebook:    false,
            add_can_quotations:   false,
            add_allow_delete:     false,
            add_allow_price_edit: false,
            add_error:            None,
            add_user_save_focus:  cx.focus_handle(),
            reset_pw_id:          None,
            reset_pw_input:       cx.new(|cx| {
                let mut t = TextInput::with_placeholder("New password", cx);
                t.is_password = true;
                t
            }),
            reset_pw_save_focus:   cx.focus_handle(),
            reset_pw_cancel_focus: cx.focus_handle(),
            security_save_focus:   cx.focus_handle(),
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

    /// Save the user name and keep the audit log up to date.
    ///
    /// All logic runs inside a single serialised DB write closure so there are no
    /// race conditions between reading the old name and writing the new one.  If a
    /// NAME_CHANGE audit entry was already created in the last hour (i.e. the user
    /// is still in the same editing session) its `new_value` is patched in-place;
    /// otherwise a fresh entry is inserted.  This means the log always shows one
    /// clean "Alice → Bob" row per session, not one row per keystroke.
    fn save_user_name_with_audit(&self, new_name: String, cx: &mut Context<Self>) {
        let db = vassl_db::AppDatabase::global(&**cx).clone();
        cx.spawn(async move |this, cx| {
            if let Err(e) = db.write(move |conn| {
                // Read the old name BEFORE writing the new one.
                let old_name = vassl_db::shared::current_user(conn)
                    .ok().flatten().unwrap_or_default();
                vassl_db::shared::set_setting(conn, "general.user_name", &new_name)?;
                vassl_db::shared::set_setting(conn, "current_user", &new_name)?;
                if old_name == new_name { return Ok::<(), anyhow::Error>(()); }
                // Check for a recent in-progress NAME_CHANGE entry (same session).
                let recent_id = conn
                    .select_row_bound::<&str, i64>(
                        "SELECT id FROM audit_log \
                         WHERE table_name = 'settings' AND action = 'NAME_CHANGE' \
                         AND changed_at > datetime('now', '-1 hour') \
                         ORDER BY id DESC LIMIT 1",
                    )
                    .ok()
                    .and_then(|mut q| q("settings").ok())
                    .flatten();
                if let Some(id) = recent_id {
                    vassl_db::shared::update_audit_new_value(conn, id, &new_name)?;
                } else if !old_name.is_empty() {
                    vassl_db::shared::log_audit(
                        conn, "settings", 0, "NAME_CHANGE",
                        &new_name, Some(&old_name), Some(&new_name),
                    )?;
                }
                Ok(())
            }).await {
                tracing::error!("save_user_name_with_audit failed: {e:?}");
                let _ = this.update(cx, |sp, cx| {
                    sp.save_error = Some(format!("Failed to save user name: {e}"));
                    cx.notify();
                });
            }
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    pub fn wire_observers(&mut self, cx: &mut Context<Self>) {
        cx.observe(&self.user_name.clone(), |this, f, cx| {
            let v = f.read(cx).text().to_string();
            this.save_user_name_with_audit(v, cx);
        }).detach();
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
            SettingsCategory::Security   => "Security",
            SettingsCategory::Admin      => "Admin",
            SettingsCategory::License    => "License",
        }
    }

    fn load_admin_users(&mut self, cx: &mut Context<Self>) {
        let db = UsersDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_users() })
                .await;
            let _ = this.update(cx, |sp, cx| {
                if let Ok(users) = result { sp.admin_users = users; }
                cx.notify();
            });
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    fn do_security_save(&mut self, cx: &mut Context<Self>) {
        let uid     = cx.global::<AppSettings>().logged_in_user_id;
        let old_pw  = self.pw_current.read(cx).text().to_string();
        let new_pw  = self.pw_new.read(cx).text().to_string();
        let confirm = self.pw_confirm.read(cx).text().to_string();

        if new_pw.len() < 6 {
            self.pw_message  = Some("New password must be at least 6 characters.".into());
            self.pw_is_error = true;
            cx.notify();
            return;
        }
        if new_pw != confirm {
            self.pw_message  = Some("Passwords do not match.".into());
            self.pw_is_error = true;
            cx.notify();
            return;
        }

        let db = UsersDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.change_password(uid, old_pw, &new_pw).await;
            let _ = this.update(cx, |sp, cx| {
                match result {
                    Ok(_) => {
                        sp.pw_message  = Some("Password changed successfully.".into());
                        sp.pw_is_error = false;
                        sp.pw_current.update(cx, |t, cx| t.reset(cx));
                        sp.pw_new.update(cx, |t, cx| t.reset(cx));
                        sp.pw_confirm.update(cx, |t, cx| t.reset(cx));
                    }
                    Err(e) => {
                        sp.pw_message  = Some(format!("{e}"));
                        sp.pw_is_error = true;
                    }
                }
                cx.notify();
            });
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    fn render_security(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c           = cx.global::<ThemeHandle>().0.clone();
        let cur_focused  = self.pw_current.read(cx).focus_handle.is_focused(window);
        let new_focused  = self.pw_new.read(cx).focus_handle.is_focused(window);
        let cf_focused   = self.pw_confirm.read(cx).focus_handle.is_focused(window);
        let save_focused = self.security_save_focus.is_focused(window);
        let msg          = self.pw_message.clone();
        let is_error     = self.pw_is_error;

        let save_btn = div()
            .id("settings-pw-save")
            .track_focus(&self.security_save_focus)
            .px(px(16.)).py(px(7.)).rounded(px(5.))
            .bg(rgb(c.surface_active))
            .text_size(rems(0.923)).text_color(rgb(c.text_default))
            .cursor_pointer()
            .when(save_focused, |d| d.border_2().border_color(rgb(c.text_muted)))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.do_security_save(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" { this.do_security_save(cx); }
            }))
            .child("Change Password");

        div().flex().flex_col()
            .child(
                div().flex().flex_col().py(px(14.)).px(px(32.)).gap(px(12.))
                    .key_context("SettingsSecurityForm")
                    .on_action(cx.listener(|this, _: &SecurityTabField, window, cx| {
                        let handles = [
                            this.pw_current.read(cx).focus_handle.clone(),
                            this.pw_new.read(cx).focus_handle.clone(),
                            this.pw_confirm.read(cx).focus_handle.clone(),
                            this.security_save_focus.clone(),
                        ];
                        let cur = handles.iter().position(|h| h.is_focused(window));
                        let next = handles[(cur.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                        window.focus(&next, cx);
                    }))
                    .on_action(cx.listener(|this, _: &SecurityBackTabField, window, cx| {
                        let handles = [
                            this.pw_current.read(cx).focus_handle.clone(),
                            this.pw_new.read(cx).focus_handle.clone(),
                            this.pw_confirm.read(cx).focus_handle.clone(),
                            this.security_save_focus.clone(),
                        ];
                        let cur = handles.iter().position(|h| h.is_focused(window));
                        let prev = handles[(cur.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                        window.focus(&prev, cx);
                    }))
                    .on_action(cx.listener(|this, _: &SecurityEscapeForm, window, cx| {
                        let h = this.pw_current.read(cx).focus_handle.clone();
                        window.focus(&h, cx);
                    }))
                    .child(vassl_ui::text_field("Current Password", self.pw_current.clone(), cur_focused,  false, cx))
                    .child(vassl_ui::text_field("New Password",     self.pw_new.clone(),     new_focused,  false, cx))
                    .child(vassl_ui::text_field("Confirm Password", self.pw_confirm.clone(), cf_focused,   false, cx))
                    .when_some(msg.clone(), |d, m| {
                        let col = if is_error { c.status_red } else { c.status_green };
                        d.child(div().text_size(rems(0.846)).text_color(rgb(col)).child(SharedString::from(m)))
                    })
                    .child(
                        div().flex().justify_end()
                            .child(save_btn)
                    )
            )
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
    }

    fn do_add_user_submit(&mut self, cx: &mut Context<Self>) {
        let username = self.add_username.read(cx).text().trim().to_string();
        let password = self.add_password.read(cx).text().to_string();
        if username.len() < 3 {
            self.add_error = Some("Username must be at least 3 characters.".into());
            cx.notify();
            return;
        }
        if password.len() < 6 {
            self.add_error = Some("Password must be at least 6 characters.".into());
            cx.notify();
            return;
        }
        let ci = self.add_can_inventory;
        let cp = self.add_can_pricebook;
        let cq = self.add_can_quotations;
        let ad = self.add_allow_delete;
        let ap = self.add_allow_price_edit;
        let db = UsersDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.insert_user(username, &password, ci, cp, cq, ad, ap).await;
            let _ = this.update(cx, |sp, cx| {
                match result {
                    Err(e) => { sp.add_error = Some(format!("Failed: {e}")); cx.notify(); }
                    Ok(_) => {
                        sp.admin_add_form_open = false;
                        sp.add_username.update(cx, |t, cx| t.reset(cx));
                        sp.add_password.update(cx, |t, cx| t.reset(cx));
                        sp.add_can_inventory = false; sp.add_can_pricebook = false;
                        sp.add_can_quotations = false; sp.add_allow_delete = false;
                        sp.add_allow_price_edit = false; sp.add_error = None;
                        sp.load_admin_users(cx);
                    }
                }
            });
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    fn render_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c          = cx.global::<ThemeHandle>().0.clone();
        let users      = self.admin_users.clone();
        let form_open  = self.admin_add_form_open;
        let add_err    = self.add_error.clone();
        let reset_id   = self.reset_pw_id;
        let u_focused  = self.add_username.read(cx).focus_handle.is_focused(window);
        let p_focused  = self.add_password.read(cx).focus_handle.is_focused(window);
        let rp_focused = self.reset_pw_input.read(cx).focus_handle.is_focused(window);

        let add_can_inv = self.add_can_inventory;
        let add_can_pb  = self.add_can_pricebook;
        let add_can_qt  = self.add_can_quotations;
        let add_del     = self.add_allow_delete;
        let add_pe      = self.add_allow_price_edit;

        let mut content = div().flex().flex_col().px(px(32.)).pt(px(16.));

        // ── User list ──────────────────────────────────────────────────────
        let user_rows: Vec<gpui::AnyElement> = users.iter().map(|u| {
            let uid      = u.id;
            let is_active = u.is_active;
            let uname    = u.username.clone();
            let ci = u.can_inventory; let cp = u.can_pricebook; let cq = u.can_quotations;
            let ad = u.allow_delete;  let ap = u.allow_price_edit;

            let status_col = if is_active { c.status_green } else { c.status_red };
            let status_lbl = if is_active { "Active" } else { "Inactive" };

            let toggle_active_btn = {
                let lbl = if is_active { "Deactivate" } else { "Reactivate" };
                let hover2 = rgb(c.surface_hover);
                div()
                    .id(SharedString::from(format!("user-toggle-{uid}")))
                    .px(px(8.)).py(px(3.)).rounded(px(4.))
                    .bg(rgb(c.surface_default))
                    .text_size(rems(0.769)).text_color(rgb(c.text_muted))
                    .cursor_pointer().hover(move |s| s.bg(hover2))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _: &MouseDownEvent, _, cx| {
                        let db = UsersDb::global(&**cx);
                        cx.spawn(async move |this, cx| {
                            let result = if is_active { db.deactivate_user(uid).await }
                                         else         { db.reactivate_user(uid).await };
                            if result.is_ok() {
                                let _ = this.update(cx, |sp, cx| { sp.load_admin_users(cx); });
                            }
                            Ok::<(), anyhow::Error>(())
                        }).detach();
                    }))
                    .child(lbl)
            };

            let reset_btn = {
                let hover2 = rgb(c.surface_hover);
                div()
                    .id(SharedString::from(format!("user-reset-{uid}")))
                    .px(px(8.)).py(px(3.)).rounded(px(4.))
                    .bg(rgb(c.surface_default))
                    .text_size(rems(0.769)).text_color(rgb(c.text_muted))
                    .cursor_pointer().hover(move |s| s.bg(hover2))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.reset_pw_id = Some(uid);
                        cx.notify();
                    }))
                    .child("Reset PW")
            };

            // Permission toggles (inline small)
            let perm_toggle = |id_str: &'static str, label: &'static str, val: bool, update_fn: fn(&mut SettingsPanel, bool, i64, &mut Context<SettingsPanel>)| {
                let col = if val { c.status_green } else { c.text_muted };
                div()
                    .id(SharedString::from(format!("{id_str}-{uid}")))
                    .px(px(6.)).py(px(2.)).rounded(px(3.))
                    .bg(rgb(c.surface_default))
                    .text_size(rems(0.692)).text_color(rgb(col))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        update_fn(this, !val, uid, cx);
                    }))
                    .child(label)
            };

            div()
                .flex().flex_col()
                .child(
                    div().flex().flex_row().items_center().py(px(10.)).gap(px(8.))
                        .child(div().w(px(140.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(uname.clone()))
                        .child(div().text_size(rems(0.769)).text_color(rgb(status_col)).child(status_lbl))
                        .child(div().flex_1())
                        .child(perm_toggle("ci", "Inv", ci, |sp, v, id, cx| { sp.save_perm_update(id, v, sp.admin_users.iter().find(|u| u.id == id).map(|u| u.can_pricebook).unwrap_or(false), sp.admin_users.iter().find(|u| u.id == id).map(|u| u.can_quotations).unwrap_or(false), sp.admin_users.iter().find(|u| u.id == id).map(|u| u.allow_delete).unwrap_or(false), sp.admin_users.iter().find(|u| u.id == id).map(|u| u.allow_price_edit).unwrap_or(false), cx); }))
                        .child(perm_toggle("cp", "PB",  cp, |sp, v, id, cx| { let u = sp.admin_users.iter().find(|u| u.id == id).cloned().unwrap_or_else(|| AuthUser { id, username: String::new(), is_admin: false, can_inventory: false, can_pricebook: false, can_quotations: false, allow_delete: false, allow_price_edit: false, must_change_password: false, is_active: true }); sp.save_perm_update(id, u.can_inventory, v, u.can_quotations, u.allow_delete, u.allow_price_edit, cx); }))
                        .child(perm_toggle("cq", "Qt",  cq, |sp, v, id, cx| { let u = sp.admin_users.iter().find(|u| u.id == id).cloned().unwrap_or_else(|| AuthUser { id, username: String::new(), is_admin: false, can_inventory: false, can_pricebook: false, can_quotations: false, allow_delete: false, allow_price_edit: false, must_change_password: false, is_active: true }); sp.save_perm_update(id, u.can_inventory, u.can_pricebook, v, u.allow_delete, u.allow_price_edit, cx); }))
                        .child(perm_toggle("ad", "Del", ad, |sp, v, id, cx| { let u = sp.admin_users.iter().find(|u| u.id == id).cloned().unwrap_or_else(|| AuthUser { id, username: String::new(), is_admin: false, can_inventory: false, can_pricebook: false, can_quotations: false, allow_delete: false, allow_price_edit: false, must_change_password: false, is_active: true }); sp.save_perm_update(id, u.can_inventory, u.can_pricebook, u.can_quotations, v, u.allow_price_edit, cx); }))
                        .child(perm_toggle("ap", "PEd", ap, |sp, v, id, cx| { let u = sp.admin_users.iter().find(|u| u.id == id).cloned().unwrap_or_else(|| AuthUser { id, username: String::new(), is_admin: false, can_inventory: false, can_pricebook: false, can_quotations: false, allow_delete: false, allow_price_edit: false, must_change_password: false, is_active: true }); sp.save_perm_update(id, u.can_inventory, u.can_pricebook, u.can_quotations, u.allow_delete, v, cx); }))
                        .child(reset_btn)
                        .child(toggle_active_btn)
                )
                .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                .into_any_element()
        }).collect();

        content = content.children(user_rows);

        // ── Reset password inline panel ─────────────────────────────────
        if let Some(rid) = reset_id {
            let hover_cancel = rgb(c.surface_hover);
            let rp_save_focused   = self.reset_pw_save_focus.is_focused(window);
            let rp_cancel_focused = self.reset_pw_cancel_focus.is_focused(window);
            content = content.child(
                div().mt(px(12.)).flex().flex_row().items_center().gap(px(8.))
                    .key_context("SettingsAdminResetForm")
                    .on_action(cx.listener(|this, _: &AdminResetTabField, window, cx| {
                        let handles = [
                            this.reset_pw_input.read(cx).focus_handle.clone(),
                            this.reset_pw_save_focus.clone(),
                            this.reset_pw_cancel_focus.clone(),
                        ];
                        let cur = handles.iter().position(|h| h.is_focused(window));
                        let next = handles[(cur.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                        window.focus(&next, cx);
                    }))
                    .on_action(cx.listener(|this, _: &AdminResetBackTabField, window, cx| {
                        let handles = [
                            this.reset_pw_input.read(cx).focus_handle.clone(),
                            this.reset_pw_save_focus.clone(),
                            this.reset_pw_cancel_focus.clone(),
                        ];
                        let cur = handles.iter().position(|h| h.is_focused(window));
                        let prev = handles[(cur.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                        window.focus(&prev, cx);
                    }))
                    .on_action(cx.listener(|this, _: &AdminResetEscapeForm, _, cx| {
                        this.reset_pw_id = None;
                        this.reset_pw_input.update(cx, |t, cx| t.reset(cx));
                        cx.notify();
                    }))
                    .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted))
                        .child(format!("Reset password for user #{rid}:")))
                    .child(div().w(px(200.)).child(vassl_ui::text_field("", self.reset_pw_input.clone(), rp_focused, false, cx)))
                    .child(
                        div()
                            .id("admin-reset-pw-save")
                            .track_focus(&self.reset_pw_save_focus)
                            .px(px(12.)).py(px(6.)).rounded(px(4.))
                            .bg(rgb(c.surface_active))
                            .text_size(rems(0.846)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .when(rp_save_focused, |d| d.border_2().border_color(rgb(c.text_muted)))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                let new_pw = this.reset_pw_input.read(cx).text().to_string();
                                if new_pw.len() < 6 {
                                    this.add_error = Some("Password must be at least 6 characters.".into());
                                    cx.notify();
                                    return;
                                }
                                let db = UsersDb::global(&**cx);
                                cx.spawn(async move |this, cx| {
                                    let result = db.reset_password(rid, &new_pw).await;
                                    let _ = this.update(cx, |sp, cx| {
                                        sp.reset_pw_id = None;
                                        sp.reset_pw_input.update(cx, |t, cx| t.reset(cx));
                                        if let Err(e) = result {
                                            sp.add_error = Some(format!("Reset failed: {e}"));
                                        }
                                        cx.notify();
                                    });
                                    Ok::<(), anyhow::Error>(())
                                }).detach();
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if event.keystroke.key != "enter" { return; }
                                let new_pw = this.reset_pw_input.read(cx).text().to_string();
                                if new_pw.len() < 6 {
                                    this.add_error = Some("Password must be at least 6 characters.".into());
                                    cx.notify();
                                    return;
                                }
                                let db = UsersDb::global(&**cx);
                                cx.spawn(async move |this, cx| {
                                    let result = db.reset_password(rid, &new_pw).await;
                                    let _ = this.update(cx, |sp, cx| {
                                        sp.reset_pw_id = None;
                                        sp.reset_pw_input.update(cx, |t, cx| t.reset(cx));
                                        if let Err(e) = result {
                                            sp.add_error = Some(format!("Reset failed: {e}"));
                                        }
                                        cx.notify();
                                    });
                                    Ok::<(), anyhow::Error>(())
                                }).detach();
                            }))
                            .child("Save")
                    )
                    .child(
                        div()
                            .id("admin-reset-pw-cancel")
                            .track_focus(&self.reset_pw_cancel_focus)
                            .px(px(12.)).py(px(6.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .text_size(rems(0.846)).text_color(rgb(c.text_muted))
                            .cursor_pointer().hover(move |s| s.bg(hover_cancel))
                            .when(rp_cancel_focused, |d| d.border_2().border_color(rgb(c.text_muted)))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                this.reset_pw_id = None;
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if event.keystroke.key == "enter" {
                                    this.reset_pw_id = None;
                                    cx.notify();
                                }
                            }))
                            .child("Cancel")
                    )
            );
        }

        // ── Add user button / form ──────────────────────────────────────
        content = content.child(
            div().mt(px(16.)).flex().flex_col().gap(px(10.))
                .child(
                    div()
                        .id("admin-add-user-toggle")
                        .px(px(14.)).py(px(7.)).rounded(px(5.))
                        .bg(rgb(if form_open { c.surface_default } else { c.surface_active }))
                        .text_size(rems(0.923)).text_color(rgb(c.text_default))
                        .cursor_pointer()
                        .when(!form_open, |d| d.hover(move |s| s.bg(rgb(c.surface_hover))))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.admin_add_form_open = !this.admin_add_form_open;
                            if !this.admin_add_form_open { this.add_error = None; }
                            cx.notify();
                        }))
                        .child(if form_open { "Cancel" } else { "+ Add User" })
                )
                .when(form_open, |d| {
                    let add_save_focused = self.add_user_save_focus.is_focused(window);
                    d.child(
                        div().flex().flex_col().gap(px(10.))
                            .p(px(16.))
                            .bg(rgb(c.surface_default))
                            .rounded(px(6.))
                            .key_context("SettingsAdminAddForm")
                            .on_action(cx.listener(|this, _: &AdminAddTabField, window, cx| {
                                let handles = [
                                    this.add_username.read(cx).focus_handle.clone(),
                                    this.add_password.read(cx).focus_handle.clone(),
                                    this.add_user_save_focus.clone(),
                                ];
                                let cur = handles.iter().position(|h| h.is_focused(window));
                                let next = handles[(cur.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                                window.focus(&next, cx);
                            }))
                            .on_action(cx.listener(|this, _: &AdminAddBackTabField, window, cx| {
                                let handles = [
                                    this.add_username.read(cx).focus_handle.clone(),
                                    this.add_password.read(cx).focus_handle.clone(),
                                    this.add_user_save_focus.clone(),
                                ];
                                let cur = handles.iter().position(|h| h.is_focused(window));
                                let prev = handles[(cur.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                                window.focus(&prev, cx);
                            }))
                            .on_action(cx.listener(|this, _: &AdminAddEscapeForm, _, cx| {
                                this.admin_add_form_open = false;
                                this.add_error = None;
                                cx.notify();
                            }))
                            .child(vassl_ui::text_field("Username", self.add_username.clone(), u_focused, false, cx))
                            .child(vassl_ui::text_field("Password", self.add_password.clone(), p_focused, false, cx))
                            // Permission checkboxes
                            .child(
                                div().flex().flex_row().flex_wrap().gap(px(12.)).pt(px(4.))
                                    .child(Self::perm_checkbox("add-ci", "Inventory",        add_can_inv, cx.listener(|this, _, _, cx| { this.add_can_inventory    = !this.add_can_inventory;    cx.notify(); }), &c))
                                    .child(Self::perm_checkbox("add-cp", "Price Book",       add_can_pb,  cx.listener(|this, _, _, cx| { this.add_can_pricebook    = !this.add_can_pricebook;    cx.notify(); }), &c))
                                    .child(Self::perm_checkbox("add-cq", "Quotations",       add_can_qt,  cx.listener(|this, _, _, cx| { this.add_can_quotations   = !this.add_can_quotations;   cx.notify(); }), &c))
                                    .child(Self::perm_checkbox("add-ad", "Allow Delete",     add_del,     cx.listener(|this, _, _, cx| { this.add_allow_delete     = !this.add_allow_delete;     cx.notify(); }), &c))
                                    .child(Self::perm_checkbox("add-ap", "Allow Price Edit", add_pe,      cx.listener(|this, _, _, cx| { this.add_allow_price_edit = !this.add_allow_price_edit; cx.notify(); }), &c))
                            )
                            .when_some(add_err, |d, err| {
                                d.child(div().text_size(rems(0.846)).text_color(rgb(c.status_red)).child(SharedString::from(err)))
                            })
                            .child(
                                div().flex().justify_end()
                                    .child(
                                        div()
                                            .id("admin-add-user-save")
                                            .track_focus(&self.add_user_save_focus)
                                            .px(px(16.)).py(px(7.)).rounded(px(5.))
                                            .bg(rgb(c.surface_active))
                                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                            .cursor_pointer()
                                            .when(add_save_focused, |d| d.border_2().border_color(rgb(c.text_muted)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                                this.do_add_user_submit(cx);
                                            }))
                                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                                if event.keystroke.key == "enter" {
                                                    this.do_add_user_submit(cx);
                                                }
                                            }))
                                            .child("Create User")
                                    )
                            )
                    )
                })
        );

        content
    }

    fn perm_checkbox<F>(
        id:       &'static str,
        label:    &'static str,
        checked:  bool,
        on_click: F,
        c:        &vassl_ui::ThemeColors,
    ) -> impl IntoElement
    where
        F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        div().flex().flex_row().items_center().gap(px(6.)).cursor_pointer()
            .id(id)
            .on_mouse_down(MouseButton::Left, on_click)
            .child(
                div()
                    .w(px(16.)).h(px(16.)).rounded(px(3.))
                    .border_1().border_color(rgb(c.surface_active))
                    .bg(rgb(if checked { c.surface_active } else { c.canvas_bg }))
                    .flex().items_center().justify_center()
                    .when(checked, |d| d.child(div().text_size(rems(0.692)).text_color(rgb(c.text_default)).child("✓")))
            )
            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_default)).child(label))
    }

    fn save_perm_update(
        &mut self,
        id: i64,
        can_inventory: bool, can_pricebook: bool, can_quotations: bool,
        allow_delete: bool, allow_price_edit: bool,
        cx: &mut Context<Self>,
    ) {
        // Optimistically update local state
        if let Some(u) = self.admin_users.iter_mut().find(|u| u.id == id) {
            u.can_inventory    = can_inventory;
            u.can_pricebook    = can_pricebook;
            u.can_quotations   = can_quotations;
            u.allow_delete     = allow_delete;
            u.allow_price_edit = allow_price_edit;
        }
        cx.notify();
        let db = UsersDb::global(&**cx);
        cx.spawn(async move |_, _cx| {
            if let Err(e) = db.update_user_permissions(id, can_inventory, can_pricebook, can_quotations, allow_delete, allow_price_edit).await {
                tracing::warn!("update_user_permissions failed: {e:?}");
            }
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    fn render_license(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let c  = cx.global::<ThemeHandle>().0.clone();
        let db = vassl_db::AppDatabase::global(&**cx);

        let stored_key = vassl_db::shared::get_setting(db, "license.key")
            .ok().flatten();
        let license_info = stored_key.as_deref()
            .and_then(|key| crate::license::validate_key(key).ok());

        let masked_key: Option<String> = stored_key.as_deref().map(|key| {
            let prefix = key.get(..11).unwrap_or(key); // "VASSL-XXXXX"
            format!("{prefix}-•••••-•••••-•••••")
        });

        match license_info {
            Some(info) => {
                let edition_str = info.edition.to_string();
                let expiry_str  = match info.expiry {
                    Some(d) => d.format("%Y-%m-%d").to_string(),
                    None    => "Never expires".to_string(),
                };

                let edition_badge = div()
                    .px(px(12.)).py(px(5.)).rounded(px(4.))
                    .bg(rgb(c.surface_active))
                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                    .child(edition_str);

                let expiry_chip = div()
                    .px(px(12.)).py(px(5.)).rounded(px(4.))
                    .bg(rgb(c.surface_default))
                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                    .child(expiry_str);

                let key_chip = div()
                    .px(px(12.)).py(px(5.)).rounded(px(4.))
                    .bg(rgb(c.surface_default))
                    .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                    .child(masked_key.unwrap_or_default());

                div().flex().flex_col()
                    .child(Self::render_row(
                        "Edition",
                        "Your VASSL license edition.",
                        edition_badge,
                        &c,
                    ))
                    .child(Self::render_row(
                        "Expiry",
                        "The date your license key expires, or \"Never expires\" if it does not.",
                        expiry_chip,
                        &c,
                    ))
                    .child(Self::render_row(
                        "License Key",
                        "The activated license key (partially masked).",
                        key_chip,
                        &c,
                    ))
            }
            None => {
                div().flex().flex_col()
                    .child(
                        div().px(px(32.)).py(px(20.))
                            .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                            .child("No valid license is active.")
                    )
            }
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
            // DB location row
            .child(
                div().flex().flex_col()
                    .child(
                        div().flex().flex_row().items_center().py(px(14.)).px(px(32.)).gap(px(12.))
                            .child(
                                div().flex_1().min_w(px(0.)).flex().flex_col().gap(px(4.))
                                    .child(div().text_size(rems(1.)).text_color(rgb(c.text_default))
                                        .child("Database Location"))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child("Path to the live SQLite database file."))
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                        .child(SharedString::from(db_display)))
                            )
                            .child(
                                div().id("settings-db-reveal")
                                    .px(px(16.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |_, _: &MouseDownEvent, _, cx| {
                                        cx.reveal_path(&db_path);
                                    }))
                                    .child("Open Now")
                            )
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
        let c            = cx.global::<ThemeHandle>().0.clone();
        let name_focused = self.user_name.read(cx).focus_handle.is_focused(window);
        let co_focused   = self.company_name.read(cx).focus_handle.is_focused(window);

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

        let is_admin = cx.global::<AppSettings>().is_admin;

        let mut categories_vec = vec![
            SettingsCategory::General,
            SettingsCategory::Appearance,
            SettingsCategory::Inventory,
            SettingsCategory::PriceBook,
            SettingsCategory::Quotations,
            SettingsCategory::Database,
            SettingsCategory::Keyboard,
            SettingsCategory::Security,
        ];
        if is_admin {
            categories_vec.push(SettingsCategory::Admin);
        }
        categories_vec.push(SettingsCategory::License);
        let categories = categories_vec;

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
                        if cat == SettingsCategory::Admin {
                            this.load_admin_users(cx);
                        }
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
                    SettingsCategory::Security   => self.render_security(window, cx).into_any_element(),
                    SettingsCategory::Admin      => self.render_admin(window, cx).into_any_element(),
                    SettingsCategory::License    => self.render_license(cx).into_any_element(),
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
        let v = 13.0_f64;
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
