use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};
use vassl_ui::ThemeHandle;

use crate::colors;
use crate::price_form::{PriceEntryForm, PriceFormEvent};
use crate::price_table::PriceTable;
use crate::store::PriceBookStore;
use crate::PriceBookStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab { PriceBook, History }

pub struct PriceBookPanel {
    store:       Entity<PriceBookStore>,
    price_table: Entity<PriceTable>,
    active_tab:  Tab,
    form:        Option<Entity<PriceEntryForm>>,
    _form_sub:   Option<Subscription>,
}

impl PriceBookPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<PriceBookStoreHandle>().0.clone();
        let price_table = cx.new(|cx| PriceTable::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_products(cx));
        Self {
            store,
            price_table,
            active_tab: Tab::PriceBook,
            form:      None,
            _form_sub: None,
        }
    }

    fn open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let (product_id, product_name) = {
            let store = self.store.read(cx);
            let Some(pid) = store.selected_product_id else { return; };
            let name = store.product_prices
                .iter()
                .find(|p| p.product_id == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (pid, name)
        };
        let form = cx.new(|cx| PriceEntryForm::new(self.store.clone(), product_id, product_name, cx));
        let first = form.read(cx).cost.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        let sub  = cx.subscribe(&form, |this, _form, ev: &PriceFormEvent, cx| {
            match ev {
                PriceFormEvent::Submitted | PriceFormEvent::Cancelled => {
                    this._form_sub = None;
                    this.form      = None;
                    cx.notify();
                }
            }
        });
        self.form      = Some(form);
        self._form_sub = Some(sub);
        cx.notify();
    }
}

impl Render for PriceBookPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

        // Extract history data while store is borrowed
        let history_rows: Vec<_> = {
            let store = self.store.read(cx);
            store.history.iter().map(|e| {
                (
                    e.effective_date[..10].to_string(),
                    e.cost_price_usd,
                    e.duty_cost_usd,
                    e.markup_percent,
                    e.selling_price_usd,
                )
            }).collect()
        };
        let history_is_empty = history_rows.is_empty();

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::PriceBook => content.child(self.price_table.clone()),
            Tab::History => {
                if !has_selection {
                    content.child(
                        div()
                            .flex_1().flex().items_center().justify_center()
                            .text_color(rgb(c.text_muted))
                            .child("Select a product row to view pricing history.")
                    )
                } else if history_is_empty {
                    content.child(
                        div()
                            .flex_1().flex().items_center().justify_center()
                            .text_color(rgb(c.text_muted))
                            .child("No price history for this product.")
                    )
                } else {
                    let rows: Vec<_> = history_rows.iter().map(|(date, cost, duty, markup, sell)| {
                        div()
                            .flex().flex_row().items_center().w_full()
                            .px(px(12.)).py(px(6.))
                            .child(div().w(px(100.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(date.clone()))
                            .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(c.text_default)).child(format!("${cost:.2}")))
                            .child(div().w(px(80.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(format!("+${duty:.2}")))
                            .child(div().w(px(70.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child(format!("{markup:.0}%")))
                            .child(div().flex_1().text_size(px(13.)).text_color(rgb(c.status_green)).child(format!("${sell:.2}")))
                    }).collect();

                    content.child(
                        div()
                            .id("history-scroll")
                            .flex_1().flex().flex_col()
                            .overflow_y_scroll()
                            .children(rows)
                    )
                }
            }
        };

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                // Tab bar + button row
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    // Price Book tab
                    .child(
                        div()
                            .id("pb-tab-pricebook")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::PriceBook { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::PriceBook;
                                cx.notify();
                            }))
                            .child("Price Book")
                    )
                    // History tab
                    .child(
                        div()
                            .id("pb-tab-history")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::History { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::History;
                                cx.notify();
                            }))
                            .child("History")
                    )
                    // Spacer
                    .child(div().flex_1())
                    // New Entry button — enabled only when a product is selected
                    .child({
                        let mut btn = div()
                            .id("pb-btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .child("+ New Entry");
                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_form(window, cx);
                                }));
                        }
                        btn
                    })
            )
            .child(content);

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        root
    }
}
