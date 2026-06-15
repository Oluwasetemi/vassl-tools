use crate::actions::{CheckForUpdates, InstallUpdate};
use crate::auto_update::{AutoUpdater, UpdateStatus};
use gpui::{
    div, prelude::*, px, relative, rems, rgb, Context, Entity, IntoElement, MouseButton, Render,
    SharedString, Window,
};
use vassl_ui::ThemeHandle;

// Shared palette — must match about_dialog.rs constants.
pub const BADGE_BLUE: u32 = 0x2d6fce;
pub const BADGE_AMBER: u32 = 0xd97706;
pub const BADGE_RED: u32 = 0xe05252;
pub const BADGE_GREEN: u32 = 0x2a7a3b;

pub struct StatusBar {
    pub last_action: Option<String>,
    pub updater: Option<Entity<AutoUpdater>>,
}

impl StatusBar {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            last_action: None,
            updater: None,
        }
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

        let update_status = self.updater.as_ref().map(|u| u.read(cx).status.clone());

        // ── Download progress bar ─────────────────────────────────────────────
        // When a download is in progress, render a full-width progress strip
        // above the status bar row so the user can't miss it.
        let progress_strip: Option<gpui::AnyElement> = match &update_status {
            Some(UpdateStatus::Downloading { pct }) => {
                let frac = (*pct as f32 / 100.0).clamp(0.0, 1.0);
                Some(
                    div()
                        .w_full()
                        .h(px(3.))
                        .bg(rgb(c.surface_default))
                        .overflow_hidden()
                        .child(
                            // relative(frac) = CSS width: X% — always proportional to
                            // the actual window width regardless of window size.
                            div().h_full().w(relative(frac)).bg(rgb(BADGE_BLUE)),
                        )
                        .into_any_element(),
                )
            }
            Some(UpdateStatus::Installing) => Some(
                div()
                    .w_full()
                    .h(px(3.))
                    .bg(rgb(BADGE_AMBER))
                    .into_any_element(),
            ),
            _ => None,
        };

        // ── Right-side badge ──────────────────────────────────────────────────
        enum Badge {
            Text {
                msg: SharedString,
                color: u32,
                clickable: bool,
                action_install: bool,
            },
            Download {
                pct: u8,
            },
        }

        let badge: Option<Badge> = match &update_status {
            Some(UpdateStatus::Checking) => Some(Badge::Text {
                msg: "Checking for updates…".into(),
                color: c.text_muted,
                clickable: false,
                action_install: false,
            }),
            Some(UpdateStatus::Available(info)) => Some(Badge::Text {
                msg: format!("↓  Update v{} available", info.version).into(),
                color: BADGE_BLUE,
                clickable: true,
                action_install: false,
            }),
            Some(UpdateStatus::Downloading { pct }) => Some(Badge::Download { pct: *pct }),
            Some(UpdateStatus::ReadyToInstall(_)) => Some(Badge::Text {
                msg: "↻  Restart to install update".into(),
                color: BADGE_AMBER,
                clickable: true,
                action_install: true,
            }),
            Some(UpdateStatus::Installing) => Some(Badge::Text {
                msg: "Installing update…".into(),
                color: c.text_muted,
                clickable: false,
                action_install: false,
            }),
            Some(UpdateStatus::Error(e)) => Some(Badge::Text {
                msg: format!("Update error: {e}").into(),
                color: 0xe05252,
                clickable: false,
                action_install: false,
            }),
            _ => None,
        };

        let badge_el: Option<gpui::AnyElement> = badge.map(|b| match b {
            Badge::Download { pct } => {
                // Wider pill with percentage text — makes it obvious a download is happening.
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .h(px(20.))
                    .rounded(px(4.))
                    .text_size(rems(0.769))
                    .text_color(rgb(BADGE_BLUE))
                    .border_1()
                    .border_color(rgb(BADGE_BLUE))
                    .child(SharedString::from(format!("Downloading update  {pct}%")))
                    .into_any_element()
            }
            Badge::Text {
                msg,
                color,
                clickable,
                action_install,
            } => {
                let mut el = div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .h(px(20.))
                    .rounded(px(4.))
                    .text_size(rems(0.769))
                    .text_color(rgb(color))
                    .border_1()
                    .border_color(rgb(color))
                    .child(msg);
                if clickable {
                    if action_install {
                        el = el.cursor_pointer().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_, _, _, cx| {
                                cx.dispatch_action(&InstallUpdate);
                            }),
                        );
                    } else {
                        el = el.cursor_pointer().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_, _, _, cx| {
                                cx.dispatch_action(&CheckForUpdates);
                            }),
                        );
                    }
                }
                el.into_any_element()
            }
        });

        // ── Assemble ──────────────────────────────────────────────────────────
        let mut wrapper = div().w_full().flex().flex_col();
        if let Some(strip) = progress_strip {
            wrapper = wrapper.child(strip);
        }

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
        if let Some(el) = badge_el {
            row = row.child(el);
        }

        wrapper.child(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_last_action_is_none() {
        let bar = StatusBar {
            last_action: None,
            updater: None,
        };
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
