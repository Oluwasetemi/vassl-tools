use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::Supplier;
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::db::SupplierDb;
use crate::store::SupplierStore;

actions!(supplier_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum SupplierFormEvent { Submitted, Cancelled }
impl EventEmitter<SupplierFormEvent> for SupplierForm {}

pub struct SupplierForm {
    store:          Entity<SupplierStore>,
    editing_id:     Option<i64>,
    pub name:       Entity<TextInput>,
    contact_person: Entity<TextInput>,
    email:          Entity<TextInput>,
    phone:          Entity<TextInput>,
    notes:          Entity<TextInput>,
    error:          Option<String>,
    focus_handle:   FocusHandle,
}

fn validate_supplier_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Name is required.".to_string()); }
    Ok(name)
}

impl SupplierForm {
    pub fn new(store: Entity<SupplierStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            editing_id:     None,
            name:           cx.new(|cx| TextInput::with_placeholder("e.g. Sony Electronics", cx)),
            contact_person: cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            email:          cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            phone:          cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            notes:          cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            error:          None,
            focus_handle:   cx.focus_handle(),
        }
    }

    pub fn edit(store: Entity<SupplierStore>, supplier: &Supplier, cx: &mut Context<Self>) -> Self {
        let name_f    = cx.new(|cx| TextInput::with_placeholder("e.g. Sony Electronics", cx));
        let contact_f = cx.new(|cx| TextInput::with_placeholder("optional", cx));
        let email_f   = cx.new(|cx| TextInput::with_placeholder("optional", cx));
        let phone_f   = cx.new(|cx| TextInput::with_placeholder("optional", cx));
        let notes_f   = cx.new(|cx| TextInput::with_placeholder("optional", cx));

        name_f.update(cx, |t, cx| t.set_text(supplier.name.clone(), cx));
        if let Some(v) = &supplier.contact_person {
            contact_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }
        if let Some(v) = &supplier.email {
            email_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }
        if let Some(v) = &supplier.phone {
            phone_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }
        if let Some(v) = &supplier.notes {
            notes_f.update(cx, |t, cx| t.set_text(v.clone(), cx));
        }

        Self {
            store,
            editing_id:     Some(supplier.id),
            name:           name_f,
            contact_person: contact_f,
            email:          email_f,
            phone:          phone_f,
            notes:          notes_f,
            error:          None,
            focus_handle:   cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name_raw   = self.name.read(cx).text().to_string();
        let contact    = self.contact_person.read(cx).text().trim().to_string();
        let email      = self.email.read(cx).text().trim().to_string();
        let phone      = self.phone.read(cx).text().trim().to_string();
        let notes      = self.notes.read(cx).text().trim().to_string();
        let contact_op = if contact.is_empty() { None } else { Some(contact) };
        let email_op   = if email.is_empty()   { None } else { Some(email) };
        let phone_op   = if phone.is_empty()   { None } else { Some(phone) };
        let notes_op   = if notes.is_empty()   { None } else { Some(notes) };

        match validate_supplier_name(&name_raw) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok(name) => {
                self.error = None;
                cx.notify();
                let db         = SupplierDb::global(&**cx);
                let store      = self.store.clone();
                let editing_id = self.editing_id;

                cx.spawn(async move |this, cx| {
                    let result = if let Some(id) = editing_id {
                        db.update_supplier(id, &name, contact_op.as_deref(), email_op.as_deref(), phone_op.as_deref(), notes_op.as_deref()).await
                            .map(|_| ())
                    } else {
                        db.insert_supplier(&name, contact_op.as_deref(), email_op.as_deref(), phone_op.as_deref(), notes_op.as_deref()).await
                            .map(|_| ())
                    };

                    match result {
                        Err(e) => {
                            let msg = if e.to_string().contains("UNIQUE") {
                                "A supplier with this name already exists.".to_string()
                            } else {
                                format!("Save failed: {e}")
                            };
                            let _ = this.update(cx, |form, cx| { form.error = Some(msg); cx.notify(); });
                        }
                        Ok(()) => {
                            let _ = store.update(cx, |s, cx| s.load_suppliers(cx));
                            let _ = this.update(cx, |_, cx| cx.emit(SupplierFormEvent::Submitted));
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                }).detach();
            }
        }
    }
}

impl Focusable for SupplierForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for SupplierForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c         = cx.global::<ThemeHandle>().0.clone();
        let name_f    = self.name.read(cx).focus_handle.is_focused(window);
        let contact_f = self.contact_person.read(cx).focus_handle.is_focused(window);
        let email_f   = self.email.read(cx).focus_handle.is_focused(window);
        let phone_f   = self.phone.read(cx).focus_handle.is_focused(window);
        let notes_f   = self.notes.read(cx).focus_handle.is_focused(window);
        let title     = if self.editing_id.is_some() { "Edit Supplier" } else { "New Supplier" };
        let save_label= if self.editing_id.is_some() { "Save Changes" } else { "Save Supplier" };

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("SupplierForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(SupplierFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.contact_person.read(cx).focus_handle.clone(),
                    this.email.read(cx).focus_handle.clone(),
                    this.phone.read(cx).focus_handle.clone(),
                    this.notes.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.contact_person.read(cx).focus_handle.clone(),
                    this.email.read(cx).focus_handle.clone(),
                    this.phone.read(cx).focus_handle.clone(),
                    this.notes.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(540.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1().text_size(px(13.)).text_color(rgb(c.text_default)).child(title))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Name"))
                                    .child(div().flex_1().child(text_field("", self.name.clone(), name_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Contact Person"))
                                    .child(div().flex_1().child(text_field("", self.contact_person.clone(), contact_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Email"))
                                    .child(div().flex_1().child(text_field("", self.email.clone(), email_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Phone"))
                                    .child(div().flex_1().child(text_field("", self.phone.clone(), phone_f, cx)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_start().py(px(10.))
                                    .child(div().w(px(160.)).pt(px(6.)).text_size(px(12.)).text_color(rgb(c.text_muted)).child("Notes"))
                                    .child(div().flex_1().h(px(64.)).child(text_field("", self.notes.clone(), notes_f, cx)))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(px(11.)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1().border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("sup-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| cx.emit(SupplierFormEvent::Cancelled)))
                                .child("Cancel"))
                            .child(div().id("sup-btn-save").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| this.submit(cx)))
                                .child(save_label))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_name() {
        assert!(validate_supplier_name("").is_err());
    }

    #[test]
    fn rejects_whitespace_only_name() {
        assert!(validate_supplier_name("   ").is_err());
    }

    #[test]
    fn accepts_valid_name() {
        let result = validate_supplier_name("  Acme Ltd  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Acme Ltd");
    }
}
