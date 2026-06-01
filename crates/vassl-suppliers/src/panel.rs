use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::store::SupplierStore;
use crate::supplier_form::{SupplierForm, SupplierFormEvent};
use crate::supplier_list::SupplierList;

pub struct SupplierPanel {
    store:        Entity<SupplierStore>,
    list:         Entity<SupplierList>,
    form:         Option<Entity<SupplierForm>>,
    _form_sub:    Option<Subscription>,
    search_input: Entity<TextInput>,
}

impl SupplierPanel {
    pub fn new(store: Entity<SupplierStore>, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| SupplierList::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_suppliers(cx));
        let search_input = cx.new(|cx| TextInput::with_placeholder("Filter…", cx));
        Self { store, list, form: None, _form_sub: None, search_input }
    }

    fn open_new_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let form  = cx.new(|cx| SupplierForm::new(self.store.clone(), cx));
        let first = form.read(cx).name.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        self.wire_form_sub(form, cx);
    }

    fn open_edit_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() { return; }
        let supplier = {
            let store = self.store.read(cx);
            let Some(id) = store.selected_supplier_id else { return; };
            store.suppliers.iter().find(|s| s.id == id).cloned()
        };
        let Some(supplier) = supplier else { return; };
        let form  = cx.new(|cx| SupplierForm::edit(self.store.clone(), &supplier, cx));
        let first = form.read(cx).name.read(cx).focus_handle.clone();
        window.focus(&first, cx);
        self.wire_form_sub(form, cx);
    }

    fn wire_form_sub(&mut self, form: gpui::Entity<SupplierForm>, cx: &mut Context<Self>) {
        let sub = cx.subscribe(&form, |this, _form, ev: &SupplierFormEvent, cx| {
            match ev {
                SupplierFormEvent::Submitted | SupplierFormEvent::Cancelled => {
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

impl Render for SupplierPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c             = cx.global::<ThemeHandle>().0.clone();
        let has_selection = self.store.read(cx).selected_supplier_id.is_some();

        // Sync filter input → store
        let q = self.search_input.read(cx).text().to_string();
        if q != self.store.read(cx).search_query {
            self.store.update(cx, |s, cx| s.set_search_query(q.clone(), cx));
        }
        let has_query = !q.is_empty();

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child(
                        div()
                            .flex().flex_row().items_center().gap(px(4.))
                            .child(
                                div()
                                    .w(px(160.))
                                    .child({
                                        let focused = self.search_input.read(cx).focus_handle.is_focused(window);
                                        text_field("", self.search_input.clone(), focused, cx)
                                    })
                            )
                            .child({
                                let mut clear = div()
                                    .id("sup-search-clear")
                                    .px(px(6.)).py(px(2.)).rounded(px(3.))
                                    .text_size(px(11.)).text_color(rgb(c.text_muted))
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
                    .child(
                        div()
                            .id("sup-btn-new")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_new_form(window, cx);
                            }))
                            .child("+ New Supplier")
                    )
                    .child({
                        let mut btn = div()
                            .id("sup-btn-edit")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(px(12.)).text_color(rgb(c.text_default))
                            .child("Edit");
                        if has_selection {
                            btn = btn
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                                    this.open_edit_form(window, cx);
                                }));
                        }
                        btn
                    })
            )
            .child(self.list.clone());

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        root
    }
}
