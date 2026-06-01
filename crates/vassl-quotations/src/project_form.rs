use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::colors;
use crate::db::QuotationDb;
use crate::store::QuotationStore;

actions!(project_form, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum ProjectFormEvent { Submitted, Cancelled }

impl EventEmitter<ProjectFormEvent> for ProjectForm {}

pub struct ProjectForm {
    store:        Entity<QuotationStore>,
    name:         Entity<TextInput>,
    client_name:  Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

pub fn validate_project(name: &str, client_name: &str) -> Result<(String, String), String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Project name is required.".to_string()); }
    let client = client_name.trim().to_string();
    if client.is_empty() { return Err("Client name is required.".to_string()); }
    Ok((name, client))
}

impl ProjectForm {
    pub fn new(store: Entity<QuotationStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            name:         cx.new(|cx| TextInput::with_placeholder("e.g. Office Renovation", cx)),
            client_name:  cx.new(|cx| TextInput::with_placeholder("e.g. Acme Corp", cx)),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let n  = self.name.read(cx).text().to_string();
        let cl = self.client_name.read(cx).text().to_string();
        match validate_project(&n, &cl) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((name, client)) => {
                let db    = QuotationDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_project(name, client).await;
                    if let Err(e) = result { tracing::error!("insert_project failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_quotations(cx));
                    this.update(cx, |_, cx| cx.emit(ProjectFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for ProjectForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for ProjectForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c            = cx.global::<ThemeHandle>().0.clone();
        let name_focused = self.name.read(cx).focus_handle.is_focused(window);
        let cli_focused  = self.client_name.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("ProjectForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(ProjectFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.client_name.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.name.read(cx).focus_handle.clone(),
                    this.client_name.read(cx).focus_handle.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(520.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1()
                                .text_size(px(13.)).text_color(rgb(c.text_default))
                                .child("New Project"))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(140.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Project Name"))
                                    .child(div().flex_1().child(text_field("", self.name.clone(), name_focused, window)))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(140.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Client Name"))
                                    .child(div().flex_1().child(text_field("", self.client_name.clone(), cli_focused, window)))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(px(11.)).text_color(rgb(c.status_red))
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
                            .child(div().id("proj-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(ProjectFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("proj-btn-save").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Create Project"))
                    )
            )
    }
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
