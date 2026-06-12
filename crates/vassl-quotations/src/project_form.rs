use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_ui::{TextInput, ThemeHandle};

use crate::db::QuotationDb;
use crate::store::QuotationStore;

actions!(project_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum ProjectFormEvent { Submitted, Cancelled }

impl EventEmitter<ProjectFormEvent> for ProjectForm {}

pub struct ProjectForm {
    store:             Entity<QuotationStore>,
    pub name:          Entity<TextInput>,
    client_name:       Entity<TextInput>,
    client_address:    Entity<TextInput>,
    client_attn:       Entity<TextInput>,
    client_tel:        Entity<TextInput>,
    cancel_focus:      FocusHandle,
    save_focus:        FocusHandle,
    error:             Option<String>,
    name_error:        bool,
    client_name_error: bool,
}

pub fn validate_project(name: &str, client_name: &str) -> Result<(String, String), String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Project name is required.".to_string()); }
    let client = client_name.trim().to_string();
    if client.is_empty() { return Err("Client name is required.".to_string()); }
    Ok((name, client))
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

impl ProjectForm {
    pub fn new(store: Entity<QuotationStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            name:           cx.new(|cx| TextInput::with_placeholder("e.g. Office Renovation", cx)),
            client_name:    cx.new(|cx| TextInput::with_placeholder("e.g. Acme Corp", cx)),
            client_address: cx.new(|cx| TextInput::with_placeholder("e.g. 12 Main St, Kingston (optional)", cx)),
            client_attn:    cx.new(|cx| TextInput::with_placeholder("e.g. Jane Smith (optional)", cx)),
            client_tel:     cx.new(|cx| TextInput::with_placeholder("e.g. 876-555-0100 (optional)", cx)),
            cancel_focus:      cx.focus_handle(),
            save_focus:        cx.focus_handle(),
            error:             None,
            name_error:        false,
            client_name_error: false,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let n   = self.name.read(cx).text().to_string();
        let cl  = self.client_name.read(cx).text().to_string();
        let adr = self.client_address.read(cx).text().to_string();
        let att = self.client_attn.read(cx).text().to_string();
        let tel = self.client_tel.read(cx).text().to_string();
        self.name_error        = n.trim().is_empty();
        self.client_name_error = cl.trim().is_empty();
        match validate_project(&n, &cl) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((name, client)) => {
                let db    = QuotationDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_project(name, client, opt(&adr), opt(&att), opt(&tel)).await;
                    if let Err(e) = result { tracing::error!("insert_project failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_quotations(cx));
                    this.update(cx, |_, cx| cx.emit(ProjectFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for ProjectForm {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle { self.name.read(cx).focus_handle.clone() }
}

impl Render for ProjectForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c       = cx.global::<ThemeHandle>().0.clone();
        let name_f  = self.name.read(cx).focus_handle.is_focused(window);
        let cli_f   = self.client_name.read(cx).focus_handle.is_focused(window);
        let adr_f   = self.client_address.read(cx).focus_handle.is_focused(window);
        let att_f   = self.client_attn.read(cx).focus_handle.is_focused(window);
        let tel_f   = self.client_tel.read(cx).focus_handle.is_focused(window);
        let cancel_f = self.cancel_focus.is_focused(window);
        let save_f   = self.save_focus.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("ProjectForm")
            .on_action(cx.listener(|_, _: &EscapeForm, window, cx| {
                let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                window.focus(&root, cx);
                cx.emit(ProjectFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.client_name.read(cx).focus_handle.clone(),
                    this.client_address.read(cx).focus_handle.clone(),
                    this.client_attn.read(cx).focus_handle.clone(),
                    this.client_tel.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.client_name.read(cx).focus_handle.clone(),
                    this.client_address.read(cx).focus_handle.clone(),
                    this.client_attn.read(cx).focus_handle.clone(),
                    this.client_tel.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(560.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .rounded_t(px(10.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1()
                                .text_size(rems(1.)).text_color(rgb(c.text_default))
                                .child("New Project"))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(field_row("Project Name", self.name.clone(), name_f, self.name_error, cx, &c))
                            .child(divider(&c))
                            .child(field_row("Client Name", self.client_name.clone(), cli_f, self.client_name_error, cx, &c))
                            .child(divider(&c))
                            .child(field_row("Address", self.client_address.clone(), adr_f, false, cx, &c))
                            .child(divider(&c))
                            .child(field_row("Attn", self.client_attn.clone(), att_f, false, cx, &c))
                            .child(divider(&c))
                            .child(field_row("Tel", self.client_tel.clone(), tel_f, false, cx, &c))
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    // ── footer ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end().gap(px(8.))
                            .child(
                                div().id("proj-btn-cancel")
                                    .track_focus(&self.cancel_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_default)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(cancel_f, |d| d.border_2().border_color(rgb(c.surface_active)))
                                    .when(!cancel_f, |d| d.border_1().border_color(rgb(c.surface_default)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, window, cx| {
                                        let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                                        window.focus(&root, cx);
                                        cx.emit(ProjectFormEvent::Cancelled);
                                    }))
                                    .child("Cancel")
                            )
                            .child(
                                div().id("proj-btn-save")
                                    .track_focus(&self.save_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(save_f, |d| d.border_2().border_color(rgb(c.text_default)))
                                    .when(!save_f, |d| d.border_1().border_color(rgb(c.surface_active)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                    .child("Create Project")
                            )
                    )
            )
    }
}

fn field_row(
    label: &str,
    input: Entity<TextInput>,
    focused: bool,
    error: bool,
    cx: &gpui::App,
    c: &vassl_ui::ThemeColors,
) -> impl gpui::IntoElement {
    use vassl_ui::text_field;
    let label_color = if error { c.status_red } else { c.text_default };
    div().flex().flex_row().items_center().py(px(10.))
        .child(div().w(px(140.)).text_size(rems(0.923)).text_color(rgb(label_color)).child(label.to_string()))
        .child(div().flex_1().child(text_field("", input, focused, error, cx)))
}

fn divider(c: &vassl_ui::ThemeColors) -> impl gpui::IntoElement {
    div().h(px(1.)).bg(rgb(c.surface_default))
}

#[cfg(test)]
mod tests {
    use super::validate_project;
    #[test] fn rejects_empty_name()   { assert!(validate_project("", "Acme").is_err()); }
    #[test] fn rejects_empty_client() { assert!(validate_project("Alpha", "").is_err()); }
    #[test] fn rejects_both_empty()   { assert!(validate_project("", "").is_err()); }
    #[test] fn accepts_valid()        {
        let (n, c) = validate_project("  Alpha  ", "  Acme  ").unwrap();
        assert_eq!(n, "Alpha");
        assert_eq!(c, "Acme");
    }
}
