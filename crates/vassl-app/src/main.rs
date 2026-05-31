mod actions;
mod app;
mod audit_log;
mod colors;
mod command_palette;
mod first_run;
mod platform;
mod root;
mod sidebar;
mod status_bar;

use actions::{FocusSearch, NewRecord, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations};
use app::VasslApp;
use gpui::{App, AppContext, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size};
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

        cx.activate(true);

        // Keybindings are also documented in assets/keymaps/default.json (kept in sync manually).
        // The JSON is not loaded at runtime — cx.bind_keys is the source of truth.
        cx.bind_keys([
            KeyBinding::new("ctrl-1",       OpenInventory,  Some("VasslRoot")),
            KeyBinding::new("ctrl-2",       OpenQuotations, Some("VasslRoot")),
            KeyBinding::new("ctrl-3",       OpenPriceBook,  Some("VasslRoot")),
            KeyBinding::new("ctrl-shift-a", OpenAuditLog,   Some("VasslRoot")),
            KeyBinding::new("ctrl-n",       NewRecord,      Some("VasslRoot")),
            KeyBinding::new("ctrl-f",       FocusSearch,    Some("VasslRoot")),
        ]);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);

        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some(platform::app_name().to_string()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| VasslRoot::new(window, cx)),
        ) {
            Ok(_handle) => {}
            Err(e) => {
                tracing::error!("failed to open main window: {e:?}");
                cx.quit();
            }
        }
    });
}
