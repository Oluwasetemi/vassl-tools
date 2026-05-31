use gpui::{Context, Entity, IntoElement, Render, Subscription, Window, div, prelude::*, rgb};

use crate::actions::{FocusSearch, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations};
use crate::audit_log::AuditLogPanel;
use crate::colors;
use crate::command_palette::{CommandPalette, PaletteEvent, PaletteCommand};
use crate::first_run::{FirstRunEvent, FirstRunPrompt};
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;
use vassl_inventory::panel::InventoryPanel;
use vassl_pricebook::panel::PriceBookPanel;
use vassl_quotations::panel::QuotationPanel;

pub struct VasslRoot {
    sidebar:          Entity<Sidebar>,
    status_bar:       Entity<StatusBar>,
    inventory_panel:  Entity<InventoryPanel>,
    pricebook_panel:  Entity<PriceBookPanel>,
    quotation_panel:  Entity<QuotationPanel>,
    first_run:        Option<Entity<FirstRunPrompt>>,
    _first_run_sub:   Option<Subscription>,
    audit_log:        Option<Entity<AuditLogPanel>>,
    palette:          Option<Entity<CommandPalette>>,
    _palette_sub:     Option<Subscription>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
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

        Self {
            sidebar:          cx.new(Sidebar::new),
            status_bar:       cx.new(StatusBar::new),
            inventory_panel:  cx.new(InventoryPanel::new),
            pricebook_panel:  cx.new(PriceBookPanel::new),
            quotation_panel:  cx.new(QuotationPanel::new),
            first_run,
            _first_run_sub,
            audit_log: None,
            palette:   None,
            _palette_sub: None,
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
        let active = self.sidebar.read(cx).active;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active {
            ActiveModule::Inventory  => content.child(self.inventory_panel.clone()),
            ActiveModule::Quotations => content.child(self.quotation_panel.clone()),
            ActiveModule::PriceBook  => content.child(self.pricebook_panel.clone()),
        };

        let mut root = div()
            .key_context("VasslRoot")
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
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.open_palette(window, cx);
            }))
            .relative()
            .flex().flex_col().w_full().h_full()
            .bg(rgb(colors::CANVAS_BG))
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

        root
    }
}
