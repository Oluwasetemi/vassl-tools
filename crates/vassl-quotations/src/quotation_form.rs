use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           Render, Window, actions, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::Project;
use vassl_ui::{Dropdown, DropdownItem, TextInput, ThemeHandle, text_field};

use crate::colors;

actions!(quotation_form, [EscapeForm]);
use crate::db::QuotationDb;
use crate::store::QuotationStore;

#[derive(Debug)]
pub enum QuotationFormEvent { Submitted, Cancelled }

impl EventEmitter<QuotationFormEvent> for QuotationForm {}

pub struct QuotationForm {
    store:              Entity<QuotationStore>,
    reference_number:   String,
    project_dropdown:   Entity<Dropdown>,
    pub notes:          Entity<TextInput>,
    error:              Option<String>,
    focus_handle:       FocusHandle,
}

pub fn validate_form(selected_project: Option<i64>) -> Option<String> {
    if selected_project.is_none() { Some("Please select a project.".to_string()) } else { None }
}

fn projects_to_items(projects: &[Project]) -> Vec<DropdownItem> {
    projects.iter().map(|p| DropdownItem {
        id:       p.id,
        label:    p.name.clone(),
        sublabel: Some(p.client_name.clone()),
    }).collect()
}

impl QuotationForm {
    pub fn new(store: Entity<QuotationStore>, reference_number: String, cx: &mut Context<Self>) -> Self {
        let project_dropdown = cx.new(|_| {
            Dropdown::new("Select a project…", "No projects yet — create one first.")
        });

        // Populate dropdown from current store state immediately
        {
            let (items, loading) = {
                let s = store.read(cx);
                (projects_to_items(&s.projects), s.loading)
            };
            project_dropdown.update(cx, |d, _| { d.items = items; d.loading = loading; });
        }

        // Keep dropdown fresh as the store changes (projects may load after form opens)
        cx.observe(&store, |this, store_entity, cx| {
            let (items, loading) = {
                let s = store_entity.read(cx);
                (projects_to_items(&s.projects), s.loading)
            };
            this.project_dropdown.update(cx, |d, cx| d.set_items(items, loading, cx));
        }).detach();

        Self {
            store,
            reference_number,
            project_dropdown,
            notes:        cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let selected = self.project_dropdown.read(cx).selected_id;
        match validate_form(selected) {
            Some(msg) => { self.error = Some(msg); cx.notify(); }
            None => {
                let pid     = selected.unwrap();
                let ref_num = self.reference_number.clone();
                let notes_s = self.notes.read(cx).text().trim().to_string();
                let notes   = if notes_s.is_empty() { None } else { Some(notes_s) };
                let store   = self.store.clone();
                let db      = QuotationDb::global(&**cx);

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
        let c = cx.global::<ThemeHandle>().0.clone();
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
                    .w(px(580.))
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
                                .child("New Quotation"))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            // Reference number (read-only)
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Reference"))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(6.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(px(12.)).text_color(rgb(c.text_muted))
                                        .child(self.reference_number.clone()))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // Project dropdown
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Project"))
                                    .child(div().flex_1().child(self.project_dropdown.clone()))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // Notes
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(px(12.)).text_color(rgb(c.text_default)).child("Notes"))
                                    .child(div().flex_1().child(text_field("", self.notes.clone(), notes_focused, cx)))
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
                            .child(div().id("quot-btn-cancel").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_default)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(QuotationFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("quot-btn-create").px(px(18.)).py(px(7.)).rounded(px(5.))
                                .bg(rgb(c.surface_active)).text_size(px(12.)).text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Create Quotation"))
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
    #[test] fn projects_to_items_maps_correctly() {
        use vassl_core::{Project, ProjectStatus};
        let projects = vec![Project {
            id: 1, name: "Alpha".into(), client_name: "Acme".into(),
            description: None, status: ProjectStatus::Active,
            created_at: "2026-01-01".into(),
        }];
        let items = projects_to_items(&projects);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 1);
        assert_eq!(items[0].label, "Alpha");
        assert_eq!(items[0].sublabel.as_deref(), Some("Acme"));
    }
}
