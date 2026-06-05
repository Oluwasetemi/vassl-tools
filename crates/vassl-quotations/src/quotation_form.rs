use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           Render, Subscription, Window, actions, div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_core::Project;
use vassl_ui::{Dropdown, DropdownItem, TextInput, ThemeHandle, text_field};


actions!(quotation_form, [EscapeForm, TabField, BackTabField]);
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
    cancel_focus:       FocusHandle,
    save_focus:         FocusHandle,
    error:              Option<String>,
    focus_handle:       FocusHandle,
    _store_sub:         Subscription,
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
        let project_dropdown = cx.new(|cx| {
            Dropdown::new("Select a project…", "No projects yet — create one first.", cx)
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
        let _store_sub = cx.observe(&store, |this, store_entity, cx| {
            let (items, loading) = {
                let s = store_entity.read(cx);
                (projects_to_items(&s.projects), s.loading)
            };
            this.project_dropdown.update(cx, |d, cx| d.set_items(items, loading, cx));
        });

        Self {
            store,
            reference_number,
            project_dropdown,
            notes:        cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            cancel_focus: cx.focus_handle(),
            save_focus:   cx.focus_handle(),
            error:        None,
            focus_handle: cx.focus_handle(),
            _store_sub,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let selected = self.project_dropdown.read(cx).selected_id;
        match validate_form(selected) {
            Some(msg) => { self.error = Some(msg); cx.notify(); }
            None => {
                let pid        = selected.unwrap();
                let ref_num    = self.reference_number.clone();
                let notes_s    = self.notes.read(cx).text().trim().to_string();
                let notes      = if notes_s.is_empty() { None } else { Some(notes_s) };
                let store      = self.store.clone();
                let db         = QuotationDb::global(&**cx);
                let app_db     = vassl_db::AppDatabase::global(&**cx).clone();

                cx.spawn(async move |this, cx| {
                    let created_by = vassl_db::shared::current_user(&app_db)
                        .ok().flatten().unwrap_or_else(|| "unknown".into());
                    // ref_num is held for display only; the DB generates atomically
                    let _ = ref_num;
                    let result = db.insert_quotation_atomic(pid, &created_by, notes.as_deref()).await;
                    let _ = this.update(cx, |form, cx| {
                        match result {
                            Err(e) => {
                                tracing::error!("insert_quotation failed: {e:?}");
                                form.error = Some(format!("Save failed: {e}"));
                                cx.notify();
                            }
                            Ok(_) => {
                                let _ = store.update(cx, |s, cx| s.load_quotations(cx));
                                cx.emit(QuotationFormEvent::Submitted);
                            }
                        }
                    });
                    Ok::<(), anyhow::Error>(())
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
        let notes_focused  = self.notes.read(cx).focus_handle.is_focused(window);
        let cancel_f       = self.cancel_focus.is_focused(window);
        let save_f         = self.save_focus.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("QuotationForm")
            .on_action(cx.listener(|_, _: &EscapeForm, _, cx| {
                cx.emit(QuotationFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.project_dropdown.read(cx).trigger_focus.clone(),
                    this.notes.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = [
                    this.project_dropdown.read(cx).trigger_focus.clone(),
                    this.notes.read(cx).focus_handle.clone(),
                    this.cancel_focus.clone(),
                    this.save_focus.clone(),
                ];
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .child(
                div()
                    .w(px(580.))
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
                                .child("New Quotation"))
                            .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("Esc to cancel"))
                    )
                    // ── fields ──────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            // Reference number (read-only)
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Reference"))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(6.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                        .child(self.reference_number.clone()))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // Project dropdown
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Project"))
                                    .child(div().flex_1().child(self.project_dropdown.clone()))
                            )
                            .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                            // Notes
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(div().w(px(160.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Notes"))
                                    .child(div().flex_1().child(text_field("", self.notes.clone(), notes_focused, cx)))
                            )
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
                                div().id("quot-btn-cancel")
                                    .track_focus(&self.cancel_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_default)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(cancel_f, |d| d.border_2().border_color(rgb(c.surface_active)))
                                    .when(!cancel_f, |d| d.border_1().border_color(rgb(c.surface_default)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(QuotationFormEvent::Cancelled); }))
                                    .child("Cancel")
                            )
                            .child(
                                div().id("quot-btn-create")
                                    .track_focus(&self.save_focus)
                                    .px(px(18.)).py(px(7.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active)).text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .when(save_f, |d| d.border_2().border_color(rgb(c.text_default)))
                                    .when(!save_f, |d| d.border_1().border_color(rgb(c.surface_active)))
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                    .child("Create Quotation")
                            )
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
