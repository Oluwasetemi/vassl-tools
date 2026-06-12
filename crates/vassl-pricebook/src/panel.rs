use gpui::{Context, Entity, EventEmitter, Focusable, IntoElement, MouseButton, MouseDownEvent,
           Render, Subscription, Window, div, prelude::*, px, rems, rgb};
use vassl_inventory::InventoryStoreHandle;
use vassl_ui::{AppSettings, TextInput, ThemeHandle, text_field, tooltip};

use crate::price_form::{PriceEntryForm, PriceFormEvent};
use crate::price_table::PriceTable;
use crate::store::PriceBookStore;
use crate::PriceBookStoreHandle;

#[derive(Clone, PartialEq)]
pub enum PriceBookPanelEvent {
    ShowPriceHistory { product_id: i64, name: String },
}

impl EventEmitter<PriceBookPanelEvent> for PriceBookPanel {}

#[derive(Clone, Copy, PartialEq)]
enum Tab { PriceBook, History }

pub struct PriceBookPanel {
    pub store:    Entity<PriceBookStore>,
    price_table:  Entity<PriceTable>,
    active_tab:   Tab,
    form:         Option<Entity<PriceEntryForm>>,
    _form_sub:    Option<Subscription>,
    search_input: Entity<TextInput>,
}

impl PriceBookPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<PriceBookStoreHandle>().0.clone();
        let price_table = cx.new(|cx| PriceTable::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_products(cx));
        let search_input = cx.new(|cx| TextInput::with_placeholder("Filter…", cx));

        cx.observe(&search_input, |this, input, cx| {
            let q = input.read(cx).text().to_string();
            this.store.update(cx, |s, cx| s.set_search_query(q, cx));
        }).detach();

        Self {
            store,
            price_table,
            active_tab:   Tab::PriceBook,
            form:         None,
            _form_sub:    None,
            search_input,
        }
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_next(cx)) {
            self.price_table.update(cx, |t, _| t.scroll_handle.scroll_to_item(idx, gpui::ScrollStrategy::Top));
        }
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_prev(cx)) {
            self.price_table.update(cx, |t, _| t.scroll_handle.scroll_to_item(idx, gpui::ScrollStrategy::Top));
        }
    }

    /// Returns the focus handle of the currently open form's first input, if any.
    pub fn form_focus_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        self.form.as_ref().map(|f| f.read(cx).focus_handle(cx))
    }

    pub fn create_form(&mut self, cx: &mut Context<Self>) -> Option<gpui::FocusHandle> {
        let (pid, name) = {
            let store = self.store.read(cx);
            let pid   = store.selected_product_id?;
            let name  = store.product_prices.iter()
                .find(|p| p.product_id == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (pid, name)
        };
        self.open_form_for(pid, name, cx);
        self.form.as_ref().map(|f| f.read(cx).focus_handle(cx))
    }

    fn open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fh) = self.create_form(cx) {
            window.defer(cx, move |window, cx| { window.focus(&fh, cx); });
        }
    }

    pub fn open_edit_form_for(&mut self, product_id: i64, name: String, entry: vassl_core::PriceEntry, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let inv_store = cx.global::<InventoryStoreHandle>().0.read(cx);
        let current_stock = inv_store.products.iter()
            .find(|p| p.product.id == product_id)
            .map(|p| p.current_stock)
            .unwrap_or(0.0);
        let duty_percent = inv_store.products.iter()
            .find(|p| p.product.id == product_id)
            .map(|p| p.product.duty_percent)
            .unwrap_or_else(|| {
                self.store.read(cx).product_prices.iter()
                    .find(|pp| pp.product_id == product_id)
                    .map(|pp| pp.duty_percent)
                    .unwrap_or(0.0)
            });
        let _ = inv_store;
        let form = cx.new(|cx| crate::price_form::PriceEntryForm::edit_for(
            self.store.clone(), product_id, name, current_stock, duty_percent, entry, cx,
        ));
        let sub = cx.subscribe(&form, |this, _form, ev: &crate::price_form::PriceFormEvent, cx| {
            match ev {
                crate::price_form::PriceFormEvent::Submitted | crate::price_form::PriceFormEvent::Cancelled => {
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

    pub fn open_form_for(&mut self, product_id: i64, name: String, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let inv_store = cx.global::<InventoryStoreHandle>().0.read(cx);
        let current_stock = inv_store.products.iter()
            .find(|p| p.product.id == product_id)
            .map(|p| p.current_stock)
            .unwrap_or(0.0);
        let duty_percent = {
            // Use product's duty from store; fall back to pricebook's own product_prices
            let from_inv = inv_store.products.iter()
                .find(|p| p.product.id == product_id)
                .map(|p| p.product.duty_percent);
            from_inv.unwrap_or_else(|| {
                self.store.read(cx).product_prices.iter()
                    .find(|pp| pp.product_id == product_id)
                    .map(|pp| pp.duty_percent)
                    .unwrap_or(0.0)
            })
        };
        let _ = inv_store; // release borrow before cx.new
        let form  = cx.new(|cx| PriceEntryForm::new(self.store.clone(), product_id, name, duty_percent, current_stock, cx));
        let sub   = cx.subscribe(&form, |this, _form, ev: &PriceFormEvent, cx| {
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab    = self.active_tab;
        let has_selection = self.store.read(cx).selected_product_id.is_some();

        let has_query = !self.search_input.read(cx).text().is_empty();

        let history_rows: Vec<_> = {
            let store = self.store.read(cx);
            store.history.iter().map(|e| {
                (
                    e.effective_date.get(..10).unwrap_or(&e.effective_date).to_string(),
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
                            .child(div().w(px(100.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(date.clone()))
                            .child(div().w(px(90.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(format!("${cost:.2}")))
                            .child(div().w(px(80.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(format!("+${duty:.2}")))
                            .child(div().w(px(70.)).text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(format!("{markup:.0}%")))
                            .child(div().flex_1().text_size(rems(1.)).text_color(rgb(c.status_green)).child(format!("${sell:.2}")))
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
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child({
                        let is_tab = active_tab == Tab::PriceBook;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("pb-tab-pricebook")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if is_tab { c.surface_active } else { c.surface_default }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::PriceBook;
                                cx.notify();
                            }))
                            .child("Price Book")
                    })
                    .child({
                        let is_tab = active_tab == Tab::History;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("pb-tab-history")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if is_tab { c.surface_active } else { c.surface_default }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::History;
                                cx.notify();
                            }))
                            .child("History")
                    })
                    .child(
                        div()
                            .flex().flex_row().items_center().gap(px(4.))
                            .child(
                                div()
                                    .w(px(160.))
                                    .child({
                                        let focused = self.search_input.read(cx).focus_handle.is_focused(window);
                                        text_field("", self.search_input.clone(), focused, false, cx)
                                    })
                            )
                            .child({
                                let mut clear = div()
                                    .id("pb-search-clear")
                                    .px(px(6.)).py(px(2.)).rounded(px(3.))
                                    .text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                    .child("×");
                                if has_query {
                                    let si = self.search_input.clone();
                                    clear = clear
                                        .cursor_pointer()
                                        .on_mouse_down(gpui::MouseButton::Left, move |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut gpui::App| {
                                            si.update(cx, |t, cx| t.reset(cx));
                                        });
                                }
                                clear
                            })
                    )
                    .child(div().flex_1())
                    .child({
                        let mut btn = div()
                            .id("pb-btn-new-entry")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .tooltip(tooltip("New Price Entry"))
                            .child("+ New Entry");
                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
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

        // Context menu overlay
        let allow_delete     = cx.global::<AppSettings>().allow_delete;
        let allow_price_edit = cx.global::<AppSettings>().allow_price_edit;
        let ctx_menu = self.store.read(cx).context_menu.clone();
        if let Some(target) = ctx_menu {
            let viewport = window.viewport_size();
            const MENU_W: f32 = 220.0;
            const MENU_H: f32 = 260.0;
            let menu_x = target.x.min((viewport.width.as_f32()  - MENU_W).max(0.0));
            let menu_y = target.y.min((viewport.height.as_f32() - MENU_H).max(0.0));
            let info_line = {
                let store = self.store.read(cx);
                store.product_prices
                    .iter()
                    .find(|pp| pp.product_id == target.product_id)
                    .map(|pp| {
                        match &pp.latest {
                            None    => "No price set".to_string(),
                            Some(e) => format!(
                                "${:.2} + ${:.2} → {:.0}% → ${:.2}",
                                e.cost_price_usd, e.duty_cost_usd, e.markup_percent, e.selling_price_usd
                            ),
                        }
                    })
                    .unwrap_or_default()
            };

            let pid  = target.product_id;
            let name = target.product_name.clone();
            let latest_entry = {
                let store = self.store.read(cx);
                store.product_prices.iter()
                    .find(|pp| pp.product_id == pid)
                    .and_then(|pp| pp.latest.clone())
            };
            let has_latest = latest_entry.is_some();
            let product_name_clone = name.clone();

            root = root
                .child(
                    div()
                        .absolute().inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseDownEvent, _: &mut Window, cx| {
                                this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                            }),
                        )
                )
                .child(
                    div()
                        .absolute()
                        .left(px(menu_x))
                        .top(px(menu_y))
                        .w(px(220.))
                        .bg(rgb(c.surface_default))
                        .rounded(px(6.))
                        .shadow_md()
                        .child(
                            div()
                                .px(px(12.)).pt(px(10.)).pb(px(4.))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(target.product_name.clone())
                        )
                        .child(
                            div()
                                .px(px(12.)).pb(px(8.))
                                .text_size(rems(0.846))
                                .text_color(rgb(c.text_muted))
                                .child(info_line)
                        )
                        .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                        .child({
                            let n = name.clone();
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-pb-price-history")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("Price History")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        cx.emit(PriceBookPanelEvent::ShowPriceHistory {
                                            product_id: pid,
                                            name:       n.clone(),
                                        });
                                    }),
                                )
                        })
                        .child({
                            let hover_bg = rgb(c.surface_hover);
                            let name_for_add = name.clone();
                            div()
                                .id("ctx-pb-add-price")
                                .px(px(12.)).py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("Add Price Entry")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, window: &mut Window, cx| {
                                        this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                        this.open_form_for(pid, name_for_add.clone(), cx);
                                        if let Some(fh) = this.form.as_ref().map(|f| f.read(cx).focus_handle(cx)) {
                                            window.defer(cx, move |window, cx| { window.focus(&fh, cx); });
                                        }
                                    }),
                                )
                        })
                        .when(allow_price_edit && has_latest, |menu| {
                            let hover_bg    = rgb(c.surface_hover);
                            let entry       = latest_entry.clone();
                            let pname_clone = product_name_clone.clone();
                            menu.child(div().h(px(1.)).bg(rgb(c.surface_default)))
                                .child(
                                    div()
                                        .id("ctx-pb-edit-price")
                                        .px(px(12.)).py(px(8.))
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg))
                                        .text_size(rems(1.))
                                        .text_color(rgb(c.text_default))
                                        .child("Edit Latest Price Entry")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _: &MouseDownEvent, window: &mut Window, cx| {
                                                this.store.update(cx, |s, cx| s.clear_context_menu(cx));
                                                if let Some(entry) = entry.clone() {
                                                    this.open_edit_form_for(pid, pname_clone.clone(), entry, cx);
                                                    if let Some(fh) = this.form.as_ref().map(|f| f.read(cx).focus_handle(cx)) {
                                                        window.defer(cx, move |window, cx| { window.focus(&fh, cx); });
                                                    }
                                                }
                                            }),
                                        )
                                )
                        })
                        .when(allow_delete && has_latest, |menu| {
                            let hover_bg = rgb(c.surface_hover);
                            let entry_id = latest_entry.as_ref().map(|e| e.id).unwrap_or(0);
                            menu.child(div().h(px(1.)).bg(rgb(c.surface_default)))
                                .child(
                                    div()
                                        .id("ctx-pb-delete-price")
                                        .px(px(12.)).py(px(8.))
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg))
                                        .text_size(rems(1.))
                                        .text_color(rgb(c.status_red))
                                        .child("Delete Latest Price Entry")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                                this.store.update(cx, |s, cx| {
                                                    s.clear_context_menu(cx);
                                                    s.delete_price_entry(entry_id, pid, cx);
                                                });
                                            }),
                                        )
                                )
                        })
                );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricebook_panel_event_show_price_history_carries_data() {
        let ev = PriceBookPanelEvent::ShowPriceHistory {
            product_id: 3,
            name:       "DVR System".to_string(),
        };
        match ev {
            PriceBookPanelEvent::ShowPriceHistory { product_id, name } => {
                assert_eq!(product_id, 3);
                assert_eq!(name, "DVR System");
            }
        }
    }
}
