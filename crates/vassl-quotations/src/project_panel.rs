use gpui::{
    div, prelude::*, px, rems, rgb, uniform_list, Context, Entity, Focusable, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, Subscription,
    UniformListScrollHandle, Window,
};
use vassl_ui::{scrollbar_geometry, tooltip_keyed, AppSettings, NewRecord, ScrollDragState, ThemeHandle};

const TRACK_W: f32 = 14.0;

use crate::project_form::{ProjectForm, ProjectFormEvent};
use crate::project_list::ProjectList;
use crate::store::QuotationStore;
use crate::QuotationStoreHandle;

pub struct ProjectPanel {
    store: Entity<QuotationStore>,
    list: Entity<ProjectList>,
    project_form: Option<Entity<ProjectForm>>,
    _project_form_sub: Option<Subscription>,
    edit_project_form: Option<Entity<ProjectForm>>,
    _edit_project_form_sub: Option<Subscription>,
    detail_open: bool,
    last_detail_gen: u32,
    detail_scroll: UniformListScrollHandle,
    detail_drag: Option<ScrollDragState>,
}

impl ProjectPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = cx.global::<QuotationStoreHandle>().0.clone();
        let list = cx.new(|cx| ProjectList::new(store.clone(), cx));
        store.update(cx, |s, cx| s.load_quotations(cx));

        cx.observe(&store, |this, _, cx| {
            let gen = this.store.read(cx).project_detail_generation;
            if gen > this.last_detail_gen {
                this.last_detail_gen = gen;
                this.detail_open = true;
                cx.notify();
            }
        })
        .detach();

        Self {
            store,
            list,
            project_form: None,
            _project_form_sub: None,
            edit_project_form: None,
            _edit_project_form_sub: None,
            detail_open: false,
            last_detail_gen: 0,
            detail_scroll: UniformListScrollHandle::default(),
            detail_drag: None,
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

    pub fn select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_project_next(cx)) {
            self.list.update(cx, |l, _| {
                l.scroll_handle
                    .scroll_to_item(idx, gpui::ScrollStrategy::Top)
            });
        }
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(idx) = self.store.update(cx, |s, cx| s.select_project_prev(cx)) {
            self.list.update(cx, |l, _| {
                l.scroll_handle
                    .scroll_to_item(idx, gpui::ScrollStrategy::Top)
            });
        }
    }

    fn open_new_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    fn open_edit_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_project_form.is_some() {
            return;
        }
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
}

impl Render for ProjectPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let allow_delete = cx.global::<AppSettings>().allow_delete;

        #[cfg(target_os = "macos")]
        let mod_key = "⌘";
        #[cfg(not(target_os = "macos"))]
        let mod_key = "Ctrl+";

        let detail_open = self.detail_open;
        let selected_project = if detail_open {
            let store = self.store.read(cx);
            store
                .selected_project_id
                .and_then(|id| store.projects.iter().find(|p| p.id == id).cloned())
        } else {
            None
        };

        let list_area = if detail_open {
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
                                    .child("Project Details"),
                            )
                            .child(
                                div()
                                    .id("proj-panel-detail-close")
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

                if let Some(p) = selected_project {
                    let status_label = format!("{:?}", p.status);
                    let proj_fields: Vec<(String, String)> = vec![
                        ("Name".into(), p.name.clone()),
                        ("Client".into(), p.client_name.clone()),
                        (
                            "Address".into(),
                            p.client_address.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Attn".into(),
                            p.client_attn.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Tel".into(),
                            p.client_tel.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Client Contact".into(),
                            p.client_contact.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "VASSL Contact".into(),
                            p.vassl_contact.clone().unwrap_or_else(|| "—".into()),
                        ),
                        ("Status".into(), status_label),
                        (
                            "Date Started".into(),
                            p.date_started.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Date Completed".into(),
                            p.date_completed.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Signed-off".into(),
                            p.signedoff_date.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Technicians".into(),
                            p.technicians.clone().unwrap_or_else(|| "—".into()),
                        ),
                        (
                            "Created".into(),
                            p.created_at.get(..10).unwrap_or("").to_string(),
                        ),
                    ];
                    let proj_count = proj_fields.len();
                    let geom = scrollbar_geometry(&self.detail_scroll);
                    let is_dragging = self.detail_drag.is_some();

                    let mut track = div()
                        .id("proj-panel-sb-track")
                        .flex_shrink_0()
                        .w(px(TRACK_W))
                        .h_full()
                        .relative()
                        .bg(rgb(c.surface_default));
                    if let Some(g) = &geom {
                        let thumb_color = if is_dragging {
                            rgb(c.text_default)
                        } else {
                            rgb(c.text_muted)
                        };
                        let (viewport_h, thumb_h, max_scroll) =
                            (g.viewport_h, g.thumb_h, g.max_scroll);
                        track = track.child(
                            div()
                                .id("proj-panel-sb-thumb")
                                .absolute()
                                .top(px(g.thumb_top))
                                .left(px(2.))
                                .w(px(TRACK_W - 4.))
                                .h(px(thumb_h))
                                .rounded(px(6.))
                                .bg(thumb_color)
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                        this.detail_drag = Some(ScrollDragState {
                                            drag_offset: ev.position.y.as_f32(),
                                            thumb_h,
                                            viewport_h,
                                            max_scroll,
                                        });
                                        cx.notify();
                                    }),
                                ),
                        );
                    }

                    let mut scroll_area = div()
                        .relative()
                        .flex_1()
                        .min_h(px(0.))
                        .flex()
                        .flex_row()
                        .child(
                            uniform_list(
                                "proj-panel-detail-scroll",
                                proj_count,
                                cx.processor(
                                    move |_this, range: std::ops::Range<usize>, _window, cx| {
                                        let c = cx.global::<ThemeHandle>().0.clone();
                                        range
                                            .map(|ix| {
                                                let (label, value) = &proj_fields[ix];
                                                proj_detail_field(label, value, &c)
                                                    .into_any_element()
                                            })
                                            .collect()
                                    },
                                ),
                            )
                            .track_scroll(&self.detail_scroll)
                            .flex_1(),
                        )
                        .child(track);

                    if is_dragging {
                        scroll_area = scroll_area.child(
                            div()
                                .id("proj-panel-sb-overlay")
                                .absolute()
                                .inset_0()
                                .cursor_pointer()
                                .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                                    if let Some(drag) = &this.detail_drag {
                                        let new_offset =
                                            drag.compute_offset(ev.position.y.as_f32());
                                        this.detail_scroll.0.borrow().base_handle.set_offset(
                                            gpui::point(gpui::px(0.), gpui::px(new_offset)),
                                        );
                                        cx.notify();
                                    }
                                }))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|this, _: &MouseUpEvent, _, cx| {
                                        this.detail_drag = None;
                                        cx.notify();
                                    }),
                                ),
                        );
                    }

                    sidebar = sidebar.child(scroll_area);
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
                .child(self.list.clone())
                .child(detail_sidebar)
        } else {
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .child(self.list.clone())
        };

        let mut root = div()
            .key_context("ProjectPanel")
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| {
                this.open_new_form(window, cx);
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
                    .child(div().flex_1())
                    .child({
                        let hover_bg = rgb(c.surface_hover);
                        div()
                            .id("proj-panel-btn-new")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(rgb(c.surface_default))
                            .hover(move |s| s.bg(hover_bg))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.open_new_form(window, cx);
                                }),
                            )
                            .tooltip(tooltip_keyed("New Project", format!("{mod_key}N")))
                            .child("+ New Project")
                    }),
            )
            .child(list_area);

        if let Some(pf) = &self.project_form {
            root = root.child(pf.clone());
        }
        if let Some(epf) = &self.edit_project_form {
            root = root.child(epf.clone());
        }

        // Context menu overlay
        let proj_ctx = self.store.read(cx).context_menu_project.clone();
        if let Some(target) = proj_ctx {
            let viewport = window.viewport_size();
            const MENU_W: f32 = 200.0;
            let menu_h: f32 = if allow_delete { 96.0 } else { 52.0 };
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
                                .id("ctx-proj-panel-view")
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
                                        this.show_detail(cx);
                                    }),
                                )
                        })
                        .child({
                            let hover_bg = rgb(c.surface_hover);
                            div()
                                .id("ctx-proj-panel-edit")
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
                                        this.open_edit_form(window, cx);
                                        this.store
                                            .update(cx, |s, cx| s.clear_project_context_menu(cx));
                                    }),
                                )
                        })
                        .when(allow_delete, |menu| {
                            let hover_bg = rgb(c.surface_hover);
                            menu.child(div().h(px(1.)).bg(rgb(c.surface_hover))).child(
                                div()
                                    .id("ctx-proj-panel-delete")
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

fn proj_detail_field(
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
