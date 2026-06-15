use gpui::{
    actions, deferred, div, prelude::*, px, rems, rgb, Context, Entity, EventEmitter, FocusHandle,
    Focusable, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Render, SharedString,
    Window,
};

use crate::{TextInput, ThemeHandle};

// Navigation actions for the dropdown. Bound to keys in main.rs under key_context("Dropdown").
// Using on_action (not on_key_down) so they bubble through the focus-context chain — the same
// mechanism GlobalSearch uses for SelectNext/SelectPrev.
actions!(
    dropdown,
    [DropdownDown, DropdownUp, DropdownConfirm, DropdownClose]
);

#[derive(Clone)]
pub struct DropdownItem {
    pub id: i64,
    pub label: String,
    pub sublabel: Option<String>,
}

#[derive(Debug)]
pub enum DropdownEvent {
    Selected(i64),
}

impl EventEmitter<DropdownEvent> for Dropdown {}

pub struct Dropdown {
    pub selected_id: Option<i64>,
    pub is_open: bool,
    placeholder: String,
    pub items: Vec<DropdownItem>,
    filtered_items: Vec<DropdownItem>,
    query: String,
    pub loading: bool,
    empty_message: String,
    search_input: Option<Entity<TextInput>>,
    pub trigger_focus: FocusHandle,
    // popup_focus is tracked on the OUTER div (never inside deferred) so it is always
    // in GPUI's normal prepaint focus-dispatch pass. All keyboard input is handled here
    // via on_key_down (chars/backspace) and on_action (navigation). The TextInput inside
    // deferred() shows a fake cursor via show_cursor: bool — no actual focus required.
    popup_focus: FocusHandle,
    focused_idx: Option<usize>,
}

impl Focusable for Dropdown {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.trigger_focus.clone()
    }
}

impl Dropdown {
    pub fn new(
        placeholder: impl Into<String>,
        empty_message: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| TextInput::with_placeholder("Search…", cx));

        cx.observe(&search_input, |this, input, cx| {
            this.query = input.read(cx).content.to_lowercase();
            this.recompute_filtered();
            cx.notify();
        })
        .detach();

        Self {
            selected_id: None,
            is_open: false,
            placeholder: placeholder.into(),
            items: Vec::new(),
            filtered_items: Vec::new(),
            query: String::new(),
            loading: true,
            empty_message: empty_message.into(),
            search_input: Some(search_input),
            trigger_focus: cx.focus_handle(),
            popup_focus: cx.focus_handle(),
            focused_idx: None,
        }
    }

    pub fn set_items(&mut self, items: Vec<DropdownItem>, loading: bool, cx: &mut Context<Self>) {
        self.items = items;
        self.loading = loading;
        self.recompute_filtered();
        cx.notify();
    }

    fn recompute_filtered(&mut self) {
        self.filtered_items = if self.query.is_empty() {
            self.items.clone()
        } else {
            self.items
                .iter()
                .filter(|item| {
                    item.label.to_lowercase().contains(&self.query)
                        || item
                            .sublabel
                            .as_deref()
                            .map(|s| s.to_lowercase().contains(&self.query))
                            .unwrap_or(false)
                })
                .cloned()
                .collect()
        };
        self.focused_idx = None;
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_open = false;
        self.focused_idx = None;
        self.query = String::new();
        if let Some(si) = &self.search_input {
            // Turn off the fake cursor and clear the search box.
            si.update(cx, |t, cx| {
                t.show_cursor = false;
                t.reset(cx);
            });
        }
        self.recompute_filtered();
        let tfh = self.trigger_focus.clone();
        window.focus(&tfh, cx);
        cx.notify();
    }

    pub fn selected_label(&self) -> Option<String> {
        format_selected_label(&self.items, self.selected_id)
    }

    fn chevrons(c: &crate::ThemeColors) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(1.))
            .child(
                div()
                    .text_size(rems(0.5))
                    .text_color(rgb(c.text_muted))
                    .child("▲"),
            )
            .child(
                div()
                    .text_size(rems(0.5))
                    .text_color(rgb(c.text_muted))
                    .child("▼"),
            )
    }
}

pub fn format_selected_label(items: &[DropdownItem], selected_id: Option<i64>) -> Option<String> {
    selected_id
        .and_then(|id| items.iter().find(|i| i.id == id))
        .map(|item| match &item.sublabel {
            Some(sub) => format!("{} / {}", item.label, sub),
            None => item.label.clone(),
        })
}

impl Render for Dropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        let is_open = self.is_open;
        let label = self.selected_label();

        let trigger_focused = self.trigger_focus.is_focused(window);
        let popup_is_focused = self.popup_focus.is_focused(window);
        let show_active_border = trigger_focused || popup_is_focused || is_open;

        let placeholder = self.placeholder.clone();
        let loading = self.loading;
        let selected_id = self.selected_id;
        let empty_message = self.empty_message.clone();
        let search_input = self.search_input.clone();
        let query = self.query.clone();
        let filtered_owned: Vec<DropdownItem> = self.filtered_items.clone();
        let focused_idx = self.focused_idx;
        let popup_focus_clone = self.popup_focus.clone();

        // ── Outer div: keyboard hub ─────────────────────────────────────────────
        //
        // track_focus + on_action + on_key_down all live HERE (not inside deferred).
        // GPUI builds the focus dispatch path during the NORMAL prepaint pass; deferred()
        // subtrees are excluded from key dispatch.
        //
        // Navigation (↓↑ Enter Esc) is wired as on_action so it bubbles correctly
        // through the focus-context chain — the same pattern used by GlobalSearch.
        // Character typing and backspace are handled via on_key_down since they are
        // not bound to named actions. The TextInput inside deferred() shows a fake
        // cursor via show_cursor: bool; it does not need actual OS focus.
        div()
            .key_context("Dropdown")
            .relative()
            .w_full()
            .min_w(px(0.))
            // overflow_hidden clips the trigger text so it never grows wider than its
            // container. deferred() elements bypass parent clip rects, so the popup
            // still renders on top of subsequent form elements.
            .overflow_hidden()
            .track_focus(&popup_focus_clone)
            // ── navigation actions ──────────────────────────────────────────────
            .on_action(cx.listener(|this, _: &DropdownDown, _, cx| {
                if !this.is_open {
                    return;
                }
                let len = this.filtered_items.len();
                if len == 0 {
                    return;
                }
                this.focused_idx = Some(match this.focused_idx {
                    None => 0,
                    Some(i) => (i + 1).min(len - 1),
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &DropdownUp, _, cx| {
                if !this.is_open {
                    return;
                }
                let len = this.filtered_items.len();
                if len == 0 {
                    return;
                }
                this.focused_idx = Some(match this.focused_idx {
                    None | Some(0) => 0,
                    Some(i) => i - 1,
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &DropdownConfirm, window, cx| {
                if !this.is_open {
                    return;
                }
                if let Some(idx) = this.focused_idx {
                    if let Some(item) = this.filtered_items.get(idx).cloned() {
                        this.selected_id = Some(item.id);
                        cx.emit(DropdownEvent::Selected(item.id));
                    }
                }
                this.close(window, cx);
            }))
            .on_action(cx.listener(|this, _: &DropdownClose, window, cx| {
                if !this.is_open {
                    return;
                }
                this.close(window, cx);
            }))
            // ── character / backspace input ─────────────────────────────────────
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _w, cx| {
                if !this.is_open {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "backspace" => {
                        let current = this
                            .search_input
                            .as_ref()
                            .map(|si| si.read(cx).text().to_string())
                            .unwrap_or_default();
                        let new_text = match current.char_indices().next_back() {
                            Some((i, _)) => current[..i].to_string(),
                            None => String::new(),
                        };
                        if let Some(si) = &this.search_input {
                            si.update(cx, |t, cx| t.set_text(new_text, cx));
                        }
                    }
                    key => {
                        // Skip any key that is bound to a named action (navigation keys
                        // are handled above via on_action). Only append printable chars.
                        let typed = if key == "space" {
                            Some(" ".to_string())
                        } else if key.chars().count() == 1
                            && !event.keystroke.modifiers.control
                            && !event.keystroke.modifiers.alt
                            && !event.keystroke.modifiers.platform
                        {
                            Some(if event.keystroke.modifiers.shift {
                                key.to_uppercase()
                            } else {
                                key.to_string()
                            })
                        } else {
                            None
                        };
                        if let Some(ch) = typed {
                            let current = this
                                .search_input
                                .as_ref()
                                .map(|si| si.read(cx).text().to_string())
                                .unwrap_or_default();
                            if let Some(si) = &this.search_input {
                                si.update(cx, |t, cx| t.set_text(format!("{current}{ch}"), cx));
                            }
                        }
                    }
                }
            }))
            // ── trigger button ────────────────────────────────────────────────
            .child(
                div()
                    .id("dropdown-trigger")
                    .track_focus(&self.trigger_focus)
                    .w_full()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .px(px(12.))
                    .py(px(7.))
                    .bg(rgb(c.surface_default))
                    .rounded(px(5.))
                    .when(show_active_border, |d| {
                        d.border_2().border_color(rgb(c.surface_active))
                    })
                    .when(!show_active_border, |d| {
                        d.border_1().border_color(rgb(c.surface_active))
                    })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, window, cx| {
                            this.is_open = !this.is_open;
                            if this.is_open {
                                this.focused_idx = None;
                                this.query = String::new();
                                this.recompute_filtered();
                                if let Some(si) = &this.search_input {
                                    // Turn on the fake cursor and clear search text.
                                    si.update(cx, |t, cx| {
                                        t.show_cursor = true;
                                        t.reset(cx);
                                    });
                                }
                                // Defer so GPUI's click handling completes before we take focus.
                                let pfh = this.popup_focus.clone();
                                window.defer(cx, move |w, cx| {
                                    w.focus(&pfh, cx);
                                });
                            } else {
                                this.close(window, cx);
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(rems(0.923))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(rgb(if label.is_some() {
                                c.text_default
                            } else {
                                c.text_muted
                            }))
                            .child(SharedString::from(label.unwrap_or(placeholder))),
                    )
                    .child(div().flex_shrink_0().child(Self::chevrons(&c))),
            )
            // ── visual popup (deferred for z-order) ───────────────────────────
            .when(is_open, move |d| {
                let search_row = search_input.as_ref().map(|si| {
                    let border_c = if popup_is_focused {
                        c.surface_active
                    } else {
                        c.canvas_bg
                    };
                    div()
                        .w_full()
                        .px(px(8.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(rgb(c.surface_default))
                        .child(
                            div()
                                .w_full()
                                .px(px(6.))
                                .py(px(3.))
                                .bg(rgb(c.canvas_bg))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(rgb(border_c))
                                .text_size(rems(0.923))
                                .text_color(rgb(c.text_default))
                                .child(si.clone()),
                        )
                });

                let item_list = {
                    let base = div()
                        .id("dropdown-items")
                        .w_full()
                        .max_h(px(160.))
                        .overflow_y_scroll();
                    if loading {
                        base.child(
                            div()
                                .px(px(10.))
                                .py(px(8.))
                                .text_size(rems(0.923))
                                .text_color(rgb(c.text_muted))
                                .child("Loading…"),
                        )
                    } else if filtered_owned.is_empty() {
                        let msg = if query.is_empty() {
                            empty_message
                        } else {
                            format!("No results for \"{query}\"")
                        };
                        base.child(
                            div()
                                .px(px(10.))
                                .py(px(8.))
                                .text_size(rems(0.923))
                                .text_color(rgb(c.text_muted))
                                .child(msg),
                        )
                    } else {
                        base.children(filtered_owned.into_iter().enumerate().map(|(i, item)| {
                            let id = item.id;
                            let selected = selected_id == Some(id);
                            let is_kb_focus = focused_idx == Some(i);
                            let bg = if selected {
                                c.surface_active
                            } else if is_kb_focus {
                                c.surface_hover
                            } else {
                                c.surface_default
                            };
                            let hover_bg = rgb(c.surface_hover);
                            let text = SharedString::from(item.sublabel.as_ref().map_or_else(
                                || item.label.clone(),
                                |sub| format!("{} · {}", item.label, sub),
                            ));
                            div()
                                .id(SharedString::from(format!("dropdown-item-{id}")))
                                .w_full()
                                .h(px(30.))
                                .overflow_hidden()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(px(10.))
                                .bg(rgb(bg))
                                .cursor_pointer()
                                .when(!selected && !is_kb_focus, |d| {
                                    d.hover(move |s| s.bg(hover_bg))
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                                        this.selected_id = Some(id);
                                        cx.emit(DropdownEvent::Selected(id));
                                        this.close(window, cx);
                                    }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_size(rems(0.923))
                                        .text_color(rgb(c.text_default))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(text),
                                )
                        }))
                    }
                };

                let mut popup = div()
                    .id("dropdown-list")
                    .absolute()
                    .top(px(36.))
                    .left(px(0.))
                    .w_full()
                    .bg(rgb(c.surface_default))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(c.surface_active))
                    .shadow_md()
                    .flex()
                    .flex_col();

                if let Some(row) = search_row {
                    popup = popup.child(row);
                }
                d.child(deferred(popup.child(item_list)))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, label: &str, sublabel: Option<&str>) -> DropdownItem {
        DropdownItem {
            id,
            label: label.into(),
            sublabel: sublabel.map(Into::into),
        }
    }

    #[test]
    fn selected_label_none_when_no_selection() {
        assert!(format_selected_label(&[], None).is_none());
    }

    #[test]
    fn selected_label_formats_with_sublabel() {
        let items = vec![item(1, "Alpha", Some("Acme"))];
        assert_eq!(
            format_selected_label(&items, Some(1)).unwrap(),
            "Alpha / Acme"
        );
    }

    #[test]
    fn selected_label_without_sublabel() {
        let items = vec![item(2, "Beta", None)];
        assert_eq!(format_selected_label(&items, Some(2)).unwrap(), "Beta");
    }
}
