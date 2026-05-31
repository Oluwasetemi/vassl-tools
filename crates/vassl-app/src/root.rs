use gpui::{Context, IntoElement, Render, Window, div, prelude::*};

pub struct VasslRoot;

impl VasslRoot {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
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
            .child("placeholder — sidebar + pane + status bar in Tasks 6-7")
    }
}
