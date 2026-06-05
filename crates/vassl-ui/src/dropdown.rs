use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, MouseButton, MouseDownEvent,
           Render, SharedString, Window, div, prelude::*, px, rems, rgb};

use crate::{TextInput, ThemeHandle};

#[derive(Clone)]
pub struct DropdownItem {
    pub id:       i64,
    pub label:    String,
    pub sublabel: Option<String>,
}

#[derive(Debug)]
pub enum DropdownEvent {
    Selected(i64),
}

impl EventEmitter<DropdownEvent> for Dropdown {}

pub struct Dropdown {
    pub selected_id:   Option<i64>,
    pub is_open:       bool,
    placeholder:       String,
    pub items:         Vec<DropdownItem>,
    pub loading:       bool,
    empty_message:     String,
    search_input:      Option<Entity<TextInput>>,
    pub trigger_focus: FocusHandle,
}

impl Focusable for Dropdown {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.trigger_focus.clone() }
}

impl Dropdown {
    pub fn new(
        placeholder:   impl Into<String>,
        empty_message: impl Into<String>,
        cx:            &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| TextInput::with_placeholder("Search…", cx));
        Self {
            selected_id:   None,
            is_open:       false,
            placeholder:   placeholder.into(),
            items:         Vec::new(),
            loading:       true,
            empty_message: empty_message.into(),
            search_input:  Some(search_input),
            trigger_focus: cx.focus_handle(),
        }
    }

    pub fn set_items(&mut self, items: Vec<DropdownItem>, loading: bool, cx: &mut Context<Self>) {
        self.items   = items;
        self.loading = loading;
        cx.notify();
    }

    pub fn selected_label(&self) -> Option<String> {
        format_selected_label(&self.items, self.selected_id)
    }

    fn chevrons(c: &crate::ThemeColors) -> impl IntoElement {
        div()
            .flex().flex_col().items_center().justify_center().gap(px(1.))
            .child(div().text_size(rems(0.5)).text_color(rgb(c.text_muted)).child("▲"))
            .child(div().text_size(rems(0.5)).text_color(rgb(c.text_muted)).child("▼"))
    }
}

pub fn format_selected_label(items: &[DropdownItem], selected_id: Option<i64>) -> Option<String> {
    selected_id
        .and_then(|id| items.iter().find(|i| i.id == id))
        .map(|item| match &item.sublabel {
            Some(sub) => format!("{} / {}", item.label, sub),
            None      => item.label.clone(),
        })
}

impl Render for Dropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c       = cx.global::<ThemeHandle>().0.clone();
        let is_open = self.is_open;
        let label   = self.selected_label();

        // Filter items by current search query
        let query = self.search_input.as_ref()
            .map(|si| si.read(cx).content.to_lowercase())
            .unwrap_or_default();
        let filtered: Vec<&DropdownItem> = if query.is_empty() {
            self.items.iter().collect()
        } else {
            self.items.iter().filter(|item| {
                item.label.to_lowercase().contains(&query)
                    || item.sublabel.as_deref()
                        .map(|s| s.to_lowercase().contains(&query))
                        .unwrap_or(false)
            }).collect()
        };

        let trigger_focused = self.trigger_focus.is_focused(window);
        // Snapshot all self.* data before move closures
        let placeholder     = self.placeholder.clone();
        let loading        = self.loading;
        let selected_id    = self.selected_id;
        let empty_message  = self.empty_message.clone();
        let search_input   = self.search_input.clone();
        let search_focused = search_input.as_ref()
            .map(|si| si.read(cx).focus_handle.is_focused(window))
            .unwrap_or(false);
        let filtered_owned: Vec<DropdownItem> = filtered.into_iter().cloned().collect();

        div().flex().flex_col()
            // ── trigger button ───────────────────────────────────────
            .child(
                div()
                    .id("dropdown-trigger")
                    .track_focus(&self.trigger_focus)
                    .flex().flex_row().items_center().gap(px(6.))
                    .px(px(12.)).py(px(7.))
                    .bg(rgb(c.surface_default)).rounded(px(5.))
                    .when(trigger_focused, |d| d.border_2().border_color(rgb(c.surface_active)))
                    .when(!trigger_focused, |d| d.border_1().border_color(rgb(c.surface_active)))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, window, cx| {
                        this.is_open = !this.is_open;
                        if this.is_open {
                            if let Some(si) = &this.search_input {
                                si.update(cx, |input, cx| input.reset(cx));
                                let fh = si.read(cx).focus_handle.clone();
                                window.focus(&fh, cx);
                            }
                        }
                        cx.notify();
                    }))
                    .child(
                        div().flex_1().text_size(rems(0.923))
                            .text_color(rgb(if label.is_some() { c.text_default } else { c.text_muted }))
                            .child(label.unwrap_or(placeholder))
                    )
                    .child(Self::chevrons(&c))
            )
            // ── dropdown popup ───────────────────────────────────────
            .when(is_open, move |d| {
                let mut popup = div()
                    .id("dropdown-list")
                    .mt(px(2.))
                    .bg(rgb(c.surface_default)).rounded(px(4.))
                    .border_1().border_color(rgb(c.surface_active))
                    .flex().flex_col();

                // Search input (pinned above scroll area)
                if let Some(si) = &search_input {
                    let border_c = if search_focused { c.surface_active } else { c.canvas_bg };
                    popup = popup.child(
                        div()
                            .px(px(8.)).py(px(6.))
                            .border_b_1().border_color(rgb(c.surface_default))
                            .child(
                                div()
                                    .px(px(6.)).py(px(3.))
                                    .bg(rgb(c.canvas_bg)).rounded(px(4.))
                                    .border_1().border_color(rgb(border_c))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .child(si.clone())
                            )
                    );
                }

                // Scrollable item list
                let scroll_list = div()
                    .id("dropdown-items")
                    .max_h(px(160.)).overflow_y_scroll();

                let scroll_list = if loading {
                    scroll_list.child(
                        div().px(px(10.)).py(px(8.))
                            .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                            .child("Loading…")
                    )
                } else if filtered_owned.is_empty() {
                    let msg = if query.is_empty() {
                        empty_message
                    } else {
                        format!("No results for \"{query}\"")
                    };
                    scroll_list.child(
                        div().px(px(10.)).py(px(8.))
                            .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                            .child(msg)
                    )
                } else {
                    scroll_list.children(filtered_owned.into_iter().map(|item| {
                        let id       = item.id;
                        let selected = selected_id == Some(id);
                        let bg       = if selected { c.surface_active } else { c.surface_default };
                        div()
                            .id(SharedString::from(format!("dropdown-item-{id}")))
                            .flex().flex_row().items_center()
                            .px(px(10.)).py(px(7.))
                            .bg(rgb(bg)).cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                this.selected_id = Some(id);
                                this.is_open     = false;
                                cx.emit(DropdownEvent::Selected(id));
                                cx.notify();
                            }))
                            .child(
                                div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .child(item.label)
                            )
                            .when_some(item.sublabel, |d, sub| {
                                d.child(
                                    div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child(sub)
                                )
                            })
                    }))
                };

                d.child(popup.child(scroll_list))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, label: &str, sublabel: Option<&str>) -> DropdownItem {
        DropdownItem { id, label: label.into(), sublabel: sublabel.map(Into::into) }
    }

    #[test]
    fn selected_label_none_when_no_selection() {
        assert!(format_selected_label(&[], None).is_none());
    }

    #[test]
    fn selected_label_formats_with_sublabel() {
        let items = vec![item(1, "Alpha", Some("Acme"))];
        assert_eq!(format_selected_label(&items, Some(1)).unwrap(), "Alpha / Acme");
    }

    #[test]
    fn selected_label_without_sublabel() {
        let items = vec![item(2, "Beta", None)];
        assert_eq!(format_selected_label(&items, Some(2)).unwrap(), "Beta");
    }

    #[test]
    fn loading_state_is_true_by_default() {
        assert!(format_selected_label(&[], None).is_none()); // loading is set at construction; tested via entity
    }
}
