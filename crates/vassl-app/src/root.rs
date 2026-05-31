use gpui::{Context, Entity, IntoElement, Render, Subscription, Window, div, prelude::*, rgb};

use crate::actions::{FocusSearch, OpenAuditLog, OpenInventory, OpenPriceBook, OpenQuotations};
use crate::audit_log::AuditLogPanel;
use crate::colors;
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
        }
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
                    // Toggle off
                    this.audit_log = None;
                } else {
                    this.audit_log = Some(cx.new(|cx| AuditLogPanel::new(cx)));
                }
                cx.notify();
            }))
            .on_action(cx.listener(|_this, _: &FocusSearch, _w, _cx| {
                // TODO(Plan 5 Task 8): open command palette
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

        root
    }
}
