use gpui::{
    Context, EventEmitter, IntoElement, Render, Window,
    div, prelude::*, px, rgb, rgba, rems,
};
use release_channel::RELEASE_CHANNEL;
use vassl_ui::ThemeHandle;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT:  &str = env!("VASSL_GIT_COMMIT");

// Accent blue — consistent with the Ok button highlight
const ACCENT_BLUE: u32 = 0x2d6fce;

pub enum AboutEvent {
    Dismissed,
    Copied,
}

impl EventEmitter<AboutEvent> for AboutDialog {}

pub struct AboutDialog;

impl AboutDialog {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }

    pub fn full_version_static() -> String {
        let channel = RELEASE_CHANNEL.dev_name();
        format!("{VERSION}+{channel}.{COMMIT}")
    }
}

impl Render for AboutDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c        = cx.global::<ThemeHandle>().0.clone();
        let version  = VERSION;
        let commit   = COMMIT;
        let full_ver = Self::full_version_static();
        let channel  = RELEASE_CHANNEL.display_name();

        // ── Backdrop ──────────────────────────────────────────────────────
        let backdrop = div()
            .absolute().inset_0()
            .bg(rgba(0x00000088));

        // ── Card ──────────────────────────────────────────────────────────
        let card = div()
            .w(px(420.))
            .rounded(px(12.))
            .bg(rgb(c.surface_default))
            .border_1()
            .border_color(rgb(c.surface_hover))
            .flex().flex_col().items_center()
            .px(px(32.)).py(px(28.))
            .gap(px(4.))
            // App name + version
            .child(
                div()
                    .text_size(rems(1.4))
                    .text_color(rgb(c.text_default))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .mt(px(8.))
                    .child(format!("VASSL  {version}"))
            )
            // Channel badge
            .child(
                div()
                    .px(px(8.)).py(px(2.))
                    .rounded_full()
                    .bg(rgb(channel_badge_color(channel)))
                    .text_size(rems(0.769))
                    .text_color(rgb(0xffffff))
                    .mt(px(2.)).mb(px(10.))
                    .child(channel)
            )
            // Commit row
            .child(
                div()
                    .flex().flex_col().items_center().gap(px(2.)).w_full()
                    .mb(px(6.))
                    .child(
                        div()
                            .text_size(rems(0.769))
                            .text_color(rgb(ACCENT_BLUE))
                            .child("Commit")
                    )
                    .child(
                        div()
                            .text_size(rems(0.923))
                            .text_color(rgb(c.text_default))
                            .font_family(gpui::SharedString::from("monospace"))
                            .child(commit)
                    )
            )
            // Version string row
            .child(
                div()
                    .flex().flex_col().items_center().gap(px(2.)).w_full()
                    .mb(px(20.))
                    .child(
                        div()
                            .text_size(rems(0.769))
                            .text_color(rgb(ACCENT_BLUE))
                            .child("Version")
                    )
                    .child(
                        div()
                            .text_size(rems(0.846))
                            .text_color(rgb(c.text_default))
                            .font_family(gpui::SharedString::from("monospace"))
                            .child(full_ver)
                    )
            )
            // Buttons
            .child(
                div()
                    .flex().flex_row().gap(px(8.)).w_full()
                    // Ok button (primary)
                    .child({
                        let hover = rgb(0x1d5ab0);
                        div()
                            .id("about-ok")
                            .flex_1()
                            .py(px(8.))
                            .rounded(px(6.))
                            .bg(rgb(ACCENT_BLUE))
                            .hover(move |s| s.bg(hover))
                            .text_size(rems(0.923))
                            .text_color(rgb(0xffffff))
                            .text_align(gpui::TextAlign::Center)
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|_, _, _, cx| {
                                    cx.emit(AboutEvent::Dismissed);
                                }),
                            )
                            .child("Ok")
                    })
                    // Copy button (secondary)
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
                                cx.listener(|_, _, _, cx| {
                                    cx.emit(AboutEvent::Copied);
                                }),
                            )
                            .child("Copy")
                    })
            );

        // ── Overlay: backdrop then card centred on top ─────────────────────
        div()
            .absolute().inset_0()
            .flex().items_center().justify_center()
            .child(backdrop)
            .child(card)
    }
}

fn channel_badge_color(channel: &str) -> u32 {
    match channel {
        "Alpha"   => 0xcc5500,
        "Beta"    => 0x1d6fa5,
        "Preview" => 0x6b3fa0,
        "Stable"  => 0x2a7a3b,
        "Nightly" => 0x333333,
        _         => 0x555555, // Dev
    }
}
