# Rust Architectural Patterns for VASSL — Research Report

Patterns extracted from: Ripgrep, Helix, Alacritty, SQLx/rusqlite.

---

## 1. Ripgrep — `BurntSushi/ripgrep`

### Workspace layout
Root is the *binary* crate; reusable engine logic is split into sibling crates under `crates/`.

```toml
[[bin]]
path = "crates/core/main.rs"
name = "rg"

[workspace]
members = [
  "crates/globset", "crates/grep", "crates/cli", "crates/matcher",
  "crates/pcre2", "crates/printer", "crates/regex", "crates/searcher",
  "crates/ignore",
]
```

- `crates/core/` — binary's private code: `main.rs`, `flags/`, `search.rs`, `haystack.rs`. Not a published library — just app shell.
- Each leaf crate has one responsibility.

### Error handling
- Binary uses `anyhow::Result` everywhere.
- Sub-crates define their own `Error` enums (hand-written, not `thiserror`).
- `main()` walks `err.chain()` and `downcast_ref::<std::io::Error>()` to special-case `BrokenPipe` for graceful exit.

### CLI — the "LowArgs / HiArgs" two-layer pattern
`crates/core/flags/` splits arg handling into two layers:

- **`LowArgs`** (`lowargs.rs`) — flat struct of raw, validated-in-isolation fields. Populated by `parse.rs` using `lexopt`.
- **`HiArgs`** (`hiargs.rs`) — "compiled" form built from `LowArgs`. Owns constructed objects and acts as a factory: `args.walk_builder()`, `args.matcher()`, `args.searcher()`, `args.printer()`.
- `mod.rs` defines a `trait Flag` for each flag — one impl per flag, easier to extend than a giant `clap` derive struct.

**Engine vs UI separation:** `main.rs` is ~600 lines of orchestration. Output goes through a `Printer<W: WriteColor>` abstraction. No engine code knows about terminals or stdout.

---

## 2. Helix — `helix-editor/helix`

### Crate layout (14 crates, one responsibility each)

| Crate | Role |
|---|---|
| `helix-core` | Pure data: ropes, selections, transactions, syntax, indent. No I/O, no UI. |
| `helix-view` | Editor state: `Editor`, `Document`, `View`, `Tree`, `Theme`, `Registers`. |
| `helix-term` | The TUI binary: `application.rs`, `commands.rs`, `keymap.rs`, `compositor.rs`, `ui/`. |
| `helix-tui` | Forked ratatui — rendering primitives. |
| `helix-lsp` / `helix-dap` | Protocol clients. |
| `helix-event` | App-wide pub/sub bus with `runtime_local!` and `dispatch()`. |
| `helix-loader` | Config dir resolution, runtime files, grammar fetching. |
| `helix-stdx` | Std-lib extensions (paths, ropes). |

`default-members = ["helix-term"]` so `cargo run` from root just runs the editor.

### Core ↔ View ↔ Term boundary
- `helix-core` knows nothing about `helix-view` (one-way dependency).
- `helix-view::Editor` owns the editable world and crosses into core only via owned data types (`Rope`, `Selection`, `Transaction`).
- `helix-term::Application` is the top-level holder:
  ```rust
  pub struct Application {
      compositor: Compositor,
      terminal: Terminal,
      pub editor: Editor,
      config: Arc<ArcSwap<Config>>,   // live config reload
      signals: Signals,
      jobs: Jobs,                      // async task queue
      lsp_progress: LspProgressMap,
  }
  ```

### Commands & keybindings
- `commands.rs`: `enum MappableCommand { Static { name, fun: fn(&mut Context), doc }, Typable {...}, Macro {...} }`
- All built-ins via a `static_commands!` macro expanding `(ident, "doc literal")` pairs. One central place to add a command.
- `keymap.rs`: `KeyTrieNode { map: IndexMap<KeyEvent, KeyTrie> }` — prefix trie. `default::default()` builds default bindings; user config merges on top via `KeyTrieNode::merge`.
- `MappableCommand` impls `Deserialize` by parsing strings, so config TOML "just works".

### Async in a TUI — the `Jobs` + `Callback` pattern
```rust
type Callback = Box<dyn FnOnce(&mut Editor, &mut Compositor) + Send>;
type JobFuture = BoxFuture<'static, anyhow::Result<Option<Callback>>>;

pub struct Jobs {
    futures: FuturesUnordered<JobFuture>,
    callback_rx: Receiver<Callback>,
    status_rx: Receiver<StatusMessage>,
}
```

Background futures return a `Callback` closure. The UI thread drains the channel each frame and applies callbacks synchronously. **The cleanest pattern for "do work off-thread, then mutate UI" in Rust.**

`runtime_local!` static `JOB_QUEUE: OnceCell<Sender<Callback>>` provides `dispatch()`/`dispatch_blocking()` from anywhere.

### Compositor (`helix-term/src/compositor.rs`)
```rust
pub trait Component {
    fn handle_event(&mut self, _event: &Event, _ctx: &mut Context) -> EventResult;
    fn render(&mut self, area: Rect, frame: &mut Surface, ctx: &mut Context);
}
pub enum EventResult { Ignored(Option<Callback>), Consumed(Option<Callback>) }
pub struct Compositor { layers: Vec<Box<dyn Component>>, area: Rect }
```
Layered components, top-most consumes events first. Maps onto GPUI's modal overlay system.

### Logging
`setup_logging` uses `fern` + `chrono`. For new apps use `tracing` instead (see Bonus section).

---

## 3. Alacritty — `alacritty/alacritty`

### Crate split
```
alacritty/                   # binary: window, input, rendering, config, IPC
alacritty_terminal/          # library: PTY, grid, ANSI parsing — zero UI knowledge
alacritty_config/            # config trait + reader
alacritty_config_derive/     # proc-macro for #[derive(ConfigDeserialize)]
```
`alacritty_terminal` has zero windowing/OpenGL knowledge — pure terminal emulator state machine.

### Main / event loop (`alacritty/src/event.rs`)
```rust
pub struct Processor {
    pub config_monitor: Option<ConfigMonitor>,
    clipboard: Clipboard,
    scheduler: Scheduler,
    windows: HashMap<WindowId, WindowContext>,
    proxy: EventLoopProxy<Event>,
    config: Rc<UiConfig>,
}
```
`Processor` implements winit's `ApplicationHandler`. Per-window state in `HashMap<WindowId, WindowContext>`. Background threads wake the UI thread via `EventLoopProxy<Event>`.

**`Scheduler`** (`alacritty/src/scheduler.rs`): `{ timers, proxy }` for "fire event in N ms" with `TimerId` + `Topic` for cancellation. Useful pattern for debouncing autosave.

### Cross-platform window handling
- All windowing through `winit` 0.30 + `glutin` for GL context.
- Platform-specific code confined to `alacritty/src/macos/`, `#[cfg(unix)] mod polling`, `#[cfg(windows)] mod panic`.
- **Don't scatter `#[cfg(...)]` across business modules — extract platform code into its own file.**

### Config persistence
`alacritty/src/config/mod.rs`:
- Custom `Error` enum, hand-rolled with manual `impl From<...>` for each variant.
- Format: TOML (`toml` + `toml_edit`). YAML kept only for legacy migration.
- `#[derive(ConfigDeserialize)]` (their own macro) gives partial-merge + unknown-key warnings.
- Hot reload: `ConfigMonitor` using `notify` watches the file and fires a custom winit event via `EventLoopProxy`.

---

## 4. SQLx & rusqlite — Database Access Patterns

### Migrations — the `sqlx::migrate!` macro

```
project/
├── Cargo.toml
└── migrations/
    ├── 20260530120000_init.sql
    └── 20260530120500_add_users.sql
```

```rust
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();  // embedded at compile time
// at startup:
MIGRATOR.run(&pool).await?;
```

Migrations **embedded in the binary at compile time** — critical for single-binary distribution.

**Gotcha:** add `build.rs` with `println!("cargo:rerun-if-changed=migrations");`.
**Line endings:** `.gitattributes` with `*.sql text eol=lf` keeps migration hashes stable across Windows/Unix.

### SQLite connection strategy (single-user GPUI app)
Three options:

1. **`Arc<Mutex<rusqlite::Connection>>`** — simplest, no extra deps. Wrap blocking calls in `tokio::task::spawn_blocking`.
2. **`r2d2_sqlite::SqliteConnectionManager` + `r2d2::Pool`** (max_size 4–8). Must set `.idle_timeout(None)` and `.max_lifetime(None)` to avoid WAL corruption.
3. **`tokio-rusqlite`** — connection lives on a dedicated thread, messages cross via channels. Best fit for async GPUI handlers.

Always apply PRAGMAs via `with_init`:
```rust
c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")
```

### Query maintainability
- **sqlx**: `query!` / `query_as!` macros for compile-time SQL verification.
- **rusqlite**: keep SQL in `const QUERY: &str = "..."` at top of each module; use `.prepare_cached(sql)` for statement reuse; `named_params!` macro for clarity.
- **Repository pattern**: one module per business domain (`db/inventory.rs`, `db/quotations.rs`, `db/pricebook.rs`), each exposing typed functions: `pub fn list_active(conn: &Connection) -> Result<Vec<Product>>`.

---

## Bonus — Cross-cutting Patterns

### `Result` propagation
- **App boundary**: `anyhow::Result` with `.context("...")`.
- **Sub-crates**: `thiserror`-derived typed errors with `#[source]` / `#[from]`.
- **Recover specifics**: `err.chain().find_map(|e| e.downcast_ref::<MyError>())`.

### Testing layout
- **Unit tests**: `#[cfg(test)] mod tests { ... }` at the bottom of each file.
- **Integration tests**: top-level `tests/` (ripgrep: `tests/tests.rs` + `tests/util.rs` + `tests/data/` corpus).
- **Test crate per concern**: helix uses `helix-term/tests/test/` with subdir-per-feature.
- **Test backend swapping**: `feature = "integration"` cargo feature + `cfg(feature)` swaps the terminal backend. Cleaner than mocking.

### Logging (modern recipe for 2026)
```rust
fn init_tracing(log_dir: &Path) -> WorkerGuard {
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("vassl").filename_suffix("log")
        .max_log_files(7)
        .build(log_dir).expect("init log appender");
    let (nb, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().with_writer(nb).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stdout).pretty())
        .init();
    guard  // MUST be held for app lifetime
}
```
Use `dirs::data_local_dir().unwrap().join("VASSL/logs")` for the dir.

### Settings serialization
- **TOML** for human-edited config; `directories` / `dirs` crate for cross-platform config-dir resolution.
- `#[derive(Deserialize, Default)]` with `#[serde(default)]` on every field so partial TOMLs load cleanly.
- Hot reload: `notify` file watcher + `Arc<ArcSwap<Config>>` (helix pattern).

---

## Concrete Takeaways for VASSL

### 1. Workspace shape (helix-inspired)
```
vassl/                       # workspace root, default-members = ["vassl-app"]
├── vassl-app/               # GPUI binary: app shell, window, layout wiring
├── vassl-core/              # pure domain types: Product, Quotation, PriceEntry — no I/O, no GPUI
├── vassl-db/                # SQLite layer: connection pool, migrations/, repository modules
├── vassl-config/            # Config struct + TOML load/save + file watcher
├── vassl-inventory/         # Inventory Panel view + state entity + actions! + init(cx)
├── vassl-quotations/        # Quotations Panel view + state entity + actions! + init(cx)
├── vassl-pricebook/         # Price Book Panel view + state entity + actions! + init(cx)
├── vassl-events/            # Cross-module event bus (helix-event style)
└── xtask/                   # Build/release automation
```

### 2. Config two-layer pattern (ripgrep HiArgs/LowArgs applied to config)
- Raw deserialized `RawConfig` (from TOML) → resolved `Config` (validated, constructed)
- `Arc<ArcSwap<Config>>` for live reload without locking

### 3. Error strategy
- `thiserror` in every sub-crate, `anyhow::Result` in `vassl-app` and GPUI handlers
- Always preserve `#[source]`, special-case sentinels via `err.chain().downcast_ref()`

### 4. Top-level app state holder (helix `Application` applied to GPUI)
```rust
pub struct VasslApp {
    pub config: Arc<ArcSwap<Config>>,
    pub db: Arc<DbPool>,               // r2d2 pool, max_size 4, WAL+FK PRAGMAs
    pub jobs: Jobs,                    // background work + ui-callback channel
    pub inventory: Entity<InventoryStore>,
    pub quotations: Entity<QuotationStore>,
    pub pricebook: Entity<PriceBookStore>,
}
```

### 5. Commands (helix `MappableCommand` → GPUI `actions!`)
- One `actions!` macro call per module
- Bind in keymap JSON via string names
- No manual palette registration — GPUI's `window.available_actions()` picks them up automatically

### 6. Async → UI mutation (helix `Jobs` pattern in GPUI)
```rust
// In GPUI, the equivalent of helix's Jobs:
cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
    let data = cx.background_spawn(async move { db.fetch_inventory().await }).await?;
    this.update(cx, |panel, cx| {
        panel.items = data;
        cx.notify();   // NEVER forget this
    })
}).detach();
```

### 7. DB layer
- `vassl-db/migrations/` + embedded via `sqlx::migrate!()` OR `rusqlite_migration` crate
- `build.rs`: `println!("cargo:rerun-if-changed=migrations");`
- `.gitattributes`: `*.sql text eol=lf`
- One repository module per module: `db::inventory::list_active(conn)`, `db::quotations::create(conn, input)`
- WAL + FK PRAGMAs via `with_init`

### 8. Platform code isolation
- All `#[cfg(target_os = "...")]` confined to `vassl-app/src/platform/{macos,windows}.rs`
- Zero `cfg` in business logic or domain crates

### 9. Testing
- `#[cfg(test)] mod tests` per file for unit tests
- `vassl-app/tests/` for integration tests with `feature = "integration"` swapping GPUI for headless harness

### 10. Logging
- `tracing` + `tracing-appender` with daily rotation, 7-day retention
- `WorkerGuard` held in `main` for the app lifetime
