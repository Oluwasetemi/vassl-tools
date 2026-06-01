mod actions;
mod app;
mod audit_log;
mod colors;
mod command_palette;
mod first_run;
mod global_search;
mod platform;
mod root;
mod settings_panel;
mod sidebar;
mod status_bar;

use actions::{ConfirmSelection, EscapeModal, FocusSearch, NewRecord, OpenAuditLog, OpenGlobalSearch, OpenInventory, OpenPriceBook, OpenQuotations, OpenSettings, SelectNext, SelectPrev};
use vassl_ui::text_input::{BackTab, Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft, SelectRight, ShowCharacterPalette, Tab as TextTab};
use vassl_inventory::product_form::{EscapeForm as ProductEscapeForm, TabField as ProductTab, BackTabField as ProductBackTab};
use vassl_inventory::stock_form::{EscapeForm as StockEscapeForm, TabField as StockTab, BackTabField as StockBackTab};
use vassl_pricebook::price_form::{EscapeForm as PriceEscapeForm, TabField as PriceTab, BackTabField as PriceBackTab};
use vassl_suppliers::supplier_form::{EscapeForm as SupplierEscapeForm, TabField as SupplierTab, BackTabField as SupplierBackTab};
use vassl_quotations::quotation_form::EscapeForm as QuotationEscapeForm;
use vassl_quotations::project_form::{EscapeForm as ProjectEscapeForm, TabField as ProjectTab, BackTabField as ProjectBackTab};
use vassl_quotations::line_item_form::{EscapeForm as LineItemEscapeForm, TabField as LineItemTab, BackTabField as LineItemBackTab};
use vassl_ui::{ThemeColors, ThemeHandle};
use app::VasslApp;
use gpui::{App, AppContext, Bounds, KeyBinding, WindowAppearance, WindowBounds, WindowOptions, px, size};
use root::VasslRoot;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter};

fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = dirs::data_local_dir()
        .expect("no local data dir")
        .join("VASSL")
        .join("logs");
    std::fs::create_dir_all(&log_dir).expect("create log dir");

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("vassl")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .expect("init log appender");

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

    gpui_platform::application().run(|cx: &mut App| {
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

        // Keybindings are also documented in assets/keymaps/default.json (kept in sync manually).
        // The JSON is not loaded at runtime — cx.bind_keys is the source of truth.
        cx.bind_keys([
            // App-level shortcuts — "secondary" maps to Cmd on macOS, Ctrl on Windows/Linux
            KeyBinding::new("secondary-1",       OpenInventory,  Some("VasslRoot")),
            KeyBinding::new("secondary-2",       OpenQuotations, Some("VasslRoot")),
            KeyBinding::new("secondary-3",       OpenPriceBook,  Some("VasslRoot")),
            KeyBinding::new("secondary-shift-a", OpenAuditLog,   Some("VasslRoot")),
            KeyBinding::new("secondary-n",       NewRecord,      Some("VasslRoot")),
            KeyBinding::new("secondary-f",       FocusSearch,      Some("VasslRoot")),
            KeyBinding::new("secondary-shift-f", OpenGlobalSearch, Some("VasslRoot")),
            KeyBinding::new("secondary-comma",   OpenSettings,   Some("VasslRoot")),
            // TextInput editing keys
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
            // QuotationForm escape
            KeyBinding::new("escape",            QuotationEscapeForm, Some("QuotationForm")),
            // ProjectForm escape + tab
            KeyBinding::new("escape",            ProjectEscapeForm,   Some("ProjectForm")),
            KeyBinding::new("tab",               ProjectTab,          Some("ProjectForm")),
            KeyBinding::new("shift-tab",         ProjectBackTab,      Some("ProjectForm")),
            // LineItemForm escape + tab
            KeyBinding::new("escape",            LineItemEscapeForm,  Some("LineItemForm")),
            KeyBinding::new("tab",               LineItemTab,         Some("LineItemForm")),
            KeyBinding::new("shift-tab",         LineItemBackTab,     Some("LineItemForm")),
        ]);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);

        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some(platform::app_name().to_string()),
                ..Default::default()
            },
            |window, cx| {
                // Update theme when OS appearance changes.
                window.observe_window_appearance(|window, cx| {
                    let dark = matches!(
                        window.appearance(),
                        WindowAppearance::Dark | WindowAppearance::VibrantDark
                    );
                    cx.set_global(ThemeHandle(if dark { ThemeColors::dark() } else { ThemeColors::light() }));
                })
                .detach();
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
