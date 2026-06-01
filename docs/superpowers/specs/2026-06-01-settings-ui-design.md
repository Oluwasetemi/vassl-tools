# Settings UI Implementation Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Settings panel to VASSL with per-module configuration, live theme/font switching, and a full system font picker.

**Architecture:** Settings opens as `ActiveModule::Settings` — the ⚙ sidebar button switches to it. `SettingsPanel` is a single entity in `vassl-app` with an internal category nav (left) and settings rows (right). All settings persist to the existing `settings` key/value DB table via `vassl_db::shared::set_setting`.

**Tech Stack:** GPUI entity/render pattern, `vassl-db` shared settings table, `cx.text_system().all_font_names()` for font enumeration, `cx.set_global(ThemeHandle(...))` for live theme propagation.

---

## Files

**Create:**
- `crates/vassl-app/src/settings_panel.rs` — `SettingsPanel` entity: category nav, all setting rows, font picker state, stepper logic

**Modify:**
- `crates/vassl-app/src/sidebar.rs` — add `ActiveModule::Settings` variant, wire ⚙ button click
- `crates/vassl-app/src/root.rs` — add `settings_panel: Entity<SettingsPanel>` field, render it when active
- `crates/vassl-app/src/actions.rs` — add `OpenSettings` action
- `crates/vassl-app/src/main.rs` — add `OpenSettings` keybinding (`secondary-,`)

---

## Settings Categories & Keys

All keys stored as `TEXT` in `settings` table. Missing keys fall back to hardcoded defaults at load time.

### General
| Setting | DB Key | Type | Default |
|---|---|---|---|
| User Name | `general.user_name` | string | falls back to `current_user` key, then `""` |
| Company Name | `general.company_name` | string | `""` |

### Appearance
| Setting | DB Key | Type | Default |
|---|---|---|---|
| Theme | `appearance.theme` | `"dark"` \| `"light"` | `"dark"` |
| UI Font Family | `appearance.font_family` | string (font name) | `"system-ui"` |
| UI Font Size | `appearance.font_size` | float string | `"13"` |

### Inventory
| Setting | DB Key | Type | Default |
|---|---|---|---|
| Low Stock Threshold | `inventory.low_stock_threshold` | integer string | `"5"` |
| Default Stock Unit | `inventory.default_unit` | string | `"pcs"` |

### Price Book
| Setting | DB Key | Type | Default |
|---|---|---|---|
| Default Currency | `pricebook.currency` | `"USD"` \| `"JMD"` | `"USD"` |
| USD → JMD Rate | `pricebook.usd_to_jmd_rate` | float string | `"157.50"` |
| Default Margin % | `pricebook.default_margin` | float string | `"20"` |

### Quotations
| Setting | DB Key | Type | Default |
|---|---|---|---|
| Quote Number Prefix | `quotations.prefix` | string | `"VASSL"` |
| Default Tax/VAT % | `quotations.tax_rate` | float string | `"0"` |
| Default Notes Template | `quotations.notes_template` | string | `""` |

---

## SettingsPanel Struct

```rust
pub struct SettingsPanel {
    active_category:   SettingsCategory,
    font_names:        Vec<SharedString>,
    open_select:       Option<SettingSelect>,  // which select is open, if any

    // General
    user_name:         Entity<TextInput>,
    company_name:      Entity<TextInput>,

    // Appearance
    theme:             String,           // "dark" | "light"
    font_family:       String,
    font_size:         f64,              // 10.0–24.0, step 0.5

    // Inventory
    low_stock:         Entity<TextInput>,
    default_unit:      Entity<TextInput>,

    // Price Book
    currency:          String,           // "USD" | "JMD"
    usd_to_jmd:        Entity<TextInput>,
    default_margin:    Entity<TextInput>,

    // Quotations
    quote_prefix:      Entity<TextInput>,
    tax_rate:          Entity<TextInput>,
    notes_template:    Entity<TextInput>,

    focus_handle:      FocusHandle,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SettingsCategory {
    General, Appearance, Inventory, PriceBook, Quotations,
}

/// Identifies which inline select list is currently open.
#[derive(Clone, Copy, PartialEq)]
pub enum SettingSelect { Theme, Currency, FontPicker }
```

---

## Controls

### Setting Row Layout
```
┌─────────────────────────────────────────────────────────┐
│ Title (13px, text_default, semibold)        [Control]   │
│ Description (11px, text_muted)                          │
├─────────────────────────────────────────────────────────┤
```
Thin `h(1)` `surface_default` divider between rows. Rows have `py(px(14.))` padding.

### Toggle Switch
- 32×18px rounded pill
- Thumb: 14×14px circle, slides left/right
- On: `surface_active` fill; Off: `surface_default` fill
- `on_mouse_down` flips bool, saves immediately, triggers theme global if applicable

### Setting Select
- Same visual as `Dropdown` trigger: `◇` indicator, `surface_default` bg, `surface_active` border
- Backed by `Vec<(&str, &str)>` (value, display label)
- State stored as `String` on `SettingsPanel` directly (not `Entity<Dropdown>`)
- Opens inline list with `.when(open, ...)` — no z-index issues

### Stepper (Font Size)
- Layout: `↺` reset icon next to label | `−` `16.00` `+` on right
- `−` / `+` buttons: `px(10.)` × `py(6.)`, `surface_default` bg, rounded
- Value display: `px(48.)` wide, center-aligned, `surface_default` bg
- Step: 0.5px, min: 10.0, max: 24.0
- Reset `↺`: restores to `13.0`, saves immediately
- Saves on every click via `cx.spawn(db.write(...))`

### Font Picker
- Trigger button: current font name rendered in that font + `◇`
- Open: scrollable list `max_h(px(200.))`, each row shows font name rendered in that font
- `font_names` populated once in `new()` via `cx.text_system().all_font_names()`
- Selected font name saved as `appearance.font_family`

---

## Data Flow

### Loading
`SettingsPanel::new(cx)` reads all keys synchronously via `AppDatabase::global(cx)` (same pattern as `FirstRunPrompt::new`). Missing keys use defaults. Font names enumerated once: `cx.text_system().all_font_names()`.

### Saving
Private `fn save_setting(key: &'static str, value: String, cx: &mut Context<Self>)`:
```rust
let db = vassl_db::AppDatabase::global(&**cx).clone();
cx.spawn(async move |_, _| {
    db.write(move |conn| vassl_db::shared::set_setting(conn, key, value)).await
}).detach();
```
- **TextInputs**: save on blur — detected by registering `cx.on_focus_out(&text_input_focus_handle, ...)` for each field in `new()`
- **Selects, Stepper, Toggle**: save immediately on click/change

### Live Theme Propagation
On theme change: `cx.set_global(ThemeHandle(ThemeColors::light()))` or `ThemeColors::dark()`. All entities using `cx.global::<ThemeHandle>()` in `render()` re-render automatically next frame.

### Live Font Size Propagation
On stepper change: call `window.set_rem_size(px(self.font_size))`. On app startup, `VasslRoot::new()` reads `appearance.font_size` and applies it before any panels render.

---

## Layout

```
┌──────┬──────────┬────────────────────────────────────────┐
│      │ General  │ General                                 │
│ Nav  │ Appearance│ General Settings                       │
│ (48) │ Inventory │ ─────────────────────────────────────  │
│      │ Price Book│ User Name          [________input____] │
│      │ Quotations│ Your display name                      │
│      │          │ ─────────────────────────────────────  │
│      │          │ Company Name       [________input____] │
│      │          │ Used on quotations                     │
└──────┴──────────┴────────────────────────────────────────┘
```

- Left module nav: 48px (existing sidebar, unchanged)
- Category nav: 160px, `sidebar_bg`, category rows with active highlight
- Content area: flex-1, `canvas_bg`, scrollable

The content area title row shows category name (large, `text_default`) + subtitle (small, `text_muted`) matching the screenshot.

---

## Testing

Unit tests in `settings_panel.rs`:
- `default_font_size_is_13` — verify default parse
- `theme_toggle_produces_correct_key` — `"dark"` ↔ `"light"`
- `stepper_clamps_at_bounds` — step below 10 stays 10, above 24 stays 24
- `currency_select_valid_values` — only `"USD"` and `"JMD"` accepted
- `setting_key_format` — all keys follow `module.name` convention

Integration: `SettingsPanel::new()` requires GPUI context — verified via app launch.
