use gpui::{Context, EventEmitter, FocusHandle, Focusable, IntoElement, MouseButton,
           Render, SharedString, Window, actions, div, prelude::*, px, rems, rgb};
use vassl_ui::{TextInput, ThemeHandle, text_field};

use crate::users_db::{AuthUser, UsersDb};

actions!(login_panel, [EscapeForm, TabField, BackTabField]);

#[derive(Debug)]
pub enum LoginPanelEvent {
    Authenticated(AuthUser),
}
impl EventEmitter<LoginPanelEvent> for LoginPanel {}

#[derive(Clone, Copy, PartialEq)]
enum LoginMode { Checking, Setup, Login }

pub struct LoginPanel {
    mode:            LoginMode,
    username_input:  gpui::Entity<TextInput>,
    password_input:  gpui::Entity<TextInput>,
    confirm_input:   gpui::Entity<TextInput>,
    error:           Option<String>,
    loading:         bool,
    auto_focused:    bool,
    submit_focus:    FocusHandle,
    focus_handle:    FocusHandle,
}

impl Focusable for LoginPanel {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl LoginPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let pw_input = cx.new(|cx| {
            let mut t = TextInput::with_placeholder("Password", cx);
            t.is_password = true;
            t
        });
        let confirm_input = cx.new(|cx| {
            let mut t = TextInput::with_placeholder("Confirm password", cx);
            t.is_password = true;
            t
        });

        let panel = Self {
            mode:           LoginMode::Checking,
            username_input: cx.new(|cx| TextInput::with_placeholder("Username", cx)),
            password_input: pw_input,
            confirm_input,
            error:          None,
            loading:        false,
            auto_focused:   false,
            submit_focus:   cx.focus_handle(),
            focus_handle:   cx.focus_handle(),
        };

        let db = UsersDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let count = cx.background_executor()
                .spawn(async move { db.user_count() })
                .await;
            let _ = this.update(cx, |p, cx| {
                p.mode = if count.unwrap_or(1) == 0 { LoginMode::Setup } else { LoginMode::Login };
                cx.notify();
            });
            Ok::<(), anyhow::Error>(())
        }).detach();

        panel
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.username_input.update(cx, |t, cx| t.reset(cx));
        self.password_input.update(cx, |t, cx| t.reset(cx));
        self.confirm_input.update(cx, |t, cx| t.reset(cx));
        self.error        = None;
        self.loading      = false;
        self.auto_focused = false;
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        match self.mode {
            LoginMode::Setup    => self.submit_setup(cx),
            LoginMode::Login    => self.submit_login(cx),
            LoginMode::Checking => {}
        }
    }

    fn submit_login(&mut self, cx: &mut Context<Self>) {
        let username = self.username_input.read(cx).text().trim().to_string();
        let password = self.password_input.read(cx).text().to_string();

        if username.is_empty() {
            self.error = Some("Username is required.".into());
            cx.notify();
            return;
        }
        if password.is_empty() {
            self.error = Some("Password is required.".into());
            cx.notify();
            return;
        }

        self.loading = true;
        self.error   = None;
        cx.notify();

        let db = UsersDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.verify_credentials(username.clone(), password).await;
            let _ = this.update(cx, |p, cx| {
                p.loading = false;
                match result {
                    Err(e) => {
                        p.error = Some(format!("Login failed: {e}"));
                        cx.notify();
                    }
                    Ok(None) => {
                        p.error = Some("Invalid username or password.".into());
                        cx.notify();
                    }
                    Ok(Some(user)) => {
                        let uid   = user.id;
                        let uname = user.username.clone();
                        let log_db = UsersDb::global(&**cx);
                        cx.spawn(async move |_, _cx| {
                            let _ = log_db.log_auth_event(uid, "LOGIN", &uname).await;
                            Ok::<(), anyhow::Error>(())
                        }).detach();
                        cx.emit(LoginPanelEvent::Authenticated(user));
                    }
                }
            });
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    fn submit_setup(&mut self, cx: &mut Context<Self>) {
        let username = self.username_input.read(cx).text().trim().to_string();
        let password = self.password_input.read(cx).text().to_string();
        let confirm  = self.confirm_input.read(cx).text().to_string();

        if username.is_empty() {
            self.error = Some("Username is required.".into());
            cx.notify();
            return;
        }
        if username.len() < 3 {
            self.error = Some("Username must be at least 3 characters.".into());
            cx.notify();
            return;
        }
        if password.len() < 6 {
            self.error = Some("Password must be at least 6 characters.".into());
            cx.notify();
            return;
        }
        if password != confirm {
            self.error = Some("Passwords do not match.".into());
            cx.notify();
            return;
        }

        self.loading = true;
        self.error   = None;
        cx.notify();

        let db = UsersDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.insert_admin(username.clone(), &password).await;
            let _ = this.update(cx, |p, cx| {
                p.loading = false;
                match result {
                    Err(e) => {
                        p.error = Some(format!("Setup failed: {e}"));
                        cx.notify();
                    }
                    Ok(id) => {
                        let log_db = UsersDb::global(&**cx);
                        let uname  = username.clone();
                        cx.spawn(async move |_, _cx| {
                            let _ = log_db.log_auth_event(id, "ADMIN_SETUP", &uname).await;
                            Ok::<(), anyhow::Error>(())
                        }).detach();
                        let user = AuthUser {
                            id, username,
                            is_admin: true, can_inventory: true, can_pricebook: true,
                            can_quotations: true, allow_delete: true, allow_price_edit: true,
                            must_change_password: false, is_active: true,
                        };
                        cx.emit(LoginPanelEvent::Authenticated(user));
                    }
                }
            });
            Ok::<(), anyhow::Error>(())
        }).detach();
    }

    fn tab_handles(&self, cx: &gpui::App) -> Vec<FocusHandle> {
        let mut handles = vec![
            self.username_input.read(cx).focus_handle.clone(),
            self.password_input.read(cx).focus_handle.clone(),
        ];
        if self.mode == LoginMode::Setup {
            handles.push(self.confirm_input.read(cx).focus_handle.clone());
        }
        handles.push(self.submit_focus.clone());
        handles
    }
}

impl Render for LoginPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c       = cx.global::<ThemeHandle>().0.clone();
        let loading = self.loading;
        let mode    = self.mode;

        if !self.auto_focused && mode != LoginMode::Checking {
            self.auto_focused = true;
            let h = self.username_input.read(cx).focus_handle.clone();
            window.focus(&h, cx);
        }

        let (title, subtitle, btn_label) = match mode {
            LoginMode::Checking => ("VASSL", "Loading…", ""),
            LoginMode::Setup    => ("Welcome to VASSL", "Create the administrator account to get started.", "Create Account"),
            LoginMode::Login    => ("Sign In", "Sign in to continue.", "Sign In"),
        };

        let u_focused      = self.username_input.read(cx).focus_handle.is_focused(window);
        let pw_focused     = self.password_input.read(cx).focus_handle.is_focused(window);
        let cf_focused     = self.confirm_input.read(cx).focus_handle.is_focused(window);
        let submit_focused = self.submit_focus.is_focused(window);

        let error_el = self.error.as_deref().map(|msg| {
            div().px(px(24.)).pb(px(4.))
                .text_size(rems(0.846)).text_color(rgb(c.status_red))
                .child(SharedString::from(msg.to_string()))
        });

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgb(c.canvas_bg))
            .font_family(gpui::SharedString::from(c.font_family.clone()))
            .key_context("LoginPanel")
            .on_action(cx.listener(|this, _: &TabField, window, cx| {
                let handles = this.tab_handles(cx);
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = handles[(current.map(|i| i + 1).unwrap_or(0)) % handles.len()].clone();
                window.focus(&next, cx);
            }))
            .on_action(cx.listener(|this, _: &BackTabField, window, cx| {
                let handles = this.tab_handles(cx);
                let current = handles.iter().position(|h| h.is_focused(window));
                let prev = handles[(current.unwrap_or(0) + handles.len() - 1) % handles.len()].clone();
                window.focus(&prev, cx);
            }))
            .on_action(cx.listener(|this, _: &EscapeForm, window, cx| {
                // Focus username field on Escape to reset keyboard position
                let h = this.username_input.read(cx).focus_handle.clone();
                window.focus(&h, cx);
            }))
            .child(
                div()
                    .w(px(400.))
                    .bg(rgb(c.surface_default))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_hover))
                    .overflow_hidden()
                    .flex().flex_col()
                    .child(
                        div()
                            .px(px(24.)).py(px(20.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_col().gap(px(4.))
                            .child(div().text_size(rems(1.231)).text_color(rgb(c.text_default)).child(title))
                            .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted)).child(subtitle))
                    )
                    .when(mode != LoginMode::Checking, |card| {
                        card
                            .child(
                                div().flex().flex_col().px(px(24.)).pt(px(16.)).pb(px(8.)).gap(px(10.))
                                    .child(text_field("Username", self.username_input.clone(), u_focused, false, cx))
                                    .child(text_field("Password", self.password_input.clone(), pw_focused, false, cx))
                                    .when(mode == LoginMode::Setup, |d| {
                                        d.child(text_field("Confirm Password", self.confirm_input.clone(), cf_focused, false, cx))
                                    })
                            )
                            .children(error_el)
                            .child(
                                div()
                                    .px(px(24.)).py(px(16.))
                                    .border_t_1().border_color(rgb(c.surface_hover))
                                    .flex().justify_end()
                                    .child({
                                        let btn_bg = if loading { c.surface_default } else { c.surface_active };
                                        div()
                                            .id("login-btn-submit")
                                            .track_focus(&self.submit_focus)
                                            .w(px(148.)).flex().items_center().justify_center()
                                            .py(px(8.)).rounded(px(5.))
                                            .bg(rgb(btn_bg))
                                            .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                            .when(!loading, |d| d.cursor_pointer())
                                            .when(submit_focused, |d| d.border_2().border_color(rgb(c.text_muted)))
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                this.submit(cx);
                                            }))
                                            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                                                if event.keystroke.key == "enter" {
                                                    this.submit(cx);
                                                }
                                            }))
                                            .child(if loading { "Please wait…" } else { btn_label })
                                    })
                            )
                    })
            )
    }
}
