use gpui::{Context, Entity, IntoElement, Render, Subscription, Window,
           div, prelude::*, px, rgb};
use vassl_ui::ThemeHandle;

use crate::store::SupplierStore;
use crate::supplier_form::{SupplierForm, SupplierFormEvent};
use crate::supplier_list::SupplierList;
use crate::SupplierStoreHandle;

pub struct SupplierPanel {
    store:         Entity<SupplierStore>,
    supplier_list: Entity<SupplierList>,
    form:          Option<Entity<SupplierForm>>,
    _form_sub:     Option<Subscription>,
}

impl SupplierPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store         = cx.global::<SupplierStoreHandle>().0.clone();
        let supplier_list = cx.new(|cx| SupplierList::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_suppliers(cx));
        Self { store, supplier_list, form: None, _form_sub: None }
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c             = cx.global::<ThemeHandle>().0.clone();
        let has_selection = self.store.read(cx).selected_supplier_id.is_some();

        let mut root = div()
            .relative()
            .flex_1().flex().flex_col().h_full()
            .child(
                div()
                    .flex().flex_row().items_center().gap(px(8.))
                    .px(px(16.)).py(px(8.))
                    .bg(rgb(c.canvas_bg))
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
            .child(self.supplier_list.clone());

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }

        root
    }
}
