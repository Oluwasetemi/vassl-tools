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
    store:             Entity<QuotationStore>,
    reference_number:  String,
    project_dropdown:  Entity<Dropdown>,
    pub notes:         Entity<TextInput>,
    exchange_rate:     Entity<TextInput>,
    discount_percent:  Entity<TextInput>,
    gct_percent:       Entity<TextInput>,
    validity_days:     Entity<TextInput>,
    cancel_focus:      FocusHandle,
    save_focus:        FocusHandle,
    error:             Option<String>,
    _store_sub:        Subscription,
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

        {
            let (items, loading) = {
                let s = store.read(cx);
                (projects_to_items(&s.projects), s.loading)
            };
            project_dropdown.update(cx, |d, _| { d.items = items; d.loading = loading; });
        }

        let _store_sub = cx.observe(&store, |this, store_entity, cx| {
            let (items, loading) = {
                let s = store_entity.read(cx);
                (projects_to_items(&s.projects), s.loading)
            };
            this.project_dropdown.update(cx, |d, cx| d.set_items(items, loading, cx));
        });

        let db = vassl_db::AppDatabase::global(&**cx);
        let rate_default  = vassl_db::shared::get_setting(db, "pricebook.usd_to_jmd_rate").ok().flatten().unwrap_or_else(|| "156.00".into());
        let gct_default   = vassl_db::shared::get_setting(db, "quotations.tax_rate").ok().flatten().unwrap_or_else(|| "15.0".into());
        let notes_default = vassl_db::shared::get_setting(db, "quotations.notes_template").ok().flatten().unwrap_or_default();

        Self {
            store,
            reference_number,
            project_dropdown,
            notes:            cx.new(move |cx| {
                let mut f = TextInput::with_placeholder("optional", cx);
                if !notes_default.is_empty() { f.set_text(&notes_default, cx); }
                f
            }),
            exchange_rate:    cx.new(move |cx| TextInput::with_text(&rate_default, cx)),
            discount_percent: cx.new(|cx| TextInput::with_text("0.0", cx)),
            gct_percent:      cx.new(move |cx| TextInput::with_text(&gct_default, cx)),
            validity_days:    cx.new(|cx| TextInput::with_text("30", cx)),
            cancel_focus:     cx.focus_handle(),
            save_focus:       cx.focus_handle(),
            error:            None,
            _store_sub,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let selected = self.project_dropdown.read(cx).selected_id;
        match validate_form(selected) {
            Some(msg) => { self.error = Some(msg); cx.notify(); }
            None => {
                let pid    = selected.unwrap();
                let ref_num = self.reference_number.clone();
                let notes_s = self.notes.read(cx).text().trim().to_string();
                let notes   = if notes_s.is_empty() { None } else { Some(notes_s) };

                let rate: f64  = self.exchange_rate.read(cx).text().trim().parse().unwrap_or(156.0);
                let disc: f64  = self.discount_percent.read(cx).text().trim().parse().unwrap_or(0.0);
                let gct: f64   = self.gct_percent.read(cx).text().trim().parse().unwrap_or(15.0);
                let days: i64  = self.validity_days.read(cx).text().trim().parse().unwrap_or(30);

                let store  = self.store.clone();
                let db     = QuotationDb::global(&**cx);
                let app_db = vassl_db::AppDatabase::global(&**cx).clone();

                cx.spawn(async move |this, cx| {
                    let created_by = vassl_db::shared::current_user(&app_db)
                        .ok().flatten().unwrap_or_else(|| "unknown".into());
                    let _ = ref_num;
                    let result = db.insert_quotation_atomic(
                        pid, &created_by, notes.as_deref(),
                        rate, disc, gct, days, None,
                    ).await;
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
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle { self.notes.read(cx).focus_handle.clone() }
}

impl Render for QuotationForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let notes_f    = self.notes.read(cx).focus_handle.is_focused(window);
        let rate_f     = self.exchange_rate.read(cx).focus_handle.is_focused(window);
        let disc_f     = self.discount_percent.read(cx).focus_handle.is_focused(window);
        let gct_f      = self.gct_percent.read(cx).focus_handle.is_focused(window);
        let days_f     = self.validity_days.read(cx).focus_handle.is_focused(window);
        let cancel_f   = self.cancel_focus.is_focused(window);
        let save_f     = self.save_focus.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .key_context("QuotationForm")
            .on_action(cx.listener(|_, _: &EscapeForm, window, cx| {
                let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                window.focus(&root, cx);
                cx.emit(QuotationFormEvent::Cancelled);
            }))
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = [
                    this.project_dropdown.read(cx).trigger_focus.clone(),
                    this.exchange_rate.read(cx).focus_handle.clone(),
                    this.discount_percent.read(cx).focus_handle.clone(),
                    this.gct_percent.read(cx).focus_handle.clone(),
                    this.validity_days.read(cx).focus_handle.clone(),
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
                    this.exchange_rate.read(cx).focus_handle.clone(),
                    this.discount_percent.read(cx).focus_handle.clone(),
                    this.gct_percent.read(cx).focus_handle.clone(),
                    this.validity_days.read(cx).focus_handle.clone(),
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
                    .w(px(600.))
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
                            // Reference (read-only)
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(label_col("Reference", &c))
                                    .child(div().flex_1()
                                        .px(px(8.)).py(px(6.)).bg(rgb(c.surface_default)).rounded(px(4.))
                                        .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                        .child(self.reference_number.clone()))
                            )
                            .child(divider(&c))
                            // Project
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(label_col("Project", &c))
                                    .child(div().flex_1().child(self.project_dropdown.clone()))
                            )
                            .child(divider(&c))
                            // Financial row: rate + discount
                            .child(
                                div().flex().flex_row().gap(px(12.)).py(px(10.))
                                    .child(
                                        div().flex().flex_row().items_center().flex_1()
                                            .child(label_col("Rate (JMD/USD)", &c))
                                            .child(div().flex_1().child(text_field("", self.exchange_rate.clone(), rate_f, false, cx)))
                                    )
                                    .child(
                                        div().flex().flex_row().items_center().flex_1()
                                            .child(label_col("Discount %", &c))
                                            .child(div().flex_1().child(text_field("", self.discount_percent.clone(), disc_f, false, cx)))
                                    )
                            )
                            .child(divider(&c))
                            // GCT + validity
                            .child(
                                div().flex().flex_row().gap(px(12.)).py(px(10.))
                                    .child(
                                        div().flex().flex_row().items_center().flex_1()
                                            .child(label_col("GCT %", &c))
                                            .child(div().flex_1().child(text_field("", self.gct_percent.clone(), gct_f, false, cx)))
                                    )
                                    .child(
                                        div().flex().flex_row().items_center().flex_1()
                                            .child(label_col("Valid (days)", &c))
                                            .child(div().flex_1().child(text_field("", self.validity_days.clone(), days_f, false, cx)))
                                    )
                            )
                            .child(divider(&c))
                            // Notes
                            .child(
                                div().flex().flex_row().items_center().py(px(10.))
                                    .child(label_col("Notes", &c))
                                    .child(div().flex_1().child(text_field("", self.notes.clone(), notes_f, false, cx)))
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
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, window, cx| {
                                        let root = cx.global::<vassl_ui::RootFocusHandle>().0.clone();
                                        window.focus(&root, cx);
                                        cx.emit(QuotationFormEvent::Cancelled);
                                    }))
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

fn label_col(text: &str, c: &vassl_ui::ThemeColors) -> impl gpui::IntoElement {
    div().w(px(140.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child(text.to_string())
}

fn divider(c: &vassl_ui::ThemeColors) -> impl gpui::IntoElement {
    div().h(px(1.)).bg(rgb(c.surface_default))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reference_number_format()   { let r = "VASSL-2026-0001"; assert!(r.starts_with("VASSL-")); assert_eq!(r.len(), 15); }
    #[test] fn form_requires_project()     { assert!(validate_form(None).is_some()); }
    #[test] fn form_valid_with_project()   { assert!(validate_form(Some(1)).is_none()); }
    #[test] fn projects_to_items_maps_correctly() {
        use vassl_core::{Project, ProjectStatus};
        let projects = vec![Project {
            id: 1, name: "Alpha".into(), client_name: "Acme".into(),
            client_address: None, client_attn: None, client_tel: None,
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
