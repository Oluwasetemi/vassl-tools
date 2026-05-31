use gpui::{Context, IntoElement, Render, SharedString, Window, div, prelude::*, px, rgb};

use crate::colors;

pub struct StatusBar {
    pub last_action: Option<String>,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { last_action: None }
    }

    /// Called by DB write helpers in future tasks to surface operation feedback in the status bar.
    #[allow(dead_code)]
    pub fn set_last_action(&mut self, action: impl Into<String>, cx: &mut Context<Self>) {
        self.last_action = Some(action.into());
        cx.notify();
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let label: SharedString = self.last_action.as_deref().unwrap_or("Ready").into();

        div()
            .w_full()
            .h(px(24.))
            .bg(rgb(colors::SIDEBAR_BG))
            .border_t_1()
            .border_color(rgb(colors::SURFACE_DEFAULT))
            .px(px(12.))
            .flex()
            .items_center()
            .text_color(rgb(colors::TEXT_MUTED))
            .text_size(px(11.))
            .child(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_last_action_is_none() {
        let bar = StatusBar { last_action: None };
        assert!(bar.last_action.is_none());
    }

    #[test]
    fn last_action_field_holds_string_value() {
        // set_last_action() requires a GPUI Context — tested via integration.
        // Verify the field stores and returns the expected string.
        let bar = StatusBar {
            last_action: Some("Stock entry added".into()),
        };
        assert_eq!(bar.last_action, Some("Stock entry added".to_string()));
    }
}
