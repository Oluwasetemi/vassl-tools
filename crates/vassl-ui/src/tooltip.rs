use gpui::{AnyView, App, BoxShadow, Context, IntoElement, Render, SharedString, Window,
           div, hsla, point, prelude::*, px, rems, rgb};

use crate::ThemeHandle;

/// A styled tooltip view.  Create and attach with the [`tooltip`] or [`tooltip_keyed`] helpers.
pub struct Tooltip {
    title:    SharedString,
    key_hint: Option<SharedString>,
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();

        let shadow = vec![BoxShadow {
            color:         hsla(0., 0., 0., 0.28),
            offset:        point(px(0.), px(3.)),
            blur_radius:   px(10.),
            spread_radius: px(0.),
            inset:         false,
        }];

        div()
            .px(px(9.))
            .py(px(6.))
            .rounded(px(5.))
            .bg(rgb(c.surface_default))
            .shadow(shadow)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(rems(0.846))
                    .text_color(rgb(c.text_default))
                    .child(self.title.clone()),
            )
            .when_some(self.key_hint.clone(), |d, key| {
                d.child(
                    div()
                        .px(px(5.))
                        .py(px(2.))
                        .rounded(px(3.))
                        .bg(rgb(c.surface_hover))
                        .text_size(rems(0.769))
                        .text_color(rgb(c.text_muted))
                        .child(key),
                )
            })
    }
}

/// Attach a plain text tooltip to any element.
///
/// ```ignore
/// div().id("btn").tooltip(tooltip("Inventory"))
/// ```
pub fn tooltip(
    title: impl Into<SharedString> + 'static,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let title: SharedString = title.into();
    move |_window, cx| {
        cx.new(|_| Tooltip { title: title.clone(), key_hint: None }).into()
    }
}

/// Attach a tooltip that shows a label AND a keyboard shortcut badge.
///
/// ```ignore
/// div().id("btn").tooltip(tooltip_keyed("Inventory", "Ctrl+1"))
/// ```
pub fn tooltip_keyed(
    title:    impl Into<SharedString> + 'static,
    key_hint: impl Into<SharedString> + 'static,
) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let title:    SharedString = title.into();
    let key_hint: SharedString = key_hint.into();
    move |_window, cx| {
        cx.new(|_| Tooltip { title: title.clone(), key_hint: Some(key_hint.clone()) }).into()
    }
}
