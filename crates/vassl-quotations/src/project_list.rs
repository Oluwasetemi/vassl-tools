use gpui::{
    div, prelude::*, px, rems, rgb, uniform_list, App, Context, Entity, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, UniformListScrollHandle, Window,
};
use vassl_core::{Project, ProjectStatus};
use vassl_ui::{scrollbar_geometry, ScrollDragState, ThemeColors, ThemeHandle};

use crate::store::{ProjectContextMenu, QuotationStore};

const TRACK_W: f32 = 14.0;

pub struct ProjectList {
    store: Entity<QuotationStore>,
    pub scroll_handle: UniformListScrollHandle,
    drag: Option<ScrollDragState>,
}

impl ProjectList {
    pub fn new(store: Entity<QuotationStore>, _cx: &mut Context<Self>) -> Self {
        Self {
            store,
            scroll_handle: UniformListScrollHandle::default(),
            drag: None,
        }
    }
}

fn project_status_color(status: &ProjectStatus, c: &ThemeColors) -> u32 {
    match status {
        ProjectStatus::Active => c.status_green,
        ProjectStatus::Completed => c.status_amber,
        ProjectStatus::Archived => c.status_grey,
    }
}

fn project_status_label(status: &ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Active => "Active",
        ProjectStatus::Completed => "Completed",
        ProjectStatus::Archived => "Archived",
    }
}

impl Render for ProjectList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let store = self.store.read(cx);

        if store.projects.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(c.text_default))
                .child("No projects yet — click \"+ New Project\" to create one.")
                .into_any_element();
        }

        let count = store.projects.len();
        let _selected_id = store.selected_project_id;
        let store_entity = self.store.clone();

        let geom = scrollbar_geometry(&self.scroll_handle);
        let is_dragging = self.drag.is_some();

        let mut track = div()
            .id("proj-list-sb-track")
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
            let (viewport_h, thumb_h, max_scroll) = (g.viewport_h, g.thumb_h, g.max_scroll);
            track = track.child(
                div()
                    .id("proj-list-sb-thumb")
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
                            this.drag = Some(ScrollDragState {
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

        let mut root = div()
            .relative()
            .flex_1()
            .flex()
            .flex_row()
            .min_h(px(0.))
            .child(
                uniform_list(
                    "project-list",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let store = this.store.read(cx);
                        let sel = store.selected_project_id;
                        let c = cx.global::<ThemeHandle>().0.clone();
                        range
                            .map(|ix| {
                                let p = &store.projects[ix];
                                project_row(p, store_entity.clone(), sel, &c)
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.scroll_handle)
                .flex_1(),
            )
            .child(track);

        if is_dragging {
            root = root.child(
                div()
                    .id("proj-list-sb-overlay")
                    .absolute()
                    .inset_0()
                    .cursor_pointer()
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        if let Some(drag) = &this.drag {
                            let new_offset = drag.compute_offset(ev.position.y.as_f32());
                            this.scroll_handle
                                .0
                                .borrow()
                                .base_handle
                                .set_offset(gpui::point(gpui::px(0.), gpui::px(new_offset)));
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, cx| {
                            this.drag = None;
                            cx.notify();
                        }),
                    ),
            );
        }

        root.into_any_element()
    }
}

fn project_row(
    p: &Project,
    store: Entity<QuotationStore>,
    selected_id: Option<i64>,
    c: &ThemeColors,
) -> impl IntoElement {
    let id = p.id;
    let badge_col = project_status_color(&p.status, c);
    let status_label = project_status_label(&p.status).to_string();
    let date_str = p.created_at.get(..10).unwrap_or("").to_string();
    let is_selected = selected_id == Some(id);
    let row_bg = if is_selected {
        c.surface_active
    } else {
        c.canvas_bg
    };
    let hover_bg = rgb(c.surface_hover);
    let name = p.name.clone();
    let client_name = p.client_name.clone();
    let name_for_menu = name.clone();
    let store_left = store.clone();

    div()
        .id(format!("proj-row-{id}"))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .bg(rgb(row_bg))
        .when(!is_selected, |d| d.hover(move |s| s.bg(hover_bg)))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |ev: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                if ev.click_count >= 2 {
                    store_left.update(cx, |s, cx| s.select_project(id, cx));
                } else {
                    store_left.update(cx, |s, cx| {
                        s.selected_project_id = Some(id);
                        cx.notify();
                    });
                }
            },
        )
        .on_mouse_down(MouseButton::Right, {
            let store2 = store.clone();
            move |ev: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                store2.update(cx, |s, cx| {
                    s.set_project_context_menu(
                        ProjectContextMenu {
                            project_id: id,
                            project_name: name_for_menu.clone(),
                            x: ev.position.x.as_f32(),
                            y: ev.position.y.as_f32(),
                        },
                        cx,
                    )
                });
            }
        })
        // Status badge
        .child(
            div()
                .w(px(8.))
                .h(px(8.))
                .rounded_full()
                .bg(rgb(badge_col))
                .mr(px(8.)),
        )
        // Project name
        .child(
            div()
                .flex_1()
                .text_size(rems(0.923))
                .text_color(rgb(c.text_default))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(name),
        )
        // Client name
        .child(
            div()
                .w(px(160.))
                .text_size(rems(0.923))
                .text_color(rgb(c.text_muted))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(client_name),
        )
        // Status label
        .child(
            div()
                .w(px(80.))
                .text_size(rems(0.846))
                .text_color(rgb(badge_col))
                .child(status_label),
        )
        // Date
        .child(
            div()
                .w(px(90.))
                .text_size(rems(0.846))
                .text_color(rgb(c.text_muted))
                .child(date_str),
        )
}
