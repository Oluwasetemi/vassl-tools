use gpui::{Context, Entity, IntoElement, MouseButton, Render, SharedString, Window,
           div, prelude::*, px, rems, rgb};
use vassl_ui::ThemeHandle;
use crate::auto_update::{AutoUpdater, UpdateStatus};
use crate::actions::{CheckForUpdates, InstallUpdate};

pub struct StatusBar {
    pub last_action: Option<String>,
    pub updater:     Option<Entity<AutoUpdater>>,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { last_action: None, updater: None }
    }

    pub fn set_updater(&mut self, updater: Entity<AutoUpdater>) {
        self.updater = Some(updater);
    }

    #[allow(dead_code)]
    pub fn set_last_action(&mut self, action: impl Into<String>, cx: &mut Context<Self>) {
        self.last_action = Some(action.into());
        cx.notify();
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let label: SharedString = self.last_action.as_deref().unwrap_or("Ready").into();

        // Determine update badge state
        let update_badge = self.updater.as_ref().map(|u| {
            let status = u.read(cx);
            match &status.status {
                UpdateStatus::Checking => Some(("Checking for updates…", false, false)),
                UpdateStatus::Available(info) => {
                    let msg = format!("Update available — v{}", info.version);
                    Some((msg.leak() as &'static str, true, false))
                }
                UpdateStatus::Downloading { pct, .. } => {
                    let msg = format!("Downloading update… {}%", pct);
                    Some((msg.leak() as &'static str, false, false))
                }
                UpdateStatus::ReadyToInstall { .. } => {
                    Some(("Restart to install update", true, true))
                }
                UpdateStatus::Installing => Some(("Installing update…", false, false)),
                UpdateStatus::Error(e)   => {
                    let msg = format!("Update error: {e}");
                    Some((msg.leak() as &'static str, false, false))
                }
                _ => None,
            }
        }).flatten();

        let mut row = div()
            .w_full()
            .h(px(24.))
            .bg(rgb(c.sidebar_bg))
            .border_t_1()
            .border_color(rgb(c.surface_default))
            .px(px(12.))
            .flex()
            .items_center()
            .justify_between()
            .text_color(rgb(c.text_muted))
            .text_size(rems(0.846));

        row = row.child(div().child(label));

        if let Some((msg, clickable, is_install)) = update_badge {
            let badge_color = if clickable { 0x2d6fce_u32 } else { c.text_muted };
            let mut badge = div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .h(px(18.))
                .rounded(px(4.))
                .text_size(rems(0.77))
                .text_color(rgb(badge_color))
                .border_1()
                .border_color(rgb(badge_color))
                .child(SharedString::from(msg));

            if clickable {
                if is_install {
                    badge = badge
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| {
                            cx.dispatch_action(&InstallUpdate);
                        }));
                } else {
                    badge = badge
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| {
                            cx.dispatch_action(&CheckForUpdates);
                        }));
                }
            }

            row = row.child(badge);
        }

        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_last_action_is_none() {
        let bar = StatusBar { last_action: None, updater: None };
        assert!(bar.last_action.is_none());
    }

    #[test]
    fn last_action_field_holds_string_value() {
        let bar = StatusBar {
            last_action: Some("Stock entry added".into()),
            updater: None,
        };
        assert_eq!(bar.last_action, Some("Stock entry added".to_string()));
    }
}
