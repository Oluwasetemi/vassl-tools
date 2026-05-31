use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*};

use crate::sidebar::Sidebar;

pub struct VasslRoot {
    sidebar: Entity<Sidebar>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar: cx.new(Sidebar::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .w_full()
            .h_full()
            .bg(gpui::rgb(0x1e1e2e))
            .child(self.sidebar.clone())
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .bg(gpui::rgb(0x1e1e2e))
                    .child("pane area — Tasks 2-4"),
            )
    }
}
