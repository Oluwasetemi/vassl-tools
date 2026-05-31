use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, rgb};

use crate::colors;
use crate::sidebar::Sidebar;
use crate::status_bar::StatusBar;

pub struct VasslRoot {
    sidebar:    Entity<Sidebar>,
    status_bar: Entity<StatusBar>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar:    cx.new(Sidebar::new),
            status_bar: cx.new(StatusBar::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(colors::CANVAS_BG))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .child("pane area — Tasks 2-4"),
                    ),
            )
            .child(self.status_bar.clone())
    }
}
