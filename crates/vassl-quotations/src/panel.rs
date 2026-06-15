use gpui::{
    div, prelude::*, px, rems, rgb, Context, Entity, Focusable, IntoElement, MouseButton,
    MouseDownEvent, Render, Subscription, Window,
};
use vassl_core::QuotationStatus;
use vassl_pricebook::store::PriceBookStoreHandle;
use vassl_ui::{tooltip, tooltip_keyed, AppSettings, NewRecord, ThemeHandle};

use crate::line_item_form::{LineItemForm, LineItemFormEvent};
use crate::project_form::{ProjectForm, ProjectFormEvent};
use crate::project_list::ProjectList;
use crate::quotation_detail::QuotationDetail;
use crate::quotation_form::{QuotationForm, QuotationFormEvent};
use crate::quotation_list::QuotationList;
use crate::store::QuotationStore;
use crate::QuotationStoreHandle;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Quotations,
    Items,
    Projects,
}

pub struct QuotationPanel {
    store: Entity<QuotationStore>,
    quot_list: Entity<QuotationList>,
    quot_detail: Entity<QuotationDetail>,
    project_list: Entity<ProjectList>,
    active_tab: Tab,
    form: Option<Entity<QuotationForm>>,
    _form_sub: Option<Subscription>,
    edit_form: Option<Entity<QuotationForm>>,
    _edit_form_sub: Option<Subscription>,
    project_form: Option<Entity<ProjectForm>>,
    _project_form_sub: Option<Subscription>,
    edit_project_form: Option<Entity<ProjectForm>>,
    _edit_project_form_sub: Option<Subscription>,
    line_item_form: Option<Entity<LineItemForm>>,
    _line_item_form_sub: Option<Subscription>,
    detail_open: bool,
    project_detail_open: bool,
    last_proj_detail_gen: u32,
}

impl QuotationPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<QuotationStoreHandle>().0.clone();
        let quot_list = cx.new(|cx| QuotationList::new(store.clone(), cx));
        let quot_detail = cx.new(|cx| QuotationDetail::new(store.clone(), cx));
        let project_list = cx.new(|cx| ProjectList::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_quotations(cx));

        cx.observe(&store, |this, _, cx| {
            if this.store.read(cx).detail_requested {
                this.detail_open = true;
                this.store.update(cx, |s, _| s.detail_requested = false);
                cx.notify();
            }
            let proj_gen = this.store.read(cx).project_detail_generation;
            if proj_gen > this.last_proj_detail_gen {
                this.last_proj_detail_gen = proj_gen;
                this.project_detail_open = true;
                this.active_tab = Tab::Projects;
                cx.notify();
            }
        })
        .detach();

        Self {
            store,
            quot_list,
            quot_detail,
            project_list,
            active_tab: Tab::Quotations,
            form: None,
            _form_sub: None,
            edit_form: None,
            _edit_form_sub: None,
            project_form: None,
            _project_form_sub: None,
            edit_project_form: None,
            _edit_project_form_sub: None,
            line_item_form: None,
            _line_item_form_sub: None,
            detail_open: false,
            project_detail_open: false,
            last_proj_detail_gen: 0,
        }
    }

    pub fn show_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_open = true;
        cx.notify();
    }
    pub fn hide_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_open = false;
        cx.notify();
    }
    pub fn show_project_detail(&mut self, cx: &mut Context<Self>) {
        self.project_detail_open = true;
        cx.notify();
    }
    pub fn hide_project_detail(&mut self, cx: &mut Context<Self>) {
        self.project_detail_open = false;
        cx.notify();
    }

    fn open_edit_project_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_project_form.is_some() {
            return;
        }
        // find the project from context menu
        let project = {
            let store = self.store.read(cx);
            let Some(ctx) = store.context_menu_project.as_ref() else {
                return;
            };
            match store
                .projects
                .iter()
                .find(|p| p.id == ctx.project_id)
                .cloned()
            {
                Some(p) => p,
                None => return,
            }
        };
        let form = cx.new(|cx| ProjectForm::edit(self.store.clone(), &project, cx));
        let fh = form.read(cx).focus_handle(cx);
        let sub = cx.subscribe(&form, |this, _form, ev: &ProjectFormEvent, cx| match ev {
            ProjectFormEvent::Submitted | ProjectFormEvent::Cancelled => {
                this._edit_project_form_sub = None;
                this.edit_project_form = None;
                cx.notify();
            }
        });
        self.edit_project_form = Some(form);
        self._edit_project_form_sub = Some(sub);
        cx.notify();
        window.defer(cx, move |window, cx| {
            window.focus(&fh, cx);
        });
    }

    fn open_edit_quotation_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_form.is_some() {
            return;
        }
        let (qid, ref_num, project_id, notes, extras) = {
            let store = self.store.read(cx);
            let Some(sid) = store.selected_id else {
                return;
            };
            let Some(q) = store.quotations.iter().find(|q| q.id == sid).cloned() else {
                return;
            };
            let extras = store.selected_extras.clone().unwrap_or_default();
            (sid, q.reference_number, q.project_id, q.notes, extras)
        };
        let form = cx.new(|cx| {
            QuotationForm::edit_for(
                self.store.clone(),
                qid,
                ref_num,
                &extras,
                notes,
                Some(project_id),
                cx,
            )
        });
        let fh = form.read(cx).focus_handle(cx);
        let sub = cx.subscribe(&form, |this, _form, ev: &QuotationFormEvent, cx| match ev {
            QuotationFormEvent::Submitted | QuotationFormEvent::Cancelled => {
                this._edit_form_sub = None;
                this.edit_form = None;
                cx.notify();
            }
        });
        self.edit_form = Some(form);
        self._edit_form_sub = Some(sub);
        cx.notify();
        window.defer(cx, move |window, cx| {
            window.focus(&fh, cx);
        });
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_next(cx)) {
            self.quot_list.update(cx, |l, _| {
                l.scroll_handle
                    .scroll_to_item(idx, gpui::ScrollStrategy::Top)
            });
        }
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_prev(cx)) {
            self.quot_list.update(cx, |l, _| {
                l.scroll_handle
                    .scroll_to_item(idx, gpui::ScrollStrategy::Top)
            });
        }
    }

    fn open_project_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project_form.is_some() {
            return;
        }
        let form = cx.new(|cx| ProjectForm::new(self.store.clone(), cx));
        let fh = form.read(cx).focus_handle(cx);
        let sub = cx.subscribe(&form, |this, _form, ev: &ProjectFormEvent, cx| match ev {
            ProjectFormEvent::Submitted | ProjectFormEvent::Cancelled => {
                this._project_form_sub = None;
                this.project_form = None;
                cx.notify();
            }
        });
        self.project_form = Some(form);
        self._project_form_sub = Some(sub);
        cx.notify();
        window.defer(cx, move |window, cx| {
            window.focus(&fh, cx);
        });
    }

    pub fn create_form(&mut self, cx: &mut Context<Self>) -> Option<gpui::FocusHandle> {
        if self.form.is_some() {
            return None;
        }
        let ref_num = {
            let db = crate::db::QuotationDb::global(&**cx);
            db.next_reference_number()
                .unwrap_or_else(|_| "VASSL-ERR-0000".to_string())
        };
        let form = cx.new(|cx| QuotationForm::new(self.store.clone(), ref_num, cx));
        let fh = form.read(cx).focus_handle(cx);
        let sub = cx.subscribe(&form, |this, _form, ev: &QuotationFormEvent, cx| match ev {
            QuotationFormEvent::Submitted | QuotationFormEvent::Cancelled => {
                this._form_sub = None;
                this.form = None;
                cx.notify();
            }
        });
        self.form = Some(form);
        self._form_sub = Some(sub);
        cx.notify();
        Some(fh)
    }

    pub fn open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fh) = self.create_form(cx) {
            window.defer(cx, move |window, cx| {
                window.focus(&fh, cx);
            });
        }
    }

    fn open_item_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.line_item_form.is_some() {
            return;
        }
        let quotation_id = match self.store.read(cx).selected_id {
            Some(id) => id,
            None => return,
        };
        let products: Vec<_> = cx
            .global::<PriceBookStoreHandle>()
            .0
            .read(cx)
            .product_prices
            .iter()
            .filter(|p| p.latest.is_some())
            .cloned()
            .collect();
        let form = cx.new(|cx| LineItemForm::new(self.store.clone(), quotation_id, products, cx));
        let fh = form.read(cx).focus_handle(cx);
        let sub = cx.subscribe(&form, |this, _, ev: &LineItemFormEvent, cx| match ev {
            LineItemFormEvent::Submitted => {
                let qid = this.store.read(cx).selected_id;
                if let Some(id) = qid {
                    let _ = this.store.update(cx, |s, cx| s.load_line_items(id, cx));
                }
                this._line_item_form_sub = None;
                this.line_item_form = None;
                cx.notify();
            }
            LineItemFormEvent::Cancelled => {
                this._line_item_form_sub = None;
                this.line_item_form = None;
                cx.notify();
            }
        });
        self.line_item_form = Some(form);
        self._line_item_form_sub = Some(sub);
        cx.notify();
        window.defer(cx, move |window, cx| {
            window.focus(&fh, cx);
        });
    }
}

impl Render for QuotationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let active_tab = self.active_tab;
        let has_selection = self.store.read(cx).selected_id.is_some();
        let allow_delete = cx.global::<AppSettings>().allow_delete;

        #[cfg(target_os = "macos")]
        let mod_key = "⌘";
        #[cfg(not(target_os = "macos"))]
        let mod_key = "Ctrl+";

        let detail_open = self.detail_open;
        let project_detail_open = self.project_detail_open;
        let list_content = div().flex_1().h_full().flex().flex_col();
        let list_content = match active_tab {
            Tab::Quotations => list_content.child(self.quot_list.clone()),
            Tab::Items => list_content.child(self.quot_detail.clone()),
            Tab::Projects => list_content.child(self.project_list.clone()),
        };

        // Build optional quotation detail sidebar
        let selected_quotation = if detail_open && active_tab == Tab::Quotations {
            let store = self.store.read(cx);
            store
                .selected_id
                .and_then(|sid| store.quotations.iter().find(|q| q.id == sid).cloned())
        } else {
            None
        };

        // Build optional project detail sidebar (Projects tab)
        let selected_project = if project_detail_open && active_tab == Tab::Projects {
            let store = self.store.read(cx);
            store
                .selected_project_id
                .and_then(|id| store.projects.iter().find(|p| p.id == id).cloned())
        } else {
            None
        };

        let content = if detail_open && active_tab == Tab::Quotations {
            let detail_sidebar = {
                let mut sidebar = div()
                    .w(px(300.))
                    .flex_shrink_0()
                    .border_l_1()
                    .border_color(rgb(c.surface_default))
                    .flex()
                    .flex_col()
                    .bg(rgb(c.canvas_bg))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(12.))
                            .py(px(10.))
                            .bg(rgb(c.sidebar_bg))
                            .border_b_1()
                            .border_color(rgb(c.surface_default))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(rems(0.923))
                                    .text_color(rgb(c.text_default))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("Quotation Details"),
                            )
                            .child(
                                div()
                                    .id("quot-detail-close")
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(c.surface_hover)))
                                    .text_size(rems(0.923))
                                    .text_color(rgb(c.text_muted))
                                    .child("×")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                            this.hide_detail(cx);
                                        }),
                                    ),
                            ),
                    );

                if let Some(q) = selected_quotation {
                    let status_label = format!("{:?}", q.status);
                    let can_edit = matches!(q.status, QuotationStatus::Draft);
                    sidebar = sidebar
                        .child(
                            div()
                                .id("quot-detail-scroll")
                                .flex_1()
                                .min_h(px(0.))
                                .overflow_y_scroll()
                                .pb(px(64.))
                                .flex()
                                .flex_col()
                                .child(quot_detail_field(
                                    "Reference",
                                    q.reference_number.clone(),
                                    &c,
                                ))
                                .child(quot_detail_field("Status", status_label, &c))
                                .child(quot_detail_field("Project", q.project_name.clone(), &c))
                                .child(quot_detail_field("Client", q.client_name.clone(), &c))
                                .child(quot_detail_field(
                                    "Total (USD)",
                                    format!("${:.2}", q.total_usd),
                                    &c,
                                ))
                                .child(quot_detail_field(
                                    "Created",
                                    q.created_at.get(..10).unwrap_or("").to_string(),
                                    &c,
                                ))
                                .child(quot_detail_field(
                                    "Notes",
                                    q.notes.clone().unwrap_or_else(|| "—".into()),
                                    &c,
                                )),
                        )
                        .child(
                            div()
                                .px(px(12.))
                                .py(px(10.))
                                .border_t_1()
                                .border_color(rgb(c.surface_default))
                                .flex()
                                .justify_end()
                                .child({
                                    let mut btn = div()
                                        .id("quot-detail-edit-btn")
                                        .px(px(12.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .text_size(rems(0.923))
                                        .text_color(rgb(c.text_default))
                                        .bg(rgb(if can_edit {
                                            c.surface_active
                                        } else {
                                            c.surface_default
                                        }))
                                        .child("Edit Quotation");
                                    if can_edit {
                                        btn = btn.cursor_pointer().on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _: &MouseDownEvent, window, cx| {
                                                this.open_edit_quotation_form(window, cx);
                                            }),
                                        );
                                    }
                                    btn
                                }),
                        );
                } else {
                    sidebar = sidebar.child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(c.text_muted))
                            .text_size(rems(0.923))
                            .child("Select a quotation to view details"),
                    );
                }
                sidebar
            };

            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .child(list_content)
                .child(detail_sidebar)
        } else if project_detail_open && active_tab == Tab::Projects {
            let proj_sidebar = {
                let mut sidebar = div()
                    .w(px(300.))
                    .flex_shrink_0()
                    .border_l_1()
                    .border_color(rgb(c.surface_default))
                    .flex()
                    .flex_col()
                    .bg(rgb(c.canvas_bg))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(12.))
                            .py(px(10.))
                            .bg(rgb(c.sidebar_bg))
                            .border_b_1()
                            .border_color(rgb(c.surface_default))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(rems(0.923))
                                    .text_color(rgb(c.text_default))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("Project Details"),
                            )
                            .child(
                                div()
                                    .id("quot-proj-detail-close")
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(c.surface_hover)))
                                    .text_size(rems(0.923))
                                    .text_color(rgb(c.text_muted))
                                    .child("×")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                            this.project_detail_open = false;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    );

                if let Some(p) = selected_project {
                    let status_label = format!("{:?}", p.status);
                    sidebar = sidebar.child(
                        div()
                            .id("quot-proj-detail-scroll")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .pb(px(64.))
                            .flex()
                            .flex_col()
                            .child(quot_detail_field("Name", p.name.clone(), &c))
                            .child(quot_detail_field("Client", p.client_name.clone(), &c))
                            .child(quot_detail_field(
                                "Address",
                                p.client_address.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Attn",
                                p.client_attn.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Tel",
                                p.client_tel.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Client Contact",
                                p.client_contact.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "VASSL Contact",
                                p.vassl_contact.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field("Status", status_label, &c))
                            .child(quot_detail_field(
                                "Date Started",
                                p.date_started.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Date Completed",
                                p.date_completed.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Signed-off",
                                p.signedoff_date.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Technicians",
                                p.technicians.clone().unwrap_or_else(|| "—".into()),
                                &c,
                            ))
                            .child(quot_detail_field(
                                "Created",
                                p.created_at.get(..10).unwrap_or("").to_string(),
                                &c,
                            )),
                    );
                } else {
                    sidebar = sidebar.child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(c.text_muted))
                            .text_size(rems(0.923))
                            .child("Select a project to view details"),
                    );
                }
                sidebar
            };

            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .child(list_content)
                .child(proj_sidebar)
        } else {
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .child(list_content)
        };

        let mut root = div()
            .key_context("QuotationPanel")
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| {
                this.open_form(window, cx);
            }))
            .relative()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(8.))
                    .bg(rgb(c.canvas_bg))
                    .child({
                        let is_tab = active_tab == Tab::Quotations;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("quot-tab-quotations")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(if is_tab {
                                c.surface_active
                            } else {
                                c.surface_default
                            }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = Tab::Quotations;
                                    cx.notify();
                                }),
                            )
                            .child("Quotations")
                    })
                    .child({
                        let is_tab = active_tab == Tab::Items;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("quot-tab-items")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(if is_tab {
                                c.surface_active
                            } else {
                                c.surface_default
                            }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = Tab::Items;
                                    cx.notify();
                                }),
                            )
                            .child("Items")
                    })
                    .child({
                        let is_tab = active_tab == Tab::Projects;
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("quot-tab-projects")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(if is_tab {
                                c.surface_active
                            } else {
                                c.surface_default
                            }))
                            .when(!is_tab, |d| d.hover(move |s| s.bg(hover_bg)))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = Tab::Projects;
                                    cx.notify();
                                }),
                            )
                            .child("Projects")
                    })
                    .child(div().flex_1())
                    .child({
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("quot-btn-new-project")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .hover(move |s| s.bg(hover_bg))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_muted))
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.open_project_form(window, cx);
                                }),
                            )
                            .tooltip(tooltip("New Project"))
                            .child("+ New Project")
                    })
                    // New Quotation — hidden on Projects tab
                    .when(active_tab != Tab::Projects, |d| {
                        d.child(
                            div()
                                .id("quot-btn-new")
                                .px(px(12.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .bg(rgb(c.surface_active))
                                .text_size(rems(0.923))
                                .text_color(rgb(c.text_default))
                                .cursor_pointer()
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.open_form(window, cx);
                                    }),
                                )
                                .tooltip(tooltip_keyed("New Quotation", format!("{mod_key}N")))
                                .child("+ New Quotation"),
                        )
                    })
                    // Add Item — only visible on Items tab when a quotation is selected
                    .when(active_tab == Tab::Items, |d| {
                        let mut btn = div()
                            .id("quot-btn-add-item")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(if has_selection {
                                c.surface_active
                            } else {
                                c.surface_default
                            }))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .child("+ Add Item");
                        if has_selection {
                            btn = btn.cursor_pointer().on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.open_item_form(window, cx);
                                }),
                            );
                        }
                        d.child(btn)
                    }),
            )
            .child(content);

        if let Some(form) = &self.form {
            root = root.child(form.clone());
        }
        if let Some(ef) = &self.edit_form {
            root = root.child(ef.clone());
        }
        if let Some(pf) = &self.project_form {
            root = root.child(pf.clone());
        }
        if let Some(epf) = &self.edit_project_form {
            root = root.child(epf.clone());
        }
        if let Some(lif) = &self.line_item_form {
            root = root.child(lif.clone());
        }

        // Project context menu overlay
        let proj_ctx = self.store.read(cx).context_menu_project.clone();
        if let Some(target) = proj_ctx {
            let viewport = window.viewport_size();
            const MENU_W: f32 = 200.0;
            let menu_h: f32 = if allow_delete { 128.0 } else { 84.0 };
            let menu_x = target.x.min((viewport.width.as_f32() - MENU_W).max(0.0));
            let menu_y = target.y.min((viewport.height.as_f32() - menu_h).max(0.0));
            let pid = target.project_id;

            root = root
                .child(div().absolute().inset_0().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _: &mut Window, cx| {
                        this.store
                            .update(cx, |s, cx| s.clear_project_context_menu(cx));
                    }),
                ))
                .child(
                    div()
                        .absolute()
                        .left(px(menu_x))
                        .top(px(menu_y))
                        .w(px(MENU_W))
                        .bg(rgb(c.surface_default))
                        .rounded(px(6.))
                        .shadow_md()
                        .child(
                            div()
                                .px(px(12.))
                                .pt(px(10.))
                                .pb(px(4.))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(target.project_name.clone()),
                        )
                        .child({
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-proj-view")
                                .px(px(12.))
                                .py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("View Details")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                        this.store.update(cx, |s, cx| {
                                            s.select_project(pid, cx);
                                            s.clear_project_context_menu(cx);
                                        });
                                        this.active_tab = Tab::Projects;
                                        this.project_detail_open = true;
                                        cx.notify();
                                    }),
                                )
                        })
                        .child({
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-proj-edit")
                                .px(px(12.))
                                .py(px(8.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .text_size(rems(1.))
                                .text_color(rgb(c.text_default))
                                .child("Edit Project")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                        this.open_edit_project_form(window, cx);
                                        this.store
                                            .update(cx, |s, cx| s.clear_project_context_menu(cx));
                                    }),
                                )
                        })
                        .when(allow_delete, |menu| {
                            let hover_bg = rgb(c.surface_hover);
                            menu.child(div().h(px(1.)).bg(rgb(c.surface_hover))).child(
                                div()
                                    .id("ctx-proj-delete")
                                    .px(px(12.))
                                    .py(px(8.))
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(hover_bg))
                                    .text_size(rems(1.))
                                    .text_color(rgb(c.status_red))
                                    .child("Delete Project")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |this, _: &MouseDownEvent, _: &mut Window, cx| {
                                                this.store.update(cx, |s, cx| {
                                                    s.clear_project_context_menu(cx);
                                                    s.delete_project(pid, cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                        }),
                );
        }

        root
    }
}

fn quot_detail_field(
    label: impl Into<String>,
    value: impl Into<String>,
    c: &vassl_ui::ThemeColors,
) -> impl gpui::IntoElement {
    div()
        .flex()
        .flex_col()
        .px(px(12.))
        .py(px(8.))
        .border_b_1()
        .border_color(rgb(c.surface_default))
        .child(
            div()
                .text_size(rems(0.769))
                .text_color(rgb(c.text_muted))
                .mb(px(2.))
                .child(label.into()),
        )
        .child(
            div()
                .text_size(rems(0.923))
                .text_color(rgb(c.text_default))
                .child(value.into()),
        )
}
