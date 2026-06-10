use gpui::{Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rems, rgb, rgba, SharedString};
use vassl_ui::{TextInput, ThemeHandle, text_field};


#[derive(Debug)]
pub enum FirstRunEvent { Saved }

impl EventEmitter<FirstRunEvent> for FirstRunPrompt {}

pub struct FirstRunPrompt {
    name_input:   gpui::Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        Err("Please enter your name.".to_string())
    } else {
        Ok(name)
    }
}

impl FirstRunPrompt {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            name_input:   cx.new(|cx| TextInput::with_placeholder("e.g. John Doe", cx)),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let raw = self.name_input.read(cx).text().to_string();
        match validate_name(&raw) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok(name) => {
                let db = vassl_db::AppDatabase::global(&**cx).clone();
                cx.spawn(async move |this, cx| {
                    let result = db.write(move |conn| -> anyhow::Result<()> {
                        vassl_db::shared::set_current_user(conn, &name)
                    }).await;
                    if let Err(e) = result {
                        tracing::error!("set_current_user failed: {e:?}");
                        return Ok(());
                    }
                    this.update(cx, |_, cx| cx.emit(FirstRunEvent::Saved))
                }).detach();
            }
        }
    }
}

impl Focusable for FirstRunPrompt {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for FirstRunPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let name_focused = self.name_input.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(480.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(16.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_col().gap(px(4.))
                            .child(div().text_size(rems(1.154)).text_color(rgb(c.text_default)).child("Welcome to VASSL"))
                            .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                .child("Enter your name to get started. It will be used for audit logs."))
                    )
                    // ── field ────────────────────────────────────────────
                    .child(
                        div().flex().flex_col().px(px(20.)).pt(px(8.)).pb(px(4.))
                            .child(
                                div().flex().flex_row().items_center().py(px(12.))
                                    .child(div().w(px(120.)).text_size(rems(0.923)).text_color(rgb(c.text_default)).child("Your Name"))
                                    .child(div().flex_1().child(text_field("", self.name_input.clone(), name_focused, false, cx)))
                            )
                            .child(
                                div().h(px(18.)).flex().items_center()
                                    .child(div().text_size(rems(0.846)).text_color(rgb(c.status_red))
                                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                            )
                    )
                    // ── footer ────────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .flex().flex_row().justify_end()
                            .child(
                                div().id("first-run-btn-save")
                                    .px(px(20.)).py(px(8.)).rounded(px(5.))
                                    .bg(rgb(c.surface_active))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.save(cx);
                                    }))
                                    .child("Get Started →")
                            )
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_name;
    #[test] fn rejects_empty()      { assert!(validate_name("").is_err()); }
    #[test] fn rejects_whitespace() { assert!(validate_name("   ").is_err()); }
    #[test] fn accepts_name()       { assert_eq!(validate_name("  Alice  ").unwrap(), "Alice"); }
}
