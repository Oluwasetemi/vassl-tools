use gpui::{Context, Entity, FocusHandle, Focusable, IntoElement, PathPromptOptions, Render,
           Subscription, Window, div, prelude::*, px, rgb, rems};
#[cfg(not(target_os = "macos"))]
use gpui::{MouseButton, MouseDownEvent, OwnedMenuItem, deferred};
#[cfg(not(target_os = "macos"))]
use vassl_ui::tooltip;
use vassl_ui::{ThemeColors, ThemeHandle, RootFocusHandle};

use crate::about_dialog::{AboutDialog, AboutEvent};
use crate::actions::{About, CheckForUpdates, DecreaseFontSize, EscapeModal, FocusSearch,
                     IncreaseFontSize, InstallUpdate, Minimize, OpenAuditLog, OpenChangelog,
                     OpenGlobalSearch, OpenInventory, OpenPriceBook, OpenQuotations, OpenSuppliers,
                     OpenSettings, SelectNext, SelectPrev, Zoom};
use crate::changelog::{ChangelogPanel, ChangelogEvent};
use crate::license_dialog::{LicenseDialog, LicenseDialogEvent};
use crate::auto_update::{AutoUpdateEvent, AutoUpdater, UpdateStatus};
use vassl_ui::NewRecord;
use crate::global_search::{GlobalSearch, GlobalSearchEvent, SearchResultKind};
use crate::settings_panel::{SettingsPanel, SettingsPanelEvent};
use crate::audit_log::AuditLogPanel;
use crate::command_palette::{CommandPalette, PaletteEvent, PaletteCommand};
use crate::first_run::{FirstRunEvent, FirstRunPrompt};
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;
use vassl_inventory::panel::{InventoryPanel, InventoryPanelEvent};
use vassl_inventory::store::InventoryStoreHandle;
use vassl_pricebook::panel::{PriceBookPanel, PriceBookPanelEvent};
use vassl_pricebook::price_history::{PriceHistoryEvent, PriceHistoryPanel};
use vassl_quotations::panel::QuotationPanel;
use vassl_suppliers::{panel::SupplierPanel, store::SupplierStoreHandle};

pub struct VasslRoot {
    sidebar:          Entity<Sidebar>,
    status_bar:       Entity<StatusBar>,
    inventory_panel:  Entity<InventoryPanel>,
    pricebook_panel:  Entity<PriceBookPanel>,
    quotation_panel:  Entity<QuotationPanel>,
    suppliers_panel:  Entity<SupplierPanel>,
    settings_panel:   Entity<SettingsPanel>,
    first_run:             Option<Entity<FirstRunPrompt>>,
    _first_run_sub:        Option<Subscription>,
    audit_log:             Option<Entity<AuditLogPanel>>,
    palette:               Option<Entity<CommandPalette>>,
    _palette_sub:          Option<Subscription>,
    price_history:         Option<Entity<PriceHistoryPanel>>,
    _price_history_sub:    Option<Subscription>,
    global_search:         Option<Entity<GlobalSearch>>,
    _global_search_sub:    Option<Subscription>,
    about_dialog:          Option<Entity<AboutDialog>>,
    _about_sub:            Option<Subscription>,
    changelog_panel:       Option<Entity<ChangelogPanel>>,
    _changelog_sub:        Option<Subscription>,
    license_dialog:        Option<Entity<LicenseDialog>>,
    _license_sub:          Option<Subscription>,
    build_expired:         bool,
    updater:               Entity<AutoUpdater>,
    _updater_sub:          Subscription,
    _inventory_panel_sub:  Subscription,
    _pricebook_panel_sub:  Subscription,
    _settings_panel_sub:   Subscription,
    focus_handle:          FocusHandle,
    /// Which top-level menu is open in the Windows custom menu bar (None = all closed).
    #[cfg(not(target_os = "macos"))]
    open_menu_index:       Option<usize>,
}

impl Focusable for VasslRoot {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl VasslRoot {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Check whether a current_user has been set in the DB.
        let needs_first_run = {
            let db = vassl_db::AppDatabase::global(&**cx);
            match vassl_db::shared::current_user(db) {
                Ok(Some(_)) => false,
                _           => true,
            }
        };

        let (first_run, _first_run_sub) = if needs_first_run {
            let form = cx.new(|cx| FirstRunPrompt::new(cx));
            let sub  = cx.subscribe(&form, |this, _form, ev: &FirstRunEvent, cx| {
                match ev {
                    FirstRunEvent::Saved => {
                        this._first_run_sub = None;
                        this.first_run      = None;
                        // Reload username into the already-constructed SettingsPanel,
                        // which was built before the first-run form saved its value.
                        let db   = vassl_db::AppDatabase::global(&**cx);
                        let name = vassl_db::shared::current_user(db)
                            .ok().flatten()
                            .unwrap_or_default();
                        let input = this.settings_panel.read(cx).user_name.clone();
                        input.update(cx, |inp, cx| inp.set_text(name, cx));
                        cx.notify();
                    }
                }
            });
            (Some(form), Some(sub))
        } else {
            (None, None)
        };

        // Apply persisted font size, font family, and theme before first render
        {
            let db = vassl_db::AppDatabase::global(&**cx);
            if let Ok(Some(size_str)) = vassl_db::shared::get_setting(db, "appearance.font_size") {
                if let Ok(size) = size_str.parse::<f32>() {
                    window.set_rem_size(px(size.max(10.0).min(24.0)));
                }
            }
            let font_family = vassl_db::shared::get_setting(db, "appearance.font_family")
                .ok().flatten()
                .unwrap_or_else(|| "system-ui".into());
            let theme = vassl_db::shared::get_setting(db, "appearance.theme")
                .ok().flatten()
                .unwrap_or_else(|| "dark".into());
            let colors = if theme == "light" {
                ThemeColors::light()
            } else {
                ThemeColors::dark()
            };
            cx.set_global(ThemeHandle(colors.with_font(font_family)));
        }

        let focus_handle = cx.focus_handle();
        // Give the root focus on startup so Cmd+F and other app-level shortcuts
        // fire immediately without requiring the user to click a text field first.
        window.focus(&focus_handle, cx);
        // Publish the root focus handle as a global so any form can restore focus
        // after dismissal without threading the handle through every constructor.
        cx.set_global(RootFocusHandle(focus_handle.clone()));

        let settings_panel   = cx.new(SettingsPanel::new);
        settings_panel.update(cx, |panel, cx| panel.wire_observers(cx));
        {
            let sp = settings_panel.read(cx);
            cx.set_global(vassl_ui::AppSettings {
                allow_delete:     sp.allow_delete,
                allow_price_edit: sp.allow_price_edit,
            });
        }
        cx.observe(&settings_panel, |_, settings_entity, cx| {
            let sp = settings_entity.read(cx);
            cx.set_global(vassl_ui::AppSettings {
                allow_delete:     sp.allow_delete,
                allow_price_edit: sp.allow_price_edit,
            });
        }).detach();
        let _settings_panel_sub = cx.subscribe(
            &settings_panel,
            |_this, panel, ev: &SettingsPanelEvent, cx| {
                match ev {
                    SettingsPanelEvent::KeymapChanged => {
                        let overrides = panel.read(cx).keymap_overrides.clone();
                        crate::apply_keybindings(&mut **cx, &overrides);
                    }
                }
            },
        );

        // ── License + build-expiry checks ─────────────────────────────────────
        let build_expired = crate::license::build_expired();
        let (license_dialog, _license_sub) = if !build_expired {
            let needs_license = {
                let db = vassl_db::AppDatabase::global(&**cx);
                let stored_key = vassl_db::shared::get_setting(db, "license.key")
                    .ok().flatten();
                match stored_key {
                    None => true,
                    Some(key) => crate::license::validate_key(&key).is_err(),
                }
            };
            if needs_license {
                let dialog = cx.new(LicenseDialog::new);
                let fh = dialog.read(cx).focus_handle(cx);
                window.focus(&fh, cx);
                let sub = cx.subscribe(&dialog, |this, _, _ev: &LicenseDialogEvent, cx| {
                    this._license_sub  = None;
                    this.license_dialog = None;
                    cx.notify();
                });
                (Some(dialog), Some(sub))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let inventory_panel  = cx.new(InventoryPanel::new);
        let pricebook_panel  = cx.new(PriceBookPanel::new);

        let _inventory_panel_sub = cx.subscribe(
            &inventory_panel,
            |this, _panel, ev: &InventoryPanelEvent, cx| {
                match ev {
                    InventoryPanelEvent::ShowPriceHistory { product_id, name } => {
                        let ph  = cx.new(|cx| PriceHistoryPanel::new(*product_id, name.clone(), cx));
                        let sub = cx.subscribe(&ph, |this, _, ev: &PriceHistoryEvent, cx| {
                            match ev {
                                PriceHistoryEvent::Dismissed => {
                                    this._price_history_sub = None;
                                    this.price_history      = None;
                                    cx.notify();
                                }
                            }
                        });
                        this.price_history      = Some(ph);
                        this._price_history_sub = Some(sub);
                        cx.notify();
                    }
                    InventoryPanelEvent::ShowPriceEntryForm { product_id, name } => {
                        let pid   = *product_id;
                        let pname = name.clone();
                        this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); });
                        this.pricebook_panel.update(cx, |panel, cx| {
                            panel.store.update(cx, |s, cx| s.select_product(pid, cx));
                            panel.open_form_for(pid, pname, cx);
                        });
                    }
                    InventoryPanelEvent::ImportXlsxRequested => {
                        let rx        = cx.prompt_for_paths(PathPromptOptions {
                            files: true, directories: false, multiple: false,
                            prompt: Some("Select XLSX file to import".into()),
                        });
                        // Capture DB handles before entering the async spawn
                        let inv_db    = vassl_inventory::db::InventoryDb::global(&**cx);
                        let sup_db    = vassl_suppliers::db::SupplierDb::global(&**cx);
                        let pb_db     = vassl_pricebook::db::PriceBookDb::global(&**cx);
                        let inv_store = cx.global::<InventoryStoreHandle>().0.clone();
                        cx.spawn(async move |_this, cx| {
                            let Ok(Ok(Some(paths))) = rx.await else { return; };
                            let Some(path) = paths.into_iter().next() else { return; };
                            match crate::importer::run_import(path, inv_db, sup_db, pb_db).await {
                                Ok(summary) => {
                                    tracing::info!(
                                        "Import complete: {} created, {} updated, {} suppliers, {} price entries, {} errors",
                                        summary.products_created, summary.products_updated,
                                        summary.suppliers_created, summary.price_entries,
                                        summary.errors.len()
                                    );
                                    for e in &summary.errors { tracing::warn!("import error: {e}"); }
                                    let _ = inv_store.update(cx, |s, cx| s.load_products(cx));
                                }
                                Err(e) => tracing::error!("XLSX import failed: {e:?}"),
                            }
                        }).detach();
                    }
                }
            },
        );

        let _pricebook_panel_sub = cx.subscribe(
            &pricebook_panel,
            |this, _panel, ev: &PriceBookPanelEvent, cx| {
                match ev {
                    PriceBookPanelEvent::ShowPriceHistory { product_id, name } => {
                        let ph  = cx.new(|cx| PriceHistoryPanel::new(*product_id, name.clone(), cx));
                        let sub = cx.subscribe(&ph, |this, _, ev: &PriceHistoryEvent, cx| {
                            match ev {
                                PriceHistoryEvent::Dismissed => {
                                    this._price_history_sub = None;
                                    this.price_history      = None;
                                    cx.notify();
                                }
                            }
                        });
                        this.price_history      = Some(ph);
                        this._price_history_sub = Some(sub);
                        cx.notify();
                    }
                }
            },
        );

        let supplier_store  = cx.global::<SupplierStoreHandle>().0.clone();
        let suppliers_panel = cx.new(|cx| SupplierPanel::new(supplier_store, cx));

        // Auto-updater — kick off a background check on startup.
        let updater = cx.new(|_| AutoUpdater::new());
        updater.update(cx, |u, cx| u.check(cx));
        let _updater_sub = cx.subscribe(&updater, |_this, _updater, _ev: &AutoUpdateEvent, cx| {
            cx.notify();
        });

        let status_bar = cx.new(StatusBar::new);
        status_bar.update(cx, |bar, _| bar.set_updater(updater.clone()));

        Self {
            sidebar:          cx.new(Sidebar::new),
            status_bar,
            inventory_panel,
            pricebook_panel,
            quotation_panel:  cx.new(QuotationPanel::new),
            suppliers_panel,
            settings_panel,
            first_run,
            _first_run_sub,
            audit_log:            None,
            palette:              None,
            _palette_sub:         None,
            price_history:        None,
            _price_history_sub:   None,
            global_search:        None,
            _global_search_sub:   None,
            about_dialog:         None,
            _about_sub:           None,
            changelog_panel:      None,
            _changelog_sub:       None,
            license_dialog,
            _license_sub,
            build_expired,
            updater,
            _updater_sub,
            _inventory_panel_sub,
            _pricebook_panel_sub,
            _settings_panel_sub,
            focus_handle,
            #[cfg(not(target_os = "macos"))]
            open_menu_index: None,
        }
    }

    fn open_global_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.global_search.is_some() { return; }
        let gs  = cx.new(|cx| GlobalSearch::new(cx));
        let qf  = gs.read(cx).query.read(cx).focus_handle.clone();
        window.focus(&qf, cx);
        let sub = cx.subscribe(&gs, |this, _, ev: &GlobalSearchEvent, cx| {
            match ev {
                GlobalSearchEvent::Dismissed => {
                    this._global_search_sub = None;
                    this.global_search      = None;
                    cx.notify();
                }
                GlobalSearchEvent::Navigate(hit) => {
                    this.sidebar.update(cx, |s, cx| { s.active = hit.module; cx.notify(); });
                    match &hit.kind {
                        SearchResultKind::Product  { id, .. } => {
                            let pid = *id;
                            cx.global::<InventoryStoreHandle>().0.clone()
                                .update(cx, |s, cx| s.select_product(pid, cx));
                        }
                        SearchResultKind::Supplier { id, .. } => {
                            let sid = *id;
                            cx.global::<SupplierStoreHandle>().0.clone()
                                .update(cx, |s, cx| s.select_supplier(sid, cx));
                        }
                        SearchResultKind::Project { .. } => {}
                    }
                    this._global_search_sub = None;
                    this.global_search      = None;
                    cx.notify();
                }
            }
        });
        self.global_search      = Some(gs);
        self._global_search_sub = Some(sub);
        cx.notify();
    }

    fn open_about(&mut self, cx: &mut Context<Self>) {
        if self.about_dialog.is_some() { return; }
        let updater = self.updater.clone();
        let dialog  = cx.new(|cx| AboutDialog::new(updater, cx));
        let sub = cx.subscribe(&dialog, |this, _, ev: &AboutEvent, cx| {
            if matches!(ev, AboutEvent::Copied) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    AboutDialog::full_version_static()
                ));
            }
            this._about_sub   = None;
            this.about_dialog = None;
            cx.notify();
        });
        self.about_dialog = Some(dialog);
        self._about_sub   = Some(sub);
        cx.notify();
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() { return; }
        let pal = cx.new(|cx| CommandPalette::new(cx));
        // Auto-focus the query text input so the user can type immediately.
        let query_focus = pal.read(cx).query.read(cx).focus_handle.clone();
        window.focus(&query_focus, cx);
        let sub = cx.subscribe(&pal, |this, _pal, ev: &PaletteEvent, cx| {
            match ev {
                PaletteEvent::Dismissed => {
                    this._palette_sub = None;
                    this.palette = None;
                    cx.notify();
                }
                PaletteEvent::Execute(cmd) => {
                    match cmd {
                        PaletteCommand::OpenInventory =>
                            this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Inventory; cx.notify(); }),
                        PaletteCommand::OpenQuotations =>
                            this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Quotations; cx.notify(); }),
                        PaletteCommand::OpenPriceBook =>
                            this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); }),
                        PaletteCommand::OpenAuditLog => {
                            if this.audit_log.is_none() {
                                this.audit_log = Some(cx.new(|cx| AuditLogPanel::new(cx)));
                            }
                        }
                    }
                    this._palette_sub = None;
                    this.palette = None;
                    cx.notify();
                }
            }
        });
        self.palette      = Some(pal);
        self._palette_sub = Some(sub);
        cx.notify();
    }

    /// Render the custom title bar used on Windows (macOS uses the native title bar).
    ///
    /// Layout: [menu buttons …] [drag area, flex-1] [─ □ ✕ caption buttons]
    ///
    /// Each region uses `window_control_area()` so GPUI translates hit-test results
    /// to the correct Win32 `NCHITTEST` values — the OS handles actual window management.
    #[cfg(not(target_os = "macos"))]
    fn render_menu_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui::WindowControlArea;

        let c = cx.global::<ThemeHandle>().0.clone();
        let menus = cx.get_menus().unwrap_or_default();

        // ── Menu buttons ──────────────────────────────────────────────────────
        let mut menu_buttons: Vec<gpui::AnyElement> = Vec::new();
        for (menu_ix, menu) in menus.into_iter().enumerate() {
            let menu_name = menu.name.clone();
            let is_open   = self.open_menu_index == Some(menu_ix);
            let btn_bg    = rgb(if is_open { c.surface_active } else { c.surface_default });
            let hover_bg  = rgb(c.surface_hover);

            let dropdown: Option<gpui::AnyElement> = if is_open {
                let mut rows: Vec<gpui::AnyElement> = Vec::new();
                for (item_ix, item) in menu.items.into_iter().enumerate() {
                    match item {
                        OwnedMenuItem::Separator => {
                            rows.push(
                                div()
                                    .id(format!("msep-{menu_ix}-{item_ix}"))
                                    .h(px(1.))
                                    .bg(rgb(c.surface_hover))
                                    .mx(px(4.)).my(px(2.))
                                    .into_any_element(),
                            );
                        }
                        OwnedMenuItem::Action { name, action, disabled, .. } => {
                            let text_col   = rgb(if disabled { c.text_muted } else { c.text_default });
                            let item_hover = rgb(c.surface_hover);
                            let item_el = div()
                                .id(format!("mitem-{menu_ix}-{item_ix}"))
                                .px(px(16.)).py(px(5.))
                                .text_size(rems(0.923))
                                .text_color(text_col)
                                .when(!disabled, |d| {
                                    d.hover(move |s| s.bg(item_hover))
                                     .cursor_pointer()
                                     .on_mouse_down(
                                         MouseButton::Left,
                                         cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                             this.open_menu_index = None;
                                             window.dispatch_action(action.boxed_clone(), cx);
                                             cx.notify();
                                         }),
                                     )
                                })
                                .child(name);
                            rows.push(item_el.into_any_element());
                        }
                        _ => {}
                    }
                }
                Some(
                    deferred(
                        div()
                            .id(format!("mdrop-{menu_ix}"))
                            .absolute()
                            .top(px(32.))
                            .left(px(0.))
                            .min_w(px(200.))
                            .bg(rgb(c.surface_default))
                            .py(px(4.))
                            .children(rows),
                    )
                    .into_any_element(),
                )
            } else {
                None
            };

            let wrapper = div()
                .id(format!("mwrap-{menu_ix}"))
                .relative()
                .child(
                    div()
                        .id(format!("mbtn-{menu_ix}"))
                        .px(px(10.))
                        .h(px(32.))
                        .flex()
                        .items_center()
                        .text_size(rems(0.846))
                        .text_color(rgb(c.text_default))
                        .bg(btn_bg)
                        .hover(move |s| s.bg(hover_bg))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                this.open_menu_index = if this.open_menu_index == Some(menu_ix) {
                                    None
                                } else {
                                    Some(menu_ix)
                                };
                                cx.notify();
                            }),
                        )
                        .child(menu_name),
                )
                .when_some(dropdown, |d, drop| d.child(drop));

            menu_buttons.push(wrapper.into_any_element());
        }

        // ── Caption buttons (Segoe Fluent Icons on Win11, MDL2 on older) ──────
        let icon_font = if cfg!(target_os = "windows") {
            // Runtime check via windows build number would require extra FFI;
            // Segoe Fluent Icons ships on all Win10 21H1+ and Win11, which covers
            // the app's minimum target. Fall back gracefully if unavailable.
            "Segoe Fluent Icons"
        } else {
            // Running in a dev/CI environment — use a reasonable fallback.
            "Segoe MDL2 Assets"
        };

        let is_maximized = window.is_maximized();

        let caption_btn = |id: &'static str, icon: &'static str, area: WindowControlArea,
                           is_close: bool, tip: &'static str| {
            let close_hover  = rgb(0xE81120u32);
            let close_active = rgb(0xBF0E1Au32);
            let normal_hover = rgb(c.surface_hover);
            div()
                .id(id)
                .w(px(46.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.))
                .text_color(rgb(c.text_default))
                .font_family(icon_font)
                .when(is_close, |d| {
                    d.hover(move |s| s.bg(close_hover).text_color(rgb(0xFFFFFF)))
                     .active(move |s| s.bg(close_active).text_color(rgb(0xFFFFFF)))
                })
                .when(!is_close, |d| {
                    d.hover(move |s| s.bg(normal_hover))
                })
                .cursor_pointer()
                .window_control_area(area)
                .tooltip(tooltip(tip))
                .child(icon)
        };

        div()
            .id("app-title-bar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(32.))
            .flex_shrink_0()
            .bg(rgb(c.surface_default))
            // Menu buttons on the left
            .children(menu_buttons)
            // Drag region fills remaining space
            .child(
                div()
                    .id("title-drag")
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
            )
            // Caption buttons on the right
            .child(caption_btn("btn-min",
                "\u{e921}", WindowControlArea::Min, false, "Minimize"))
            .child(caption_btn(
                if is_maximized { "btn-restore" } else { "btn-max" },
                if is_maximized { "\u{e923}" } else { "\u{e922}" },
                WindowControlArea::Max, false,
                if is_maximized { "Restore" } else { "Maximize" }))
            .child(caption_btn("btn-close",
                "\u{e8bb}", WindowControlArea::Close, true, "Close"))
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();

        // Build expiry blocks the entire app — render a static dead-end screen.
        if self.build_expired {
            return div()
                .relative().flex().flex_col().w_full().h_full()
                .font_family(gpui::SharedString::from(c.font_family.clone()))
                .bg(rgb(c.canvas_bg))
                .flex().items_center().justify_center()
                .child(
                    div().flex().flex_col().items_center().gap(px(12.))
                        .child(div().text_size(rems(1.385)).text_color(rgb(c.text_default))
                            .child("This VASSL build has expired"))
                        .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted))
                            .child("Please contact your VASSL administrator for an updated version."))
                        .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                            .child(format!("Version {}", env!("CARGO_PKG_VERSION"))))
                );
        }

        let active = self.sidebar.read(cx).active;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active {
            ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
            ActiveModule::Quotations => content.child(self.quotation_panel.clone()),
            ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
            ActiveModule::Suppliers  => content.child(self.suppliers_panel.clone()),
            ActiveModule::Settings   => content.child(self.settings_panel.clone()),
        };

        let root = div()
            .key_context("VasslRoot")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &OpenInventory, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Inventory; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenQuotations, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Quotations; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenPriceBook, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &OpenSuppliers, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Suppliers; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| {
                let active = this.sidebar.read(cx).active;
                let fh = match active {
                    ActiveModule::Inventory  => this.inventory_panel.update(cx, |p, cx| p.create_product_form(cx)),
                    ActiveModule::Suppliers  => this.suppliers_panel.update(cx, |p, cx| p.create_new_form(cx)),
                    ActiveModule::Quotations => this.quotation_panel.update(cx, |p, cx| p.create_form(cx)),
                    ActiveModule::PriceBook  => this.pricebook_panel.update(cx, |p, cx| p.create_form(cx)),
                    ActiveModule::Settings   => None,
                };
                if let Some(fh) = fh { window.focus(&fh, cx); }
            }))
            .on_action(cx.listener(|this, _: &OpenAuditLog, _w, cx| {
                if this.audit_log.is_some() {
                    this.audit_log = None;
                } else {
                    this.audit_log = Some(cx.new(|cx| AuditLogPanel::new(cx)));
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _w, cx| {
                this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::Settings; cx.notify(); });
            }))
            .on_action(cx.listener(|this, _: &IncreaseFontSize, window, cx| {
                this.settings_panel.update(cx, |sp, cx| {
                    sp.font_size = (sp.font_size + 0.5).min(24.0);
                    sp.save_setting("appearance.font_size", format!("{:.1}", sp.font_size), cx);
                    cx.notify();
                });
                let font_size = this.settings_panel.read(cx).font_size as f32;
                window.set_rem_size(px(font_size));
            }))
            .on_action(cx.listener(|this, _: &DecreaseFontSize, window, cx| {
                this.settings_panel.update(cx, |sp, cx| {
                    sp.font_size = (sp.font_size - 0.5).max(10.0);
                    sp.save_setting("appearance.font_size", format!("{:.1}", sp.font_size), cx);
                    cx.notify();
                });
                let font_size = this.settings_panel.read(cx).font_size as f32;
                window.set_rem_size(px(font_size));
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.open_palette(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenGlobalSearch, window, cx| {
                this.open_global_search(window, cx);
            }))
            .on_action(cx.listener(|_this, _: &Minimize, window, _cx| {
                window.minimize_window();
            }))
            .on_action(cx.listener(|_this, _: &Zoom, window, _cx| {
                window.zoom_window();
            }))
            .on_action(cx.listener(|this, _: &About, _w, cx| {
                this.open_about(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenChangelog, _w, cx| {
                if this.changelog_panel.is_some() {
                    this._changelog_sub  = None;
                    this.changelog_panel = None;
                } else {
                    let panel = cx.new(ChangelogPanel::new);
                    let sub   = cx.subscribe(&panel, |this, _, _ev: &ChangelogEvent, cx| {
                        this._changelog_sub  = None;
                        this.changelog_panel = None;
                        cx.notify();
                    });
                    this.changelog_panel = Some(panel);
                    this._changelog_sub  = Some(sub);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CheckForUpdates, _, cx| {
                this.updater.update(cx, |u, cx| u.check(cx));
            }))
            .on_action(cx.listener(|this, _: &InstallUpdate, _, cx| {
                let status = this.updater.read(cx).status.clone();
                if let UpdateStatus::ReadyToInstall(zip) = status {
                    this.updater.update(cx, |u, cx| u.install_and_restart(zip, cx));
                } else if let UpdateStatus::Available(info) = status {
                    this.updater.update(cx, |u, cx| u.download(info, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                let active = this.sidebar.read(cx).active;
                match active {
                    ActiveModule::Inventory  => { this.inventory_panel.update(cx, |p, cx| p.select_next(cx)); }
                    ActiveModule::PriceBook  => { this.pricebook_panel.update(cx, |p, cx| p.select_next(cx)); }
                    ActiveModule::Suppliers  => { this.suppliers_panel.update(cx, |p, cx| p.select_next(cx)); }
                    ActiveModule::Quotations => { this.quotation_panel.update(cx, |p, cx| p.select_next(cx)); }
                    ActiveModule::Settings   => {}
                }
            }))
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                let active = this.sidebar.read(cx).active;
                match active {
                    ActiveModule::Inventory  => { this.inventory_panel.update(cx, |p, cx| p.select_prev(cx)); }
                    ActiveModule::PriceBook  => { this.pricebook_panel.update(cx, |p, cx| p.select_prev(cx)); }
                    ActiveModule::Suppliers  => { this.suppliers_panel.update(cx, |p, cx| p.select_prev(cx)); }
                    ActiveModule::Quotations => { this.quotation_panel.update(cx, |p, cx| p.select_prev(cx)); }
                    ActiveModule::Settings   => {}
                }
            }))
            .on_action(cx.listener(|this, _: &EscapeModal, w, cx| {
                if this.changelog_panel.is_some() {
                    this._changelog_sub  = None;
                    this.changelog_panel = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.about_dialog.is_some() {
                    this._about_sub   = None;
                    this.about_dialog = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.palette.is_some() {
                    this._palette_sub = None;
                    this.palette = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.global_search.is_some() {
                    this._global_search_sub = None;
                    this.global_search      = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.price_history.is_some() {
                    this._price_history_sub = None;
                    this.price_history      = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.audit_log.is_some() {
                    this.audit_log = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                }
            }))
            // When the keyboard settings panel is in listening mode, capture the
            // next keypress here (root always holds focus) and forward it as the
            // new binding.  Escape cancels without capturing.
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _w, cx| {
                if this.settings_panel.read(cx).listening_for.is_none() { return; }
                // Ignore bare modifier keys — wait for the actual key
                match event.keystroke.key.as_str() {
                    "shift" | "alt" | "control" | "platform" | "function" | "cmd" | "win" | "super" => return,
                    _ => {}
                }
                if event.keystroke.key == "escape" {
                    this.settings_panel.update(cx, |sp, cx| {
                        sp.listening_for = None;
                        cx.emit(crate::settings_panel::SettingsPanelEvent::KeymapChanged);
                        cx.notify();
                    });
                    return;
                }
                let keystroke = crate::keybindings::normalize_for_keybinding(&event.keystroke);
                this.settings_panel.update(cx, |sp, cx| sp.capture_key_for_listening(keystroke, cx));
            }))
            .relative()
            .flex().flex_col().w_full().h_full()
            .font_family(gpui::SharedString::from(c.font_family.clone()))
            .bg(rgb(c.canvas_bg));

        // On Windows with appears_transparent=true the DWM resize border occupies
        // the top ~8 px of the client area, pushing rendered content out of frame.
        #[cfg(target_os = "windows")]
        let root = root.pt(px(8.));

        // Windows/Linux: render menus as a custom bar; macOS uses the native system menu bar.
        #[cfg(not(target_os = "macos"))]
        let mut root = root.child(self.render_menu_bar(_window, cx));

        let mut root = root
            .child(
                div().flex().flex_row().flex_1().min_h(px(0.)).overflow_hidden()
                    .child(self.sidebar.clone())
                    .child(content),
            )
            .child(self.status_bar.clone());

        // Click-away capture: covers the full window so clicking outside the open menu closes it.
        #[cfg(not(target_os = "macos"))]
        if self.open_menu_index.is_some() {
            root = root.child(
                deferred(
                    div()
                        .id("menu-clickaway")
                        .absolute().inset_0()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.open_menu_index = None;
                            cx.notify();
                        }))
                )
            );
        }

        if let Some(form) = &self.first_run {
            root = root.child(form.clone());
        }
        if let Some(panel) = &self.audit_log {
            root = root.child(panel.clone());
        }
        if let Some(pal) = &self.palette {
            root = root.child(pal.clone());
        }
        if let Some(ph) = &self.price_history {
            root = root.child(ph.clone());
        }
        if let Some(gs) = &self.global_search {
            root = root.child(gs.clone());
        }
        if let Some(about) = &self.about_dialog {
            root = root.child(about.clone());
        }
        if let Some(changelog) = &self.changelog_panel {
            root = root.child(changelog.clone());
        }
        if let Some(dialog) = &self.license_dialog {
            root = root.child(dialog.clone());
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use vassl_inventory::panel::InventoryPanelEvent;
    use vassl_pricebook::panel::PriceBookPanelEvent;

    #[test]
    fn inventory_panel_event_variants_are_accessible() {
        let ev = InventoryPanelEvent::ShowPriceHistory {
            product_id: 1,
            name:       "Test".to_string(),
        };
        assert!(matches!(ev, InventoryPanelEvent::ShowPriceHistory { .. }));
    }

    #[test]
    fn pricebook_panel_event_variants_are_accessible() {
        let ev = PriceBookPanelEvent::ShowPriceHistory {
            product_id: 2,
            name:       "Test".to_string(),
        };
        assert!(matches!(ev, PriceBookPanelEvent::ShowPriceHistory { .. }));
    }

    #[test]
    fn open_global_search_action_is_distinct_from_focus_search() {
        use crate::actions::{FocusSearch, OpenGlobalSearch};
        let _a: FocusSearch      = FocusSearch;
        let _b: OpenGlobalSearch = OpenGlobalSearch;
    }
}
