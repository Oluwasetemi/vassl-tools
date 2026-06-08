// Suppress the console window on Windows in release builds.
// Debug builds keep it so that tracing output is visible during development.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod about_dialog;
mod actions;
mod assets;
mod auto_update;
mod app;
mod app_menus;
mod audit_log;
mod command_palette;
mod first_run;
mod global_search;
mod importer;
mod keybindings;
mod platform;
mod root;
mod settings_panel;
mod sidebar;
mod status_bar;

use actions::{CheckForUpdates, ConfirmSelection, DecreaseFontSize, EscapeModal, FocusSearch, Hide, HideOthers, IncreaseFontSize, InstallUpdate, Minimize, OpenAuditLog, OpenGlobalSearch, OpenInventory, OpenPriceBook, OpenQuotations, OpenSuppliers, OpenSettings, Quit, SelectNext, SelectPrev, ShowAll};
use vassl_ui::NewRecord;
use vassl_ui::text_input::{BackTab, Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft, SelectRight, ShowCharacterPalette, Tab as TextTab};
use vassl_inventory::product_form::{EscapeForm as ProductEscapeForm, TabField as ProductTab, BackTabField as ProductBackTab};
use vassl_inventory::stock_form::{EscapeForm as StockEscapeForm, TabField as StockTab, BackTabField as StockBackTab};
use vassl_pricebook::price_form::{EscapeForm as PriceEscapeForm, TabField as PriceTab, BackTabField as PriceBackTab};
use vassl_suppliers::supplier_form::{EscapeForm as SupplierEscapeForm, TabField as SupplierTab, BackTabField as SupplierBackTab};
use vassl_quotations::quotation_form::{EscapeForm as QuotationEscapeForm, TabField as QuotationTab, BackTabField as QuotationBackTab};
use vassl_quotations::project_form::{EscapeForm as ProjectEscapeForm, TabField as ProjectTab, BackTabField as ProjectBackTab};
use vassl_quotations::line_item_form::{EscapeForm as LineItemEscapeForm, TabField as LineItemTab, BackTabField as LineItemBackTab};
use vassl_ui::{ThemeColors, ThemeHandle};
use app::VasslApp;
use gpui::{App, AppContext, Bounds, KeyBinding, WindowAppearance, WindowBounds, WindowOptions, px, size};
use root::VasslRoot;
use std::collections::HashMap;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

pub use keybindings::default_app_bindings;

/// Re-register all keybindings, applying any user overrides to remappable bindings.
pub fn apply_keybindings(cx: &mut App, overrides: &HashMap<String, String>) {
    cx.clear_key_bindings();
    cx.bind_keys([
        // App-level shortcuts — "secondary" maps to Cmd on macOS, Ctrl on Windows/Linux
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-m", Minimize, Some("VasslRoot")),
        // Remappable navigation shortcuts
        KeyBinding::new(
            overrides.get("vassl::OpenInventory").map(|s| s.as_str()).unwrap_or("secondary-1"),
            OpenInventory, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::OpenQuotations").map(|s| s.as_str()).unwrap_or("secondary-2"),
            OpenQuotations, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::OpenPriceBook").map(|s| s.as_str()).unwrap_or("secondary-3"),
            OpenPriceBook, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::OpenSuppliers").map(|s| s.as_str()).unwrap_or("secondary-4"),
            OpenSuppliers, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::OpenAuditLog").map(|s| s.as_str()).unwrap_or("secondary-shift-a"),
            OpenAuditLog, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::NewRecord").map(|s| s.as_str()).unwrap_or("secondary-n"),
            NewRecord, None,
        ),
        KeyBinding::new(
            overrides.get("vassl::FocusSearch").map(|s| s.as_str()).unwrap_or("secondary-f"),
            FocusSearch, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::OpenGlobalSearch").map(|s| s.as_str()).unwrap_or("secondary-shift-f"),
            OpenGlobalSearch, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::OpenSettings").map(|s| s.as_str()).unwrap_or("secondary-,"),
            OpenSettings, Some("VasslRoot"),
        ),
        KeyBinding::new(
            overrides.get("vassl::IncreaseFontSize").map(|s| s.as_str()).unwrap_or("secondary-="),
            IncreaseFontSize, Some("VasslRoot"),
        ),
        KeyBinding::new("secondary-shift-=", IncreaseFontSize, Some("VasslRoot")),
        KeyBinding::new(
            overrides.get("vassl::DecreaseFontSize").map(|s| s.as_str()).unwrap_or("secondary--"),
            DecreaseFontSize, Some("VasslRoot"),
        ),
        KeyBinding::new("secondary-shift-u", CheckForUpdates, Some("VasslRoot")),
        KeyBinding::new("secondary-shift-i", InstallUpdate,   Some("VasslRoot")),
        // TextInput editing keys (non-remappable)
        KeyBinding::new("backspace",        Backspace,   Some("TextInput")),
        KeyBinding::new("delete",           Delete,      Some("TextInput")),
        KeyBinding::new("left",             Left,        Some("TextInput")),
        KeyBinding::new("right",            Right,       Some("TextInput")),
        KeyBinding::new("shift-left",       SelectLeft,  Some("TextInput")),
        KeyBinding::new("shift-right",      SelectRight, Some("TextInput")),
        KeyBinding::new("secondary-a",      SelectAll,   Some("TextInput")),
        KeyBinding::new("home",             Home,        Some("TextInput")),
        KeyBinding::new("end",              End,         Some("TextInput")),
        KeyBinding::new("secondary-v",      Paste,       Some("TextInput")),
        KeyBinding::new("secondary-c",      Copy,        Some("TextInput")),
        KeyBinding::new("secondary-x",      Cut,              Some("TextInput")),
        KeyBinding::new("tab",               TextTab,              Some("TextInput")),
        KeyBinding::new("shift-tab",         BackTab,              Some("TextInput")),
        KeyBinding::new("ctrl-cmd-space",    ShowCharacterPalette, Some("TextInput")),
        // Escape closes overlays
        KeyBinding::new("escape",            EscapeModal,      Some("VasslRoot")),
        // CommandPalette keyboard navigation
        KeyBinding::new("down",              SelectNext,       Some("VasslRoot")),
        KeyBinding::new("up",                SelectPrev,       Some("VasslRoot")),
        KeyBinding::new("down",              SelectNext,       Some("CommandPalette")),
        KeyBinding::new("up",                SelectPrev,       Some("CommandPalette")),
        KeyBinding::new("enter",             ConfirmSelection, Some("CommandPalette")),
        // GlobalSearch keyboard navigation
        KeyBinding::new("down",              SelectNext,       Some("GlobalSearch")),
        KeyBinding::new("up",                SelectPrev,       Some("GlobalSearch")),
        KeyBinding::new("enter",             ConfirmSelection, Some("GlobalSearch")),
        // ProductForm escape + tab
        KeyBinding::new("escape",            ProductEscapeForm, Some("ProductForm")),
        KeyBinding::new("tab",               ProductTab,        Some("ProductForm")),
        KeyBinding::new("shift-tab",         ProductBackTab,    Some("ProductForm")),
        // StockEntryForm escape + tab
        KeyBinding::new("escape",            StockEscapeForm,  Some("StockEntryForm")),
        KeyBinding::new("tab",               StockTab,         Some("StockEntryForm")),
        KeyBinding::new("shift-tab",         StockBackTab,     Some("StockEntryForm")),
        // PriceEntryForm escape + tab
        KeyBinding::new("escape",            PriceEscapeForm,  Some("PriceEntryForm")),
        KeyBinding::new("tab",               PriceTab,         Some("PriceEntryForm")),
        KeyBinding::new("shift-tab",         PriceBackTab,     Some("PriceEntryForm")),
        // SupplierForm escape + tab
        KeyBinding::new("escape",    SupplierEscapeForm, Some("SupplierForm")),
        KeyBinding::new("tab",       SupplierTab,        Some("SupplierForm")),
        KeyBinding::new("shift-tab", SupplierBackTab,    Some("SupplierForm")),
        // QuotationForm escape + tab
        KeyBinding::new("escape",    QuotationEscapeForm, Some("QuotationForm")),
        KeyBinding::new("tab",       QuotationTab,        Some("QuotationForm")),
        KeyBinding::new("shift-tab", QuotationBackTab,    Some("QuotationForm")),
        // ProjectForm escape + tab
        KeyBinding::new("escape",            ProjectEscapeForm,   Some("ProjectForm")),
        KeyBinding::new("tab",               ProjectTab,          Some("ProjectForm")),
        KeyBinding::new("shift-tab",         ProjectBackTab,      Some("ProjectForm")),
        // LineItemForm escape + tab
        KeyBinding::new("escape",            LineItemEscapeForm,  Some("LineItemForm")),
        KeyBinding::new("tab",               LineItemTab,         Some("LineItemForm")),
        KeyBinding::new("shift-tab",         LineItemBackTab,     Some("LineItemForm")),
    ]);
}

fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    // PANIC: all three expects below are unrecoverable startup failures —
    // there is no fallback path for missing OS data dirs or a broken log appender.
    let log_dir = dirs::data_local_dir()
        .expect("OS has no local data directory (required for log storage)")
        .join("VASSL")
        .join("logs");
    std::fs::create_dir_all(&log_dir).expect("could not create VASSL log directory");

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("vassl")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("could not initialise rolling log appender");

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stdout).pretty())
        .init();

    guard
}

fn main() {
    let _tracing_guard = init_tracing();
    // Bridge log::warn! / log::error! from GPUI into our tracing pipeline.
    tracing_log::LogTracer::init().ok();
    tracing::info!("VASSL starting");

    gpui_platform::application()
        .with_assets(assets::VasslAssets)
        .run(|cx: &mut App| {
        if let Err(e) = vassl_db::init(cx) {
            tracing::error!("DB init failed: {e:?}");
            cx.quit();
            return;
        }

        let _app_state = VasslApp::new(cx);

        vassl_inventory::init(cx);
        vassl_quotations::init(cx);
        vassl_pricebook::init(cx);
        vassl_suppliers::init(cx);

        // Initialize theme based on current OS appearance.
        let initial_dark = matches!(
            cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        cx.set_global(ThemeHandle(if initial_dark { ThemeColors::dark() } else { ThemeColors::light() }));

        cx.activate(true);

        // App-level menu actions (no window context needed)
        cx.on_action(|_: &Quit,       cx| cx.quit());
        #[cfg(target_os = "macos")]
        cx.on_action(|_: &Hide,       cx| cx.hide());
        #[cfg(target_os = "macos")]
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        #[cfg(target_os = "macos")]
        cx.on_action(|_: &ShowAll,    cx| cx.unhide_other_apps());

        cx.set_menus(app_menus::app_menus());

        // Load any persisted keymap overrides from the settings DB.
        let keymap_overrides: HashMap<String, String> = {
            let db = vassl_db::AppDatabase::global(cx);
            keybindings::default_app_bindings()
                .iter()
                .filter_map(|(action_name, _default, _label)| {
                    let db_key = format!("keymap.{action_name}");
                    vassl_db::shared::get_setting(db, &db_key)
                        .ok()
                        .flatten()
                        .map(|v| (action_name.to_string(), v))
                })
                .collect()
        };

        // Register all keybindings (remappable ones with overrides applied).
        apply_keybindings(cx, &keymap_overrides);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);

        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some(platform::app_name().to_string()),
                // Hide the native Win32 title bar on Windows so GPUI draws custom caption
                // buttons that match the app theme without the DWM compositing delay.
                // macOS uses the system titlebar by default (appears_transparent=false).
                #[cfg(target_os = "windows")]
                titlebar: Some(gpui::TitlebarOptions {
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| VasslRoot::new(window, cx))
            },
        ) {
            Ok(_handle) => {}
            Err(e) => {
                tracing::error!("failed to open main window: {e:?}");
                cx.quit();
            }
        }
    });
}
