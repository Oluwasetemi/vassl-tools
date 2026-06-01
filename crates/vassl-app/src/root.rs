use gpui::{Context, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription, Window, div, prelude::*, px, rgb};
use vassl_ui::{ThemeColors, ThemeHandle};

use crate::actions::{EscapeModal, FocusSearch, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations, OpenSettings};
use crate::settings_panel::SettingsPanel;
use crate::audit_log::AuditLogPanel;
use crate::command_palette::{CommandPalette, PaletteEvent, PaletteCommand};
use crate::first_run::{FirstRunEvent, FirstRunPrompt};
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;
use vassl_inventory::panel::{InventoryPanel, InventoryPanelEvent};
use vassl_pricebook::panel::{PriceBookPanel, PriceBookPanelEvent};
use vassl_pricebook::price_history::{PriceHistoryEvent, PriceHistoryPanel};
use vassl_quotations::panel::QuotationPanel;

pub struct VasslRoot {
    sidebar:          Entity<Sidebar>,
    status_bar:       Entity<StatusBar>,
    inventory_panel:  Entity<InventoryPanel>,
    pricebook_panel:  Entity<PriceBookPanel>,
    quotation_panel:  Entity<QuotationPanel>,
    settings_panel:   Entity<SettingsPanel>,
    first_run:             Option<Entity<FirstRunPrompt>>,
    _first_run_sub:        Option<Subscription>,
    audit_log:             Option<Entity<AuditLogPanel>>,
    palette:               Option<Entity<CommandPalette>>,
    _palette_sub:          Option<Subscription>,
    price_history:         Option<Entity<PriceHistoryPanel>>,
    _price_history_sub:    Option<Subscription>,
    _inventory_panel_sub:  Subscription,
    _pricebook_panel_sub:  Subscription,
    focus_handle:          FocusHandle,
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
                        cx.notify();
                    }
                }
            });
            (Some(form), Some(sub))
        } else {
            (None, None)
        };

        // Apply persisted font size and theme before first render
        {
            let db = vassl_db::AppDatabase::global(&**cx);
            if let Ok(Some(size_str)) = vassl_db::shared::get_setting(db, "appearance.font_size") {
                if let Ok(size) = size_str.parse::<f32>() {
                    window.set_rem_size(px(size.max(10.0).min(24.0)));
                }
            }
            if let Ok(Some(theme)) = vassl_db::shared::get_setting(db, "appearance.theme") {
                let colors = if theme == "light" {
                    ThemeColors::light()
                } else {
                    ThemeColors::dark()
                };
                cx.set_global(ThemeHandle(colors));
            }
        }

        let focus_handle = cx.focus_handle();
        // Give the root focus on startup so Cmd+F and other app-level shortcuts
        // fire immediately without requiring the user to click a text field first.
        window.focus(&focus_handle, cx);

        let settings_panel   = cx.new(SettingsPanel::new);
        settings_panel.update(cx, |panel, cx| panel.wire_observers(cx));

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
                    InventoryPanelEvent::ShowPriceEntryForm { product_id, .. } => {
                        let pid = *product_id;
                        this.sidebar.update(cx, |s, cx| { s.active = ActiveModule::PriceBook; cx.notify(); });
                        this.pricebook_panel.update(cx, |panel, cx| {
                            panel.store.update(cx, |s, cx| s.select_product(pid, cx));
                        });
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

        Self {
            sidebar:          cx.new(Sidebar::new),
            status_bar:       cx.new(StatusBar::new),
            inventory_panel,
            pricebook_panel,
            quotation_panel:  cx.new(QuotationPanel::new),
            settings_panel,
            first_run,
            _first_run_sub,
            audit_log:            None,
            palette:              None,
            _palette_sub:         None,
            price_history:        None,
            _price_history_sub:   None,
            _inventory_panel_sub,
            _pricebook_panel_sub,
            focus_handle,
        }
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
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active = self.sidebar.read(cx).active;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active {
            ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
            ActiveModule::Quotations => content.child(self.quotation_panel.clone()),
            ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
            ActiveModule::Settings   => content.child(self.settings_panel.clone()),
        };

        let mut root = div()
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
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.open_palette(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EscapeModal, w, cx| {
                if this.palette.is_some() {
                    this._palette_sub = None;
                    this.palette = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.audit_log.is_some() {
                    this.audit_log = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                } else if this.price_history.is_some() {
                    this._price_history_sub = None;
                    this.price_history      = None;
                    w.focus(&this.focus_handle, cx);
                    cx.notify();
                }
            }))
            .relative()
            .flex().flex_col().w_full().h_full()
            .bg(rgb(c.canvas_bg))
            .child(
                div().flex().flex_row().flex_1()
                    .child(self.sidebar.clone())
                    .child(content),
            )
            .child(self.status_bar.clone());

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
}
