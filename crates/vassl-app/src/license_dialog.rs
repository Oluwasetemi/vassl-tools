use gpui::{Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_ui::{TextInput, ThemeHandle, text_field};

pub enum LicenseDialogEvent { Validated }

impl EventEmitter<LicenseDialogEvent> for LicenseDialog {}

pub struct LicenseDialog {
    key_input:    gpui::Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

impl LicenseDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            key_input:    cx.new(|cx| TextInput::with_placeholder("VASSL-XXXXX-XXXXX-XXXXX-XXXXX", cx)),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let raw = self.key_input.read(cx).text().to_string();
        match crate::license::validate_key(&raw) {
            Ok(_info) => {
                let db = vassl_db::AppDatabase::global(&**cx).clone();
                let key_to_save = raw.trim().to_string();
                cx.spawn(async move |this, cx| {
                    let _ = db.write(move |conn| -> anyhow::Result<()> {
                        vassl_db::shared::set_setting(conn, "license.key", &key_to_save)?;
                        Ok(())
                    }).await;
                    let _ = this.update(cx, |_, cx| cx.emit(LicenseDialogEvent::Validated));
                    Ok::<(), anyhow::Error>(())
                }).detach();
            }
            Err(e) => {
                self.error = Some(e.to_string());
                cx.notify();
            }
        }
    }
}

impl Focusable for LicenseDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for LicenseDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let key_focused = self.key_input.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x000000CC))
            .child(
                div()
                    .w(px(520.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    .child(
                        div()
                            .px(px(20.)).py(px(16.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_col().gap(px(4.))
                            .child(div().text_size(rems(1.154)).text_color(rgb(c.text_default)).child("License Required"))
                            .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                .child("Enter your VASSL license key to continue. Contact your VASSL administrator if you don't have one."))
                    )
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(12.))
                                    .child(div().w(px(110.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("License Key"))
                                    .child(div().flex_1().child(text_field("", self.key_input.clone(), key_focused, false, cx)))
                            )
                            .child(
                                div().h(px(20.)).flex().items_center()
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end()
                            .child(
                                div().id("license-btn-activate")
                                    .px(px(20.)).py(px(8.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.submit(cx);
                                    }))
                                    .child("Activate →")
                            )
                    )
            )
    }
}
