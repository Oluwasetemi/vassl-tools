use crate::actions::{CheckForUpdates, InstallUpdate};
use crate::auto_update::{AutoUpdater, UpdateStatus};
use gpui::{
    div, img, prelude::*, px, relative, rems, rgb, rgba, Context, Entity, EventEmitter,
    IntoElement, Render, SharedString, Subscription, Window,
};
use release_channel::RELEASE_CHANNEL;
use vassl_ui::ThemeHandle;
// Re-use the same color palette as the status bar to stay visually consistent.
use crate::status_bar::{BADGE_AMBER, BADGE_BLUE, BADGE_GREEN, BADGE_RED};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT: &str = env!("VASSL_GIT_COMMIT");

/// What the action button in the update panel should do.
#[derive(Clone, Copy, PartialEq)]
enum UpdateAction {
    None,
    Check,
    Download,
    Restart,
}

pub enum AboutEvent {
    Dismissed,
    Copied,
}

impl EventEmitter<AboutEvent> for AboutDialog {}

pub struct AboutDialog {
    updater: Entity<AutoUpdater>,
    _updater_sub: Subscription,
}

impl AboutDialog {
    pub fn new(updater: Entity<AutoUpdater>, cx: &mut Context<Self>) -> Self {
        // Re-render whenever the updater status changes.
        let _updater_sub = cx.observe(&updater, |_, _entity, cx| cx.notify());
        Self {
            updater,
            _updater_sub,
        }
    }

    pub fn full_version_static() -> String {
        let channel = RELEASE_CHANNEL.dev_name();
        format!("{VERSION}+{channel}.{COMMIT}")
    }
}

impl Render for AboutDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let version = VERSION;
        let commit = COMMIT;
        let full_ver = Self::full_version_static();
        let channel = RELEASE_CHANNEL.display_name();

        // ── Update status row ─────────────────────────────────────────────
        let update_status = self.updater.read(cx).status.clone();

        let (status_text, status_color, btn_label, btn_color, btn_action): (
            SharedString,
            u32,
            Option<&'static str>,
            u32,
            UpdateAction,
        ) = match &update_status {
            UpdateStatus::Idle => (
                "Up to date".into(),
                BADGE_GREEN,
                Some("Check for updates"),
                BADGE_BLUE,
                UpdateAction::Check,
            ),
            UpdateStatus::UpToDate => (
                "Already up to date".into(),
                BADGE_GREEN,
                Some("Check again"),
                BADGE_BLUE,
                UpdateAction::Check,
            ),
            UpdateStatus::Checking => (
                "Checking for updates…".into(),
                c.text_muted,
                Some("Checking…"),
                c.surface_active,
                UpdateAction::None,
            ),
            UpdateStatus::Available(info) => (
                format!("v{} available", info.version).into(),
                BADGE_BLUE,
                Some("Download"),
                BADGE_BLUE,
                UpdateAction::Download,
            ),
            UpdateStatus::Downloading { pct } => (
                format!("Downloading… {pct}%").into(),
                BADGE_BLUE,
                Some("Downloading…"),
                c.surface_active,
                UpdateAction::None,
            ),
            UpdateStatus::ReadyToInstall(_) => (
                "Ready to install".into(),
                BADGE_AMBER,
                Some("Restart to Update"),
                BADGE_AMBER,
                UpdateAction::Restart,
            ),
            UpdateStatus::Installing => (
                "Installing…".into(),
                BADGE_AMBER,
                None,
                0,
                UpdateAction::None,
            ),
            UpdateStatus::Error(e) => (
                format!("Error: {e}").into(),
                BADGE_RED,
                Some("Retry"),
                BADGE_BLUE,
                UpdateAction::Check,
            ),
        };

        // Progress bar — uses relative() so it truly tracks the card's inner width.
        let progress_bar: Option<gpui::AnyElement> =
            if let UpdateStatus::Downloading { pct } = &update_status {
                let frac = (*pct as f32 / 100.0).clamp(0.0, 1.0);
                Some(
                    div()
                        .w_full()
                        .h(px(4.))
                        .rounded_full()
                        .bg(rgb(c.surface_hover))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .w(relative(frac))
                                .bg(rgb(BADGE_BLUE)),
                        )
                        .into_any_element(),
                )
            } else {
                None
            };

        let update_row = {
            let mut row = div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(12.))
                .py(px(10.))
                .rounded(px(8.))
                .bg(rgb(c.canvas_bg))
                .mb(px(12.));

            let mut top = div().flex().flex_row().items_center().justify_between();
            top = top.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(rgb(status_color)),
                    )
                    .child(
                        div()
                            .text_size(rems(0.846))
                            .text_color(rgb(status_color))
                            .child(status_text),
                    ),
            );

            if let Some(label) = btn_label {
                let disabled = btn_action == UpdateAction::None;
                let btn = div()
                    .id("about-update-btn")
                    .px(px(10.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(btn_color))
                    .text_size(rems(0.769))
                    .text_color(rgb(btn_color))
                    .when(!disabled, |d| d.cursor_pointer())
                    .when(disabled, |d| d.opacity(0.5))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |_, _, _, cx| match btn_action {
                            UpdateAction::Check => cx.dispatch_action(&CheckForUpdates),
                            UpdateAction::Download => cx.dispatch_action(&InstallUpdate),
                            UpdateAction::Restart => cx.dispatch_action(&InstallUpdate),
                            UpdateAction::None => {}
                        }),
                    )
                    .child(label);
                top = top.child(btn);
            }

            row = row.child(top);
            if let Some(bar) = progress_bar {
                row = row.child(bar);
            }
            row
        };

        // ── Backdrop ──────────────────────────────────────────────────────
        let backdrop = div().absolute().inset_0().bg(rgba(0x00000088));

        // ── Card ──────────────────────────────────────────────────────────
        let card = div()
            .w(px(420.))
            .rounded(px(12.))
            .bg(rgb(c.surface_default))
            .border_1()
            .border_color(rgb(c.surface_hover))
            .flex()
            .flex_col()
            .items_center()
            .px(px(32.))
            .py(px(28.))
            .gap(px(4.))
            // Logo
            .child(img("logo-about.png").w(px(356.)).h(px(118.)).mb(px(4.)))
            // App name + version
            .child(
                div()
                    .text_size(rems(1.4))
                    .text_color(rgb(c.text_default))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .mt(px(8.))
                    .child(format!("VASSL  {version}")),
            )
            // Channel badge
            .child(
                div()
                    .px(px(8.))
                    .py(px(2.))
                    .rounded_full()
                    .bg(rgb(channel_badge_color(channel)))
                    .text_size(rems(0.769))
                    .text_color(rgb(0xffffff))
                    .mt(px(2.))
                    .mb(px(10.))
                    .child(channel),
            )
            // Commit row
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .w_full()
                    .mb(px(6.))
                    .child(
                        div()
                            .text_size(rems(0.769))
                            .text_color(rgb(BADGE_BLUE))
                            .child("Commit"),
                    )
                    .child(
                        div()
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .font_family(SharedString::from("monospace"))
                            .child(commit),
                    ),
            )
            // Full version row
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .w_full()
                    .mb(px(16.))
                    .child(
                        div()
                            .text_size(rems(0.769))
                            .text_color(rgb(BADGE_BLUE))
                            .child("Version"),
                    )
                    .child(
                        div()
                            .text_size(rems(0.846))
                            .text_color(rgb(c.text_default))
                            .font_family(SharedString::from("monospace"))
                            .child(full_ver),
                    ),
            )
            // Update status panel
            .child(update_row)
            // Buttons
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .w_full()
                    .child({
                        let hover = rgb(0x1d5ab0);
                        div()
                            .id("about-ok")
                            .flex_1()
                            .py(px(8.))
                            .rounded(px(6.))
                            .bg(rgb(BADGE_BLUE))
                            .hover(move |s| s.bg(hover))
                            .text_size(rems(0.923))
                            .text_color(rgb(0xffffff))
                            .text_align(gpui::TextAlign::Center)
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|_, _, _, cx| cx.emit(AboutEvent::Dismissed)),
                            )
                            .child("Ok")
                    })
                    .child({
                        let hover = rgb(c.surface_hover);
                        div()
                            .id("about-copy")
                            .flex_1()
                            .py(px(8.))
                            .rounded(px(6.))
                            .bg(rgb(c.surface_active))
                            .hover(move |s| s.bg(hover))
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .text_align(gpui::TextAlign::Center)
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|_, _, _, cx| cx.emit(AboutEvent::Copied)),
                            )
                            .child("Copy")
                    }),
            );

        // ── Overlay ───────────────────────────────────────────────────────
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(card)
    }
}

fn channel_badge_color(channel: &str) -> u32 {
    match channel {
        "Alpha" => 0xcc5500,
        "Beta" => 0x1d6fa5,
        "Preview" => 0x6b3fa0,
        "Stable" => 0x2a7a3b,
        "Nightly" => 0x333333,
        _ => 0x555555, // Dev
    }
}
