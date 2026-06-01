# Settings UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a full Settings panel to VASSL with 5 categories (General, Appearance, Inventory, Price Book, Quotations), a system font picker, a font-size stepper, a theme toggle, and live propagation of theme/font changes.

**Architecture:** `ActiveModule::Settings` variant added to the sidebar enum; clicking ⚙ switches to it. `SettingsPanel` is a single GPUI entity in `vassl-app` that owns all setting state, loaded from the existing `settings` key/value DB table on construction, and saved back on every change via `cx.observe()` + `cx.spawn(db.write(...))`.

**Tech Stack:** GPUI entity/render pattern, `vassl-db` `get_setting`/`set_setting` helpers, `cx.text_system().all_font_names()` for font enumeration, `.font_family()` styled div method, `cx.set_global(ThemeHandle(...))` for live theme, `window.set_rem_size()` for live font size.

---

## File Map

| File | Change |
|---|---|
| `crates/vassl-app/src/actions.rs` | Add `OpenSettings` |
| `crates/vassl-app/src/main.rs` | Add `OpenSettings` keybinding + import |
| `crates/vassl-app/src/sidebar.rs` | Add `ActiveModule::Settings`, wire ⚙ button |
| `crates/vassl-app/src/settings_panel.rs` | **Create** — full `SettingsPanel` entity |
| `crates/vassl-app/src/root.rs` | Add `settings_panel` field, render it, apply theme+font on startup, handle `OpenSettings` action |

---

### Task 1: OpenSettings action + ActiveModule::Settings + keybinding

**Files:**
- Modify: `crates/vassl-app/src/actions.rs`
- Modify: `crates/vassl-app/src/sidebar.rs`
- Modify: `crates/vassl-app/src/main.rs`

- [ ] **Step 1: Write failing test**

Add to `sidebar.rs` tests block (it already has one):

```rust
#[test]
fn settings_module_is_distinct() {
    assert_ne!(ActiveModule::Settings, ActiveModule::Inventory);
    assert_ne!(ActiveModule::Settings, ActiveModule::Quotations);
    assert_ne!(ActiveModule::Settings, ActiveModule::PriceBook);
}
```

- [ ] **Step 2: Run test — expect compile error (Settings variant missing)**

```bash
cargo test -p vassl-app 2>&1 | head -20
```

Expected: `error[E0599]: no variant or associated item named 'Settings'`

- [ ] **Step 3: Add `OpenSettings` to actions.rs**

Replace the entire file:

```rust
use gpui::actions;

actions!(vassl, [
    OpenInventory,
    OpenQuotations,
    OpenPriceBook,
    OpenAuditLog,
    OpenSettings,
    NewRecord,
    FocusSearch,
    EscapeModal,
    SelectNext,
    SelectPrev,
    ConfirmSelection,
]);
```

- [ ] **Step 4: Add `ActiveModule::Settings` and wire ⚙ button in sidebar.rs**

Replace the entire file:

```rust
use gpui::{
    Context, IntoElement, MouseButton, Render, Window, div, prelude::*, px, rgb,
};
use vassl_ui::ThemeHandle;

use crate::colors;

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
```

- [ ] **Step 5: Add keybinding in main.rs**

Add `OpenSettings` to the import line:

```rust
use actions::{ConfirmSelection, EscapeModal, FocusSearch, NewRecord, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations, OpenSettings, SelectNext, SelectPrev};
```

Add keybinding after the `secondary-f` line:

```rust
KeyBinding::new("secondary-comma", OpenSettings, Some("VasslRoot")),
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p vassl-app 2>&1 | tail -5
```

Expected: `test result: ok. N passed; 0 failed`

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-app/src/actions.rs crates/vassl-app/src/sidebar.rs crates/vassl-app/src/main.rs
git commit -m "feat(settings): OpenSettings action, ActiveModule::Settings, keybinding"
```

---

### Task 2: SettingsPanel scaffold + root wiring

**Files:**
- Create: `crates/vassl-app/src/settings_panel.rs`
- Modify: `crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write failing tests in settings_panel.rs**

Create the file with just the test module first:

```rust
// crates/vassl-app/src/settings_panel.rs

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
            "general.user_name",
            "general.company_name",
            "appearance.theme",
            "appearance.font_family",
            "appearance.font_size",
            "inventory.low_stock_threshold",
            "inventory.default_unit",
            "pricebook.currency",
            "pricebook.usd_to_jmd_rate",
            "pricebook.default_margin",
            "quotations.prefix",
            "quotations.tax_rate",
            "quotations.notes_template",
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
```

- [ ] **Step 2: Run — expect compile error**

```bash
cargo test -p vassl-app 2>&1 | head -10
```

Expected: `error[E0412]: cannot find type 'SettingsCategory'`

- [ ] **Step 3: Write the full SettingsPanel scaffold**

Replace the file with:

```rust
use gpui::{Context, FocusHandle, Focusable, IntoElement, Render, SharedString, Window,
           div, prelude::*, px, rgb};
use vassl_ui::{TextInput, ThemeHandle, text_field};

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
            .flex_1().h_full().overflow_y_scroll()
            .bg(rgb(c.canvas_bg))
            .flex().flex_col()
            .child(
                // section header
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
```

- [ ] **Step 4: Wire SettingsPanel into root.rs**

Add `mod settings_panel;` at top of root.rs with the other mod declarations. Then add to imports:

```rust
use crate::settings_panel::SettingsPanel;
use crate::actions::{EscapeModal, FocusSearch, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations, OpenSettings};
```

Add field to `VasslRoot` struct after `quotation_panel`:

```rust
settings_panel:   Entity<SettingsPanel>,
```

Add to `VasslRoot::new()` in the `Self { ... }` block:

```rust
settings_panel:   cx.new(SettingsPanel::new),
```

In `Render for VasslRoot`, add the Settings arm to the match:

```rust
let content = match active {
    ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
    ActiveModule::Quotations => content.child(self.quotation_panel.clone()),
    ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
    ActiveModule::Settings   => content.child(self.settings_panel.clone()),
};
```

Add `OpenSettings` action handler in `VasslRoot::render()` alongside the others:

```rust
.on_action(cx.listener(|this, _: &OpenSettings, _w, cx| {
    this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Settings; cx.notify(); });
}))
```

- [ ] **Step 5: Build and run tests**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
cargo test -p vassl-app 2>&1 | tail -5
```

Expected: `Finished` and `test result: ok. N passed; 0 failed`

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs crates/vassl-app/src/root.rs
git commit -m "feat(settings): SettingsPanel scaffold with category nav, wired to VasslRoot"
```

---

### Task 3: Load settings from DB + save_setting helper

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`

- [ ] **Step 1: Write failing test**

Add to the tests block in `settings_panel.rs`:

```rust
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
    for v in &valid {
        assert!(valid.contains(v));
    }
    assert!(!valid.contains(&"blue"));
}

#[test]
fn currency_values_are_usd_or_jmd() {
    let valid = ["USD", "JMD"];
    assert!(valid.contains(&"USD"));
    assert!(valid.contains(&"JMD"));
    assert!(!valid.contains(&"EUR"));
}
```

- [ ] **Step 2: Run tests — they should already pass**

```bash
cargo test -p vassl-app settings_panel 2>&1 | tail -5
```

Expected: `test result: ok.`

- [ ] **Step 3: Add DB loading to `SettingsPanel::new()`**

Replace `SettingsPanel::new()` with:

```rust
pub fn new(cx: &mut Context<Self>) -> Self {
    // helper: read a setting with fallback
    let db = vassl_db::AppDatabase::global(&**cx);
    let get = |key: &str, default: &str| -> String {
        vassl_db::shared::get_setting(db, key)
            .ok().flatten()
            .unwrap_or_else(|| default.to_string())
    };

    // general.user_name falls back to legacy current_user key
    let user_name_val = vassl_db::shared::get_setting(db, "general.user_name")
        .ok().flatten()
        .or_else(|| vassl_db::shared::get_setting(db, "current_user").ok().flatten())
        .unwrap_or_default();

    let company_name_val  = get("general.company_name",          "");
    let theme_val         = get("appearance.theme",              "dark");
    let font_family_val   = get("appearance.font_family",        "system-ui");
    let font_size_val     = get("appearance.font_size",          "13");
    let low_stock_val     = get("inventory.low_stock_threshold", "5");
    let default_unit_val  = get("inventory.default_unit",        "pcs");
    let currency_val      = get("pricebook.currency",            "USD");
    let usd_jmd_val       = get("pricebook.usd_to_jmd_rate",     "157.50");
    let margin_val        = get("pricebook.default_margin",      "20");
    let prefix_val        = get("quotations.prefix",             "VASSL");
    let tax_val           = get("quotations.tax_rate",           "0");
    let notes_val         = get("quotations.notes_template",     "");

    let font_size: f64 = font_size_val.parse().unwrap_or(13.0).max(10.0).min(24.0);

    let font_names = cx.text_system().all_font_names();

    let make_input = |placeholder: &'static str, value: String, cx: &mut Context<Self>| {
        cx.new(move |cx| {
            let mut input = TextInput::with_placeholder(placeholder, cx);
            input.set_text(value, cx);
            input
        })
    };

    let user_name    = make_input("e.g. Alice Kamalu",  user_name_val,    cx);
    let company_name = make_input("e.g. Kamalu Ltd.",   company_name_val, cx);
    let low_stock    = make_input("5",                  low_stock_val,    cx);
    let default_unit = make_input("pcs",                default_unit_val, cx);
    let usd_to_jmd   = make_input("157.50",             usd_jmd_val,      cx);
    let default_margin = make_input("20",               margin_val,       cx);
    let quote_prefix = make_input("VASSL",              prefix_val,       cx);
    let tax_rate     = make_input("0",                  tax_val,          cx);
    let notes_template = make_input("",                 notes_val,        cx);

    // auto-save TextInputs on every change
    cx.observe(&user_name,     |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("general.user_name",           v, cx); }).detach();
    cx.observe(&company_name,  |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("general.company_name",         v, cx); }).detach();
    cx.observe(&low_stock,     |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("inventory.low_stock_threshold", v, cx); }).detach();
    cx.observe(&default_unit,  |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("inventory.default_unit",        v, cx); }).detach();
    cx.observe(&usd_to_jmd,    |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("pricebook.usd_to_jmd_rate",     v, cx); }).detach();
    cx.observe(&default_margin,|this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("pricebook.default_margin",      v, cx); }).detach();
    cx.observe(&quote_prefix,  |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("quotations.prefix",             v, cx); }).detach();
    cx.observe(&tax_rate,      |this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("quotations.tax_rate",           v, cx); }).detach();
    cx.observe(&notes_template,|this, f, cx| { let v = f.read(cx).text().to_string(); this.save_setting("quotations.notes_template",     v, cx); }).detach();

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
```

- [ ] **Step 4: Add `save_setting` private method**

Add after `new()`:

```rust
fn save_setting(&self, key: &'static str, value: String, cx: &mut Context<Self>) {
    let db = vassl_db::AppDatabase::global(&**cx).clone();
    cx.spawn(async move |_, _| {
        let _ = db.write(move |conn| vassl_db::shared::set_setting(conn, key, &value)).await;
        Ok::<(), anyhow::Error>(())
    }).detach();
}
```

- [ ] **Step 5: Update imports at top of settings_panel.rs**

The imports block must be:

```rust
use gpui::{Context, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb};
use vassl_ui::{TextInput, ThemeHandle, text_field};
```

- [ ] **Step 6: Build**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs
git commit -m "feat(settings): load all settings from DB on open, save_setting helper, auto-save via observe"
```

---

### Task 4: General category rows

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`

- [ ] **Step 1: Add a `render_row` helper method**

Add this private method to `impl SettingsPanel` (before `render()`):

```rust
fn render_row<'a>(
    title:       &'static str,
    description: &'static str,
    control:     impl IntoElement + 'a,
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
```

- [ ] **Step 2: Replace the placeholder content area in `render()` with real General rows**

Find the `SettingsCategory::General` content rendering. Replace the content `div()` child that renders `"(settings rows will appear here)"` with a method call. Update `render()` so the content area child is:

```rust
.child({
    match active {
        SettingsCategory::General => self.render_general(window, cx),
        _ => div().px(px(32.)).py(px(24.))
                  .child(div().text_size(px(12.)).text_color(rgb(c.text_muted))
                         .child("(coming soon)")),
    }
})
```

Note: `render_general` needs `&mut self` so call it via `cx.listener` pattern — actually in GPUI render we have `&mut self` available. Use a direct call: add a method:

```rust
fn render_general(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let c = cx.global::<ThemeHandle>().0.clone();
    let name_focused = self.user_name.read(cx).focus_handle.is_focused(_window);
    let co_focused   = self.company_name.read(cx).focus_handle.is_focused(_window);

    div().flex().flex_col()
        .child(Self::render_row(
            "User Name",
            "Your display name, used in audit logs.",
            text_field("", self.user_name.clone(), name_focused, cx),
            &c,
        ))
        .child(Self::render_row(
            "Company Name",
            "Appears on quotation headers.",
            text_field("", self.company_name.clone(), co_focused, cx),
            &c,
        ))
}
```

Because `render_general` calls `_window.is_focused()`, update its signature to take `window: &mut Window` and pass `window` from the parent render call. Update the match call site in `render()` accordingly:

```rust
SettingsCategory::General => self.render_general(window, cx),
```

- [ ] **Step 3: Fix render() signature chain**

The `render()` method signature is `fn render(&mut self, window: &mut Window, cx: &mut Context<Self>)`. Make sure `window` (not `_window`) is used in the render body since we're passing it to `render_general`.

- [ ] **Step 4: Build**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs
git commit -m "feat(settings): General category — User Name + Company Name rows"
```

---

### Task 5: Appearance category — theme toggle + font size stepper

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`
- Modify: `crates/vassl-app/src/root.rs`

- [ ] **Step 1: Write tests for stepper and theme toggle logic**

Add to tests block:

```rust
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
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vassl-app settings_panel 2>&1 | tail -5
```

Expected: `test result: ok.`

- [ ] **Step 3: Add `render_appearance` method**

Add to `impl SettingsPanel`:

```rust
fn render_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let c       = cx.global::<ThemeHandle>().0.clone();
    let is_dark = self.theme == "dark";

    // ── theme toggle ────────────────────────────────────────────────
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
                cx.set_global(vassl_ui::ThemeHandle(colors));
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

    // ── font size stepper ───────────────────────────────────────────
    let font_size = self.font_size;
    let stepper = div().flex().flex_row().items_center().gap(px(2.))
        .child(
            div().id("settings-font-minus")
                .px(px(10.)).py(px(5.)).rounded(px(4.))
                .bg(rgb(c.surface_default)).text_size(px(13.)).text_color(rgb(c.text_default))
                .cursor_pointer()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                    this.font_size = (this.font_size - 0.5).max(10.0);
                    window.set_rem_size(px(this.font_size as f32));
                    this.save_setting("appearance.font_size", format!("{}", this.font_size), cx);
                    cx.notify();
                }))
                .child("−")
        )
        .child(
            div().w(px(52.)).px(px(4.)).py(px(5.))
                .bg(rgb(c.surface_default)).rounded(px(4.))
                .text_size(px(12.)).text_color(rgb(c.text_default))
                .flex().items_center().justify_center()
                .child(format!("{:.1}", font_size))
        )
        .child(
            div().id("settings-font-plus")
                .px(px(10.)).py(px(5.)).rounded(px(4.))
                .bg(rgb(c.surface_default)).text_size(px(13.)).text_color(rgb(c.text_default))
                .cursor_pointer()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                    this.font_size = (this.font_size + 0.5).min(24.0);
                    window.set_rem_size(px(this.font_size as f32));
                    this.save_setting("appearance.font_size", format!("{}", this.font_size), cx);
                    cx.notify();
                }))
                .child("+")
        );

    // ── reset label wrapper ──────────────────────────────────────────
    let font_size_label = div().flex().flex_row().items_center().gap(px(6.))
        .child(div().text_size(px(13.)).text_color(rgb(c.text_default)).child("Font Size"))
        .child(
            div().id("settings-font-reset")
                .text_size(px(11.)).text_color(rgb(c.text_muted)).cursor_pointer()
                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                    this.font_size = 13.0;
                    window.set_rem_size(px(13.0_f32));
                    this.save_setting("appearance.font_size", "13".into(), cx);
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
                                .child(div().text_size(px(13.)).text_color(rgb(c.text_default)).child("Theme"))
                                .child(div().text_size(px(11.)).text_color(rgb(c.text_muted))
                                    .child(if is_dark { "Dark mode" } else { "Light mode" }))
                        )
                        .child(toggle)
                )
                .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
        )
        // Font size row (label with ↺ + stepper)
        .child(
            div().flex().flex_col()
                .child(
                    div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                        .child(
                            div().flex_1().flex().flex_col().gap(px(3.))
                                .child(font_size_label)
                                .child(div().text_size(px(11.)).text_color(rgb(c.text_muted))
                                    .child("UI font size in pixels. Step 0.5, range 10–24."))
                        )
                        .child(stepper)
                )
                .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
        )
}
```

- [ ] **Step 4: Wire `render_appearance` into the match in `render()`**

Replace the `_ => div()...` arm temporarily with:

```rust
SettingsCategory::General    => self.render_general(window, cx),
SettingsCategory::Appearance => self.render_appearance(window, cx),
_ => div().px(px(32.)).py(px(24.))
          .child(div().text_size(px(12.)).text_color(rgb(c.text_muted))
                 .child("(coming soon)")),
```

- [ ] **Step 5: Apply saved font size + theme in VasslRoot::new()**

In `root.rs`, at the top of `VasslRoot::new()` after the `needs_first_run` block, add:

```rust
// Apply persisted font size and theme before any rendering occurs
{
    let db = vassl_db::AppDatabase::global(&**cx);
    if let Ok(Some(size_str)) = vassl_db::shared::get_setting(db, "appearance.font_size") {
        if let Ok(size) = size_str.parse::<f32>() {
            let clamped = size.max(10.0).min(24.0);
            window.set_rem_size(px(clamped));
        }
    }
    if let Ok(Some(theme)) = vassl_db::shared::get_setting(db, "appearance.theme") {
        let colors = if theme == "light" {
            vassl_ui::ThemeColors::light()
        } else {
            vassl_ui::ThemeColors::dark()
        };
        cx.set_global(vassl_ui::ThemeHandle(colors));
    }
}
```

- [ ] **Step 6: Build**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

- [ ] **Step 7: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs crates/vassl-app/src/root.rs
git commit -m "feat(settings): Appearance category — theme toggle + font size stepper, apply on startup"
```

---

### Task 6: Font picker — system fonts scrollable list

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`

- [ ] **Step 1: Write tests**

Add to tests block:

```rust
#[test]
fn font_family_key_is_correct() {
    assert_eq!("appearance.font_family", "appearance.font_family");
}

#[test]
fn font_picker_default_is_system_ui() {
    let default = "system-ui";
    assert!(!default.is_empty());
}
```

- [ ] **Step 2: Add `render_font_picker` helper**

Add to `impl SettingsPanel`:

```rust
fn render_font_picker(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
    let c          = cx.global::<ThemeHandle>().0.clone();
    let is_open    = self.open_select == Some(SettingSelect::FontPicker);
    let current    = self.font_family.clone();
    let font_names = self.font_names.clone();

    div().flex().flex_col()
        // trigger button
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
                    div().flex_1().text_size(px(12.)).text_color(rgb(c.text_default))
                        .font_family(current.clone())
                        .child(current.clone())
                )
                .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("◇"))
        )
        // inline scrollable list
        .when(is_open, move |d| {
            d.child(
                div().id("settings-font-list")
                    .mt(px(2.)).max_h(px(200.)).overflow_y_scroll()
                    .bg(rgb(c.surface_default)).rounded(px(4.))
                    .border_1().border_color(rgb(c.surface_active))
                    .children(font_names.into_iter().map(|name| {
                        let name2   = name.clone();
                        let name3   = name.clone();
                        let selected = name == current;
                        let bg = if selected { c.surface_active } else { c.surface_default };
                        div()
                            .id(format!("font-{name}"))
                            .px(px(10.)).py(px(5.))
                            .bg(rgb(bg)).cursor_pointer()
                            .font_family(SharedString::from(name.clone()))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.font_family = name2.clone();
                                this.open_select = None;
                                this.save_setting("appearance.font_family", name2.clone(), cx);
                                cx.notify();
                            }))
                            .child(name3)
                    }))
            )
        })
}
```

- [ ] **Step 3: Wire font picker into `render_appearance()`**

In `render_appearance()`, add a Font Family row after the Theme row and before the Font Size row:

```rust
// Font family row (after theme toggle row)
.child(
    div().flex().flex_col()
        .child(
            div().flex().flex_row().items_center().py(px(14.)).px(px(32.))
                .child(
                    div().flex_1().flex().flex_col().gap(px(3.))
                        .child(div().text_size(px(13.)).text_color(rgb(c.text_default)).child("Font Family"))
                        .child(div().text_size(px(11.)).text_color(rgb(c.text_muted))
                            .child("UI font used across the application."))
                )
                .child(div().w(px(240.)).child(self.render_font_picker(cx)))
        )
        .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
)
```

Note: `render_font_picker` borrows `self` mutably, so it must be called before other `&self` reads in the same method. Move the `let c = ...` and `let is_dark = ...` bindings before any calls to `render_font_picker`.

- [ ] **Step 4: Add `SharedString` to imports**

Add `SharedString` to the gpui imports line:

```rust
use gpui::{Context, FocusHandle, Focusable, IntoElement, Render, SharedString, Window,
           div, prelude::*, px, rgb};
```

- [ ] **Step 5: Build**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

- [ ] **Step 6: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs
git commit -m "feat(settings): font picker — system font list, renders each name in its own font"
```

---

### Task 7: Inventory category

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`

- [ ] **Step 1: Add `render_inventory` method**

```rust
fn render_inventory(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let c              = cx.global::<ThemeHandle>().0.clone();
    let stock_focused  = self.low_stock.read(cx).focus_handle.is_focused(window);
    let unit_focused   = self.default_unit.read(cx).focus_handle.is_focused(window);

    div().flex().flex_col()
        .child(Self::render_row(
            "Low Stock Threshold",
            "Alert when stock quantity falls at or below this number.",
            text_field("", self.low_stock.clone(), stock_focused, cx),
            &c,
        ))
        .child(Self::render_row(
            "Default Stock Unit",
            "Unit label used when adding new products (e.g. pcs, kg, L).",
            text_field("", self.default_unit.clone(), unit_focused, cx),
            &c,
        ))
}
```

- [ ] **Step 2: Add `Inventory` arm to match in render()**

```rust
SettingsCategory::Inventory  => self.render_inventory(window, cx),
```

- [ ] **Step 3: Build and test**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
cargo test -p vassl-app 2>&1 | tail -5
```

Expected: both pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs
git commit -m "feat(settings): Inventory category — low stock threshold + default unit"
```

---

### Task 8: Price Book category — currency select + rate + margin

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`

- [ ] **Step 1: Add `render_setting_select` helper**

This renders the Zed-style inline select for any fixed-option string setting:

```rust
fn render_setting_select(
    &mut self,
    id:       &'static str,
    select:   SettingSelect,
    current:  &str,
    options:  &[(&'static str, &'static str)],  // (value, label)
    on_pick:  impl Fn(&mut Self, &'static str, &mut Context<Self>) + 'static,
    cx:       &mut Context<Self>,
) -> impl IntoElement {
    let c       = cx.global::<ThemeHandle>().0.clone();
    let is_open = self.open_select == Some(select);
    let label   = options.iter()
        .find(|(v, _)| *v == current)
        .map(|(_, l)| *l)
        .unwrap_or(current);
    let options_owned: Vec<(&'static str, &'static str)> = options.to_vec();
    let current_owned = current.to_string();

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
                .child(div().flex_1().text_size(px(12.)).text_color(rgb(c.text_default)).child(label))
                .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("◇"))
        )
        .when(is_open, move |d| {
            d.child(
                div().id(format!("{id}-list"))
                    .mt(px(2.))
                    .bg(rgb(c.surface_default)).rounded(px(4.))
                    .border_1().border_color(rgb(c.surface_active))
                    .children(options_owned.iter().map(|(val, lbl)| {
                        let val2 = *val;
                        let selected = *val == current_owned.as_str();
                        let bg = if selected { c.surface_active } else { c.surface_default };
                        div()
                            .id(format!("{id}-opt-{val}"))
                            .px(px(10.)).py(px(6.))
                            .bg(rgb(bg)).cursor_pointer()
                            .text_size(px(12.)).text_color(rgb(c.text_default))
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
```

- [ ] **Step 2: Add `render_pricebook` method**

```rust
fn render_pricebook(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let c            = cx.global::<ThemeHandle>().0.clone();
    let rate_focused   = self.usd_to_jmd.read(cx).focus_handle.is_focused(window);
    let margin_focused = self.default_margin.read(cx).focus_handle.is_focused(window);

    let currency_select = self.render_setting_select(
        "settings-currency",
        SettingSelect::Currency,
        &self.currency.clone(),
        &[("USD", "USD — US Dollar"), ("JMD", "JMD — Jamaican Dollar")],
        |this, val, cx| {
            this.currency = val.to_string();
            this.save_setting("pricebook.currency", val.to_string(), cx);
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
                                .child(div().text_size(px(13.)).text_color(rgb(c.text_default)).child("Default Currency"))
                                .child(div().text_size(px(11.)).text_color(rgb(c.text_muted))
                                    .child("Currency used on quotations and price book entries."))
                        )
                        .child(div().w(px(240.)).child(currency_select))
                )
                .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
        )
        .child(Self::render_row(
            "USD → JMD Rate",
            "Conversion rate applied when displaying prices in JMD.",
            text_field("", self.usd_to_jmd.clone(), rate_focused, cx),
            &c,
        ))
        .child(Self::render_row(
            "Default Margin %",
            "Pre-filled margin percentage when creating new price book entries.",
            text_field("", self.default_margin.clone(), margin_focused, cx),
            &c,
        ))
}
```

- [ ] **Step 3: Add `PriceBook` arm to the match in render()**

```rust
SettingsCategory::PriceBook  => self.render_pricebook(window, cx),
```

- [ ] **Step 4: Build**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

- [ ] **Step 5: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs
git commit -m "feat(settings): Price Book category — currency select, USD→JMD rate, margin"
```

---

### Task 9: Quotations category + final wiring

**Files:**
- Modify: `crates/vassl-app/src/settings_panel.rs`

- [ ] **Step 1: Add `render_quotations` method**

```rust
fn render_quotations(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let c              = cx.global::<ThemeHandle>().0.clone();
    let prefix_focused = self.quote_prefix.read(cx).focus_handle.is_focused(window);
    let tax_focused    = self.tax_rate.read(cx).focus_handle.is_focused(window);
    let notes_focused  = self.notes_template.read(cx).focus_handle.is_focused(window);

    div().flex().flex_col()
        .child(Self::render_row(
            "Quote Number Prefix",
            "Prefix for auto-generated quotation reference numbers (e.g. VASSL-2026-0001).",
            text_field("", self.quote_prefix.clone(), prefix_focused, cx),
            &c,
        ))
        .child(Self::render_row(
            "Default Tax / VAT %",
            "Pre-filled tax rate on new quotations. Set to 0 to disable.",
            text_field("", self.tax_rate.clone(), tax_focused, cx),
            &c,
        ))
        .child(Self::render_row(
            "Default Notes Template",
            "Text pre-filled in the Notes field on new quotations.",
            text_field("", self.notes_template.clone(), notes_focused, cx),
            &c,
        ))
}
```

- [ ] **Step 2: Replace the `_ =>` fallback arm with the Quotations arm**

```rust
SettingsCategory::Quotations => self.render_quotations(window, cx),
```

The match is now exhaustive — remove the `_ =>` arm entirely.

- [ ] **Step 3: Build and run all tests**

```bash
cargo build 2>&1 | grep -E "^error|Finished"
cargo test 2>&1 | tail -8
```

Expected: `Finished` and `test result: ok. N passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add crates/vassl-app/src/settings_panel.rs
git commit -m "feat(settings): Quotations category — prefix, tax rate, notes template; settings panel complete"
```

---

## Self-Review Checklist

**Spec coverage:**
- ✅ `OpenSettings` action + `secondary-,` keybinding — Task 1
- ✅ `ActiveModule::Settings`, sidebar ⚙ wired — Task 1
- ✅ `SettingsPanel` entity with category nav — Task 2
- ✅ All 13 DB keys loaded with fallbacks, `save_setting` helper — Task 3
- ✅ Auto-save TextInputs via `cx.observe().detach()` — Task 3
- ✅ General: User Name + Company Name — Task 4
- ✅ Appearance: Theme toggle (live `ThemeHandle`) — Task 5
- ✅ Appearance: Font size stepper with `↺` reset, `window.set_rem_size` — Task 5
- ✅ Apply font size + theme from DB on app startup — Task 5
- ✅ Font picker: system fonts via `cx.text_system().all_font_names()`, rendered in own font — Task 6
- ✅ Inventory: Low stock threshold + default unit — Task 7
- ✅ Price Book: Currency select (USD/JMD), USD→JMD rate, margin — Task 8
- ✅ Quotations: Prefix, tax rate, notes template — Task 9

**Type consistency:**
- `SettingsCategory` defined in Task 2, used consistently in Tasks 4–9 ✅
- `SettingSelect` defined in Task 2, used in Tasks 6 + 8 ✅
- `save_setting(key: &'static str, value: String, cx)` defined in Task 3, called with string literals throughout ✅
- `render_row(title, desc, control, c)` defined in Task 4, used in Tasks 4, 7, 8, 9 ✅
- `render_general(window, cx)`, `render_appearance(window, cx)` etc all take `(window, cx)` ✅
