use gpui::{Context, Entity, IntoElement, MouseButton, MouseDownEvent, Render, Subscription, Window,
           div, prelude::*, px, rems, rgb};
use vassl_ui::{AppSettings, NewRecord, TextInput, ThemeHandle, text_field, tooltip_keyed};

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

        cx.observe(&search_input, |this, input, cx| {
            let q = input.read(cx).text().to_string();
            this.store.update(cx, |s, cx| s.set_search_query(q, cx));
        }).detach();

        Self { store, list, form: None, _form_sub: None, search_input }
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_next(cx)) {
            self.list.update(cx, |l, _| l.scroll_handle.scroll_to_item(idx, gpui::ScrollStrategy::Top));
        }
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_prev(cx)) {
            self.list.update(cx, |l, _| l.scroll_handle.scroll_to_item(idx, gpui::ScrollStrategy::Top));
        }
    }

    pub fn create_new_form(&mut self, cx: &mut Context<Self>) -> Option<gpui::FocusHandle> {
        if self.form.is_some() { return None; }
        let form  = cx.new(|cx| SupplierForm::new(self.store.clone(), cx));
        let first = form.read(cx).name.read(cx).focus_handle.clone();
        self.wire_form_sub(form, cx);
        Some(first)
    }

    pub fn open_new_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fh) = self.create_new_form(cx) {
            window.focus(&fh, cx);
        }
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

        #[cfg(target_os = "macos")]
        let mod_key = "⌘";
        #[cfg(not(target_os = "macos"))]
        let mod_key = "Ctrl+";

        let has_query = !self.search_input.read(cx).text().is_empty();

        let mut root = div()
            .key_context("SupplierPanel")
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| {
                this.open_new_form(window, cx);
            }))
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
                                        text_field("", self.search_input.clone(), focused, false, cx)
                                    })
                            )
                            .child({
                                let mut clear = div()
                                    .id("sup-search-clear")
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
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("sup-btn-new")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .hover(move |s| s.bg(hover_bg))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, cx| {
                                this.open_new_form(window, cx);
                            }))
                            .tooltip(tooltip_keyed("New Supplier", format!("{mod_key}N")))
                            .child("+ New Supplier")
                    })
                    .child({
                        let mut btn = div()
                            .id("sup-btn-edit")
                            .px(px(12.)).py(px(4.)).rounded(px(4.))
                            .bg(rgb(if has_selection { c.surface_active } else { c.surface_default }))
                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
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

        // Context menu overlay
        let allow_delete = cx.global::<AppSettings>().allow_delete;
        let ctx_menu = self.store.read(cx).context_menu.clone();
        if let Some(target) = ctx_menu {
            let viewport = window.viewport_size();
            const MENU_W: f32 = 220.0;
            const MENU_H: f32 = 120.0;
            let menu_x = target.x.min((viewport.width.as_f32()  - MENU_W).max(0.0));
            let menu_y = target.y.min((viewport.height.as_f32() - MENU_H).max(0.0));
            let sid = target.supplier_id;

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
                                .child(target.supplier_name.clone())
                        )
                        .when(allow_delete, |menu| {
                            let hover_bg = rgb(c.surface_hover);
                            menu.child(div().h(px(1.)).bg(rgb(c.surface_default)))
                                .child(
                                    div()
                                        .id("ctx-sup-delete")
                                        .px(px(12.)).py(px(8.))
                                        .cursor_pointer()
                                        .hover(move |s| s.bg(hover_bg))
                                        .text_size(rems(1.))
                                        .text_color(rgb(c.status_red))
                                        .child("Delete Supplier")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                                this.store.update(cx, |s, cx| {
                                                    s.clear_context_menu(cx);
                                                    s.delete_supplier(sid, cx);
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
