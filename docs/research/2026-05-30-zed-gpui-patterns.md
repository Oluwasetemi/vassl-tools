# Zed / GPUI Research — Patterns for VASSL

All paths below are relative to the Zed repo (`zed-industries/zed`). Line numbers are approximate but accurate within a few lines.

---

## 1. GPUI App Bootstrap

**Entry crate:** `crates/zed/src/main.rs`

1. `main.rs:202` — `fn main()` parses CLI args, sets up logging, paths, telemetry.
2. `main.rs:93` — Constructs the GPUI app: `Application::new_inaccessible(platform)`.
3. `main.rs:477` — `app.run(move |cx| { ... })` — closure receives `&mut App` after platform event loop is live.
4. Inside `run`, settings/themes/globals are initialized, then `cx.open_window(WindowOptions::default(), |window, cx| { build_root_view })`.

**Core types (all in `crates/gpui/src/app.rs` / `crates/gpui/src/app/`):**

| Type | File:Line | Role |
|---|---|---|
| `Application` | `crates/gpui/src/app.rs:140` | Pre-launch builder. `.run(FnOnce(&mut App))`. |
| `App` | `crates/gpui/src/app.rs:611` | Synchronous root context. Holds entities, globals, windows. |
| `Context<T>` | `crates/gpui/src/app/context.rs` | Per-entity context. Exposes `notify()`, `spawn()`, `observe()`, `subscribe()`, `emit()`. |
| `Window` | `crates/gpui/src/window.rs` | Per-window state — focus, dispatch tree, layout. |
| `AsyncApp` | `crates/gpui/src/app/async_context.rs:153` | Holdable across `.await`. Use `cx.to_async()` or inside `cx.spawn(async move |cx| ...)`. |

> **Note:** There is no separate `AppContext`/`WindowContext`/`ViewContext` anymore. Modern GPUI uses `App` + `Window` + `Context<T>`. Old tutorials using those names are outdated.

**Minimal bootstrap pattern for VASSL:**

```rust
fn main() {
    Application::new()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            theme::init(cx);
            settings::init(cx);
            cx.activate(true);
            cx.open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| VasslRoot::new(window, cx))
            }).unwrap();
        });
}
```

---

## 2. View / Element Model

**`Render` trait:** `crates/gpui/src/element.rs:147`

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

Any entity that `impl Render` becomes a "view". There is **no** `View<T>` type — both views and models share `Entity<T>`. An entity becomes a view only by also implementing `Render`.

**`div()` element builder:** `crates/gpui/src/elements/div.rs:1487`. Layout is Taffy (flexbox). Style methods come from `Styled`, `InteractiveElement`, `ParentElement` traits.

**`h_flex()` / `v_flex()` shortcuts:** `crates/ui/src/styled_ext.rs`

**Minimal custom view pattern:**

```rust
struct Hello { count: usize }

impl Render for Hello {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex().flex_col().gap_2().p_4()
            .bg(cx.theme().colors().background)
            .text_color(cx.theme().colors().text)
            .child(format!("clicks: {}", self.count))
            .child(
                div()
                    .id("btn")
                    .px_3().py_1()
                    .bg(cx.theme().colors().element_background)
                    .on_click(cx.listener(|this, _ev, _w, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
                    .child("Click me"),
            )
    }
}

// Construct:
let view = cx.new(|_cx| Hello { count: 0 });
```

---

## 3. Workspace / Pane System

**Files:** `crates/workspace/src/workspace.rs`, `pane.rs`, `pane_group.rs`, `dock.rs`

**`Workspace` struct** — `workspace.rs:1353`. Key fields:

```rust
pub struct Workspace {
    center: PaneGroup,
    left_dock: Entity<Dock>,
    bottom_dock: Entity<Dock>,
    right_dock: Entity<Dock>,
    panes: Vec<Entity<Pane>>,
    active_pane: Entity<Pane>,
    status_bar: Entity<StatusBar>,
    modal_layer: Entity<ModalLayer>,
    ...
}
```

**`PaneGroup` tree** — `pane_group.rs:30`:

```rust
pub struct PaneGroup { pub root: Member, pub is_center: bool }

pub enum Member {
    Axis(PaneAxis),        // horizontal/vertical split node
    Pane(Entity<Pane>),    // leaf
}
```

`PaneAxis` holds `members: Vec<Member>`, `axis: Axis`, and `flexes: Rc<RefCell<Vec<f32>>>` for splitter ratios.

**Splitting a pane:** `PaneGroup::split` at `pane_group.rs:59`. User-facing entry: `Pane::split` at `pane.rs:2558`.

**Rendering** — `impl Render for Workspace` at `workspace.rs:8412`: vertical flex — titlebar → main area (left dock + PaneGroup + right dock) → status bar → bottom dock.

---

## 4. Sidebar / Activity Bar

Zed does **not** have a VS Code-style activity bar. Icon buttons live in the **status bar** as `PanelButtons`.

**`PanelButtons`** — `dock.rs:356`. Constructed at `workspace.rs:1718–1720`:

```rust
let left_dock_buttons = cx.new(|cx| PanelButtons::new(left_dock.clone(), cx));
status_bar.add_left_item(left_dock_buttons, window, cx);
```

`PanelButtons` reads the dock's `panel_entries` and renders one icon per panel via `Panel::icon(window, cx) -> Option<IconName>`.

**The `Panel` trait** — `dock.rs:36`:

```rust
pub trait Panel: Focusable + EventEmitter<PanelEvent> + Render + Sized {
    fn persistent_name() -> &'static str;
    fn position(&self, window: &Window, cx: &App) -> DockPosition;
    fn icon(&self, window: &Window, cx: &App) -> Option<IconName>;
    fn icon_tooltip(&self, window: &Window, cx: &App) -> Option<&'static str>;
    fn toggle_action(&self) -> Box<dyn Action>;
    fn activation_priority(&self) -> u32;
    ...
}
```

**Panel registration:** `workspace.add_panel(panel, window, cx)` at `workspace.rs:2545`.

**For VASSL:** Three `Panel` impls (Inventory, Quotations, Price Book) returning `DockPosition::Left`. Their icons appear automatically via `PanelButtons`. For a true vertical icon rail on the left (not the bottom), write a small custom view rendered before the left dock in the workspace render.

---

## 5. Actions and Keybindings

**Definition:** `crates/gpui/src/action.rs:24`

```rust
actions!(editor, [MoveUp, MoveDown, Newline]);  // creates editor::MoveUp etc.
```

Complex actions: `#[derive(Action)]` directly with `#[action(namespace = ..., name = ...)]` attributes. Requires `Clone + PartialEq + serde::Deserialize + schemars::JsonSchema`.

**Handler registration:**
- `cx.on_action(|action: &MyAction, window, cx| { ... })` inside `render`
- `workspace.register_action(|workspace, action, window, cx| { ... })` — canonical for feature-level handlers

**Keybindings:** JSON files in `assets/keymaps/`:

```json
[
  {
    "context": "Editor",
    "bindings": {
      "cmd-shift-p": "command_palette::Toggle",
      "ctrl-w right": ["pane::SplitRight", {}]
    }
  }
]
```

Loaded via `KeymapFile::load_asset(...)` → `cx.bind_keys(...)` in `crates/zed/src/zed.rs:2139`.

---

## 6. Command Palette

**Crate:** `crates/command_palette/`. Main view: `command_palette.rs:38`

```rust
pub struct CommandPalette {
    picker: Entity<Picker<CommandPaletteDelegate>>,
}
```

Thin wrapper around generic `Picker<D: PickerDelegate>` in `crates/picker/src/picker.rs`.

**Registration & toggling** (`command_palette.rs:31`):

```rust
pub fn init(cx: &mut App) {
    cx.observe_new(CommandPalette::register).detach();
}
fn register(workspace: &mut Workspace, ...) {
    workspace.register_action(|workspace, _: &Toggle, window, cx| {
        Self::toggle(workspace, "", window, cx)
    });
}
```

**Command collection** — `command_palette.rs:105`:

```rust
let commands = window.available_actions(cx)   // GPUI walks focused dispatch chain
    .into_iter()
    .filter_map(|action| Some(Command {
        name: humanize_action_name(action.name()),
        action,
    }))
    .collect();
```

**No manual command registration needed** — define actions with `actions!`, and they appear automatically if reachable from the current focus context.

**Fuzzy matching:** `fuzzy_nucleo::{StringMatch, StringMatchCandidate}`.

---

## 7. State Management / Inter-Module Communication

**Unified `Entity<T>` model.** `crates/gpui/src/app/entity_map.rs:414`. No `Model<T>` vs `View<T>` split — both are `Entity<T>`.

```rust
// Construction
let entity = cx.new(|cx: &mut Context<T>| T { ... });

// Read
entity.read(cx) -> &T

// Write (gets fresh Context<T>)
entity.update(cx, |t, cx| { ... })

// Trigger re-render
cx.notify()
```

**Events:**
```rust
impl EventEmitter<MyEvent> for MyEntity {}
// Emit:
cx.emit(MyEvent { ... })
// Subscribe:
cx.subscribe(&other_entity, |this, other, event, cx| { ... }).detach()
```

**Observation (notify-only):**
```rust
cx.observe(&other_entity, |this, other, cx| { ... })
cx.observe_global::<MyGlobal>(|this, cx| { ... })
```

**Globals** for app-wide singletons: `cx.set_global(value)`, `cx.global::<T>()`, `cx.update_global::<T, _>(|g, cx| ...)`. Zed uses this for `SettingsStore`, theme, `AppDatabase`, etc.

**For VASSL:** Each module = entity holding domain state (`InventoryStore`, `QuotationStore`, `PriceBookStore`) + a `Panel`-implementing view that reads from it. Cross-module communication via `EventEmitter` + `cx.subscribe`.

---

## 8. Status Bar

**File:** `crates/workspace/src/status_bar.rs`

```rust
pub trait StatusItemView: Render {                    // status_bar.rs:42
    fn set_active_pane_item(&mut self, item: Option<&dyn ItemHandle>,
                            window: &mut Window, cx: &mut Context<Self>);
}

pub struct StatusBar {                                // status_bar.rs:100
    left_items: Vec<Box<dyn StatusItemViewHandle>>,
    right_items: Vec<Box<dyn StatusItemViewHandle>>,
    active_pane: Entity<Pane>,
    ...
}
```

**Adding items:**
```rust
status_bar.add_left_item(item, window, cx);
status_bar.add_right_item(item, window, cx);
```

Constructed once in `Workspace::new` (`workspace.rs:1725`). Rendered as `h_flex().w_full().justify_between()`.

---

## 9. Crate Organization

**Foundation:**
- `gpui` — UI framework core (App, Entity, Window, Element, div, Taffy, key dispatch)
- `ui` — Zed-flavored component library (Button, Label, Icon, h_flex, v_flex, ListItem, Tooltip) built on gpui
- `theme`, `theme_settings` — colors/fonts
- `settings`, `settings_macros` — JSON settings with hot reload
- `util`, `collections`, `paths` — helpers

**Persistence:**
- `db` — global SQLite wrapper, `static_connection!` macro, migration registry
- `sqlez`, `sqlez_macros` — thin async-friendly SQLite wrapper with compile-time SQL checking

**Workspace shell:**
- `workspace` — Workspace, Pane, PaneGroup, Dock, Panel, StatusBar, ModalLayer, Item trait
- `picker` — generic fuzzy picker + delegate trait
- `command_palette`, `command_palette_hooks`
- `title_bar`, `notifications`, `toast_layer`, `modal_layer`

**Feature crates** (each is independent and registers via `init(cx)` + `cx.observe_new(register)`):
- `project_panel`, `outline_panel`, `git_ui`, `terminal_view`, etc.

**The binary:** `crates/zed/` — `main.rs` + `zed.rs` (top-level `init` calls into every feature crate's `init`).

**Registration pattern:**
```rust
pub fn init(cx: &mut App) {
    cx.observe_new(MyFeature::register).detach();
}
fn register(workspace: &mut Workspace, _w: Option<&mut Window>, _cx: &mut Context<Workspace>) {
    workspace.register_action(...);
}
```

**VASSL crate structure:**
```
crates/
  vassl/            # binary: main.rs + init wiring
  vassl_ui/         # shared UI helpers, theme bridge
  vassl_db/         # sqlez-backed connection + migrations
  vassl_inventory/  # state entity + Panel view + actions! + init(cx)
  vassl_quotations/ # ditto
  vassl_pricebook/  # ditto
```

---

## 10. SQLite Usage

Zed uses SQLite via its own in-house wrapper.

**Crates:**
- `crates/sqlez/` — `connection.rs`, `thread_safe_connection.rs`, `migrations.rs`, `domain.rs`
- `crates/sqlez_macros/` — compile-time `sql!("...")` macro
- `crates/db/` — `AppDatabase` (a GPUI Global), migration registry, `static_connection!` macro

**Migration / domain pattern** (`db.rs:29–58`):

```rust
pub struct DomainMigration {
    pub name: &'static str,
    pub migrations: &'static [&'static str],
    pub dependencies: &'static [&'static str],
}
inventory::collect!(DomainMigration);
```

Each domain calls `db::static_connection!(WorkspaceDb, [])` — the macro auto-implements `Deref` to `ThreadSafeConnection` and submits migrations to the `inventory` collector at link time. On startup `AppDatabase::new()` opens one shared connection and applies all registered `DomainMigration`s in topological order.

**Async DB pattern:**

```rust
cx.spawn(async move |this, cx| {
    let items = cx.background_spawn(async move {
        conn.fetch_inventory().await
    }).await?;
    this.update(cx, |panel, cx| {
        panel.items = items;
        cx.notify();
    })
}).detach();
```

---

## 11. Theme System

**Crate:** `crates/theme/src/theme.rs`

- `pub struct Theme` at `theme.rs:208` — holds `ThemeColors`, `StatusColors`, fonts, syntax highlighting.
- Stored as `GlobalTheme` via `cx.set_global(GlobalTheme { theme, icon_theme })`.

**Access via `ActiveTheme` trait** (`theme.rs:119`):

```rust
impl ActiveTheme for App {
    fn theme(&self) -> &Arc<Theme> { GlobalTheme::theme(self) }
}
```

Inside any render method:
```rust
let colors = cx.theme().colors();
div().bg(colors.background).text_color(colors.text)
```

`ThemeColors` field list: `crates/theme/src/styles/colors.rs`.

---

## 12. Async in GPUI

**Executors** — `crates/gpui/src/executor.rs`:
- `BackgroundExecutor` (`:89`) — `Send` futures on a thread pool. For I/O, DB calls, heavy compute.
- `ForegroundExecutor` (`:314`) — `!Send` local futures on the main thread.

Get via: `cx.background_executor()` / `cx.foreground_executor()`.

**Spawn patterns:**

```rust
// Entity-scoped spawn (most common):
cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
    let data = fetch().await?;
    this.update(cx, |this, cx| {
        this.data = data;
        cx.notify();
    })
}).detach();

// Background (Send) task — no UI access:
cx.background_spawn(async move {
    expensive_computation().await
}).detach();

// App-level spawn:
cx.spawn(async move |cx: &mut AsyncApp| {
    let result = some_async_fn().await;
    cx.update(|cx| { /* sync App access */ })?;
    anyhow::Ok(())
}).detach();
```

The only way to touch `App` or any `Entity` after `.await` is `cx.update(|cx| ...)` on `AsyncApp`, or `weak_entity.update(cx, |this, cx| ...)`.

`cx.notify()` after mutating state is required — the #1 reason a UI doesn't refresh is forgetting it.

**`Task<T>`** is GPUI's future handle. Either `.detach()` (fire-and-forget) or `.await` it.

---

## Key File Map for VASSL Implementation

| Concern | File to study first |
|---|---|
| Bootstrap | `crates/zed/src/main.rs:202`, `crates/gpui/src/app.rs:140` |
| Render trait & div | `crates/gpui/src/element.rs:147`, `crates/gpui/src/elements/div.rs:1487` |
| Workspace + docks | `crates/workspace/src/workspace.rs:1353`, `dock.rs:36` (Panel trait) |
| Splittable panes | `crates/workspace/src/pane_group.rs:30`, `pane.rs:2558` |
| Status bar | `crates/workspace/src/status_bar.rs:42` (trait), `:100` (struct) |
| Panel example | `crates/project_panel/src/project_panel.rs:7213` |
| Actions | `crates/gpui/src/action.rs:24` (`actions!` macro) |
| Keymap load | `crates/zed/src/zed.rs:2139` |
| Command palette | `crates/command_palette/src/command_palette.rs:38` |
| Generic picker | `crates/picker/src/picker.rs` |
| Entity / state | `crates/gpui/src/app/entity_map.rs:414`, `context.rs:63` |
| Theme | `crates/theme/src/theme.rs:119` (ActiveTheme) |
| SQLite | `crates/db/src/db.rs:247` (`static_connection!`), `crates/sqlez/src/` |
| Async | `crates/gpui/src/app/context.rs:237` (`Context::spawn`), `executor.rs` |
