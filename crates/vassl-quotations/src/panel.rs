use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};

use crate::colors;
use crate::quotation_detail::QuotationDetail;
use crate::quotation_form::{QuotationForm, QuotationFormEvent};
use crate::quotation_list::QuotationList;
use crate::store::QuotationStore;
use crate::QuotationStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab { Quotations, Items }

pub struct QuotationPanel {
    store:       Entity<QuotationStore>,
    quot_list:   Entity<QuotationList>,
    quot_detail: Entity<QuotationDetail>,
    active_tab:  Tab,
    form:        Option<Entity<QuotationForm>>,
    _form_sub:   Option<Subscription>,
}

impl QuotationPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store       = cx.global::<QuotationStoreHandle>().0.clone();
        let quot_list   = cx.new(|cx| QuotationList::new(store.clone(), cx));
        let quot_detail = cx.new(|cx| QuotationDetail::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_quotations(cx));
        Self {
            store,
            quot_list,
            quot_detail,
            active_tab: Tab::Quotations,
            form:       None,
            _form_sub:  None,
        }
    }

    fn open_form(&mut self, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let (ref_num, projects) = {
            let store    = self.store.read(cx);
            let db       = crate::db::QuotationDb::global(&**cx);
            let ref_num  = db.next_reference_number().unwrap_or_else(|_| "VASSL-ERR-0000".to_string());
            (ref_num, store.projects.clone())
        };
        let form = cx.new(|cx| QuotationForm::new(self.store.clone(), ref_num, projects, cx));
        let sub  = cx.subscribe(&form, |this, _form, ev: &QuotationFormEvent, cx| {
            match ev {
                QuotationFormEvent::Submitted | QuotationFormEvent::Cancelled => {
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

impl Render for QuotationPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        let content = div().flex_1().h_full().flex().flex_col();
        let content = match active_tab {
            Tab::Quotations => content.child(self.quot_list.clone()),
            Tab::Items      => content.child(self.quot_detail.clone()),
        };

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(colors::CANVAS_BG))
                    .child(
                        div()
                            .id("quot-tab-quotations")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Quotations { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Quotations;
                                cx.notify();
                            }))
                            .child("Quotations")
                    )
                    .child(
                        div()
                            .id("quot-tab-items")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if active_tab == Tab::Items { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT }))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.active_tab = Tab::Items;
                                cx.notify();
                            }))
                            .child("Items")
                    )
                    .child(div().flex_1())
                    // New Quotation button — always enabled (form has inline project picker)
                    .child(
                        div()
                            .id("quot-btn-new")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(colors::SURFACE_ACTIVE))
                            .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                this.open_form(cx);
                            }))
                            .child("+ New Quotation")
                    )
            )
            .child(content);

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        root
    }
}
