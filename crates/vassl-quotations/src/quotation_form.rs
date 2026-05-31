use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           MouseButton, MouseDownEvent, Render, Window, actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::Project;
use vassl_ui::{TextInput, text_field};

use crate::colors;

actions!(quotation_form, [EscapeForm]);
use crate::db::QuotationDb;
use crate::store::QuotationStore;

#[derive(Debug)]
pub enum QuotationFormEvent { Submitted, Cancelled }

impl EventEmitter<QuotationFormEvent> for QuotationForm {}

pub struct QuotationForm {
    store:            Entity<QuotationStore>,
    reference_number: String,
    projects:         Vec<Project>,
    selected_project: Option<i64>,
    notes:            Entity<TextInput>,
    error:            Option<String>,
    focus_handle:     FocusHandle,
}

pub fn validate_form(selected_project: Option<i64>) -> Option<String> {
    if selected_project.is_none() { Some("Please select a project.".to_string()) } else { None }
}

impl QuotationForm {
    pub fn new(store: Entity<QuotationStore>, reference_number: String, projects: Vec<Project>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            reference_number,
            projects,
            selected_project: None,
            notes:            cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            error:            None,
            focus_handle:     cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match validate_form(self.selected_project) {
            Some(msg) => { self.error = Some(msg); cx.notify(); }
            None => {
                let pid      = self.selected_project.unwrap();
                let ref_num  = self.reference_number.clone();
                let notes_s  = self.notes.read(cx).text().trim().to_string();
                let notes    = if notes_s.is_empty() { None } else { Some(notes_s) };
                let store    = self.store.clone();
                let db       = QuotationDb::global(&**cx);

                cx.spawn(async move |this, cx| {
                    let result = db.insert_quotation_with_notes(pid, ref_num, "user", notes.as_deref()).await;
                    if let Err(e) = result { tracing::error!("insert_quotation failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_quotations(cx));
                    this.update(cx, |_, cx| cx.emit(QuotationFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for QuotationForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for QuotationForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notes_focused = self.notes.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("QuotationForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(QuotationFormEvent::Cancelled);
            }))
            .child(
                div()
                    .w(px(460.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT)).child("New Quotation"))
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Reference Number"))
                            .child(div().px(px(8.)).py(px(6.)).bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                .text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .child(self.reference_number.clone()))
                    )
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Select Project"))
                            .child(
                                div().id("project-picker").h(px(120.)).overflow_y_scroll()
                                    .bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                    .children(self.projects.iter().map(|p| {
                                        let pid      = p.id;
                                        let selected = self.selected_project == Some(pid);
                                        let bg       = if selected { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT };
                                        div()
                                            .id(format!("pick-project-{pid}"))
                                            .flex().flex_row().items_center()
                                            .px(px(8.)).py(px(5.))
                                            .bg(rgb(bg)).cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                                this.selected_project = Some(pid);
                                                this.error = None;
                                                cx.notify();
                                            }))
                                            .child(div().flex_1().text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT)).child(p.name.clone()))
                                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child(p.client_name.clone()))
                                    }))
                            )
                    )
                    .child(text_field("Notes (optional)", self.notes.clone(), notes_focused, window))
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("quot-btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(QuotationFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("quot-btn-create").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Create"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reference_number_format()       { let r = "VASSL-2026-0001"; assert!(r.starts_with("VASSL-")); assert_eq!(r.len(), 15); }
    #[test] fn form_requires_project()         { assert!(validate_form(None).is_some()); }
    #[test] fn form_valid_with_project()       { assert!(validate_form(Some(1)).is_none()); }
}
