use gpui::{Context, EventEmitter, IntoElement, MouseButton, MouseDownEvent,
           Render, Window, div, prelude::*, px, rems, rgb};

use crate::ThemeHandle;

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
}

impl Dropdown {
    pub fn new(
        placeholder:   impl Into<String>,
        empty_message: impl Into<String>,
    ) -> Self {
        Self {
            selected_id:   None,
            is_open:       false,
            placeholder:   placeholder.into(),
            items:         Vec::new(),
            loading:       true,
            empty_message: empty_message.into(),
        }
    }

    pub fn set_items(&mut self, items: Vec<DropdownItem>, loading: bool, cx: &mut Context<Self>) {
        self.items   = items;
        self.loading = loading;
        cx.notify();
    }

    pub fn selected_label(&self) -> Option<String> {
        self.selected_id
            .and_then(|id| self.items.iter().find(|i| i.id == id))
            .map(|item| match &item.sublabel {
                Some(sub) => format!("{} / {}", item.label, sub),
                None      => item.label.clone(),
            })
    }
}

impl Render for Dropdown {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c       = cx.global::<ThemeHandle>().0.clone();
        let is_open = self.is_open;
        let label   = self.selected_label();

        div().flex().flex_col()
            // ── trigger button ───────────────────────────────────────
            .child(
                div()
                    .id("dropdown-trigger")
                    .flex().flex_row().items_center().gap(px(6.))
                    .px(px(12.)).py(px(7.))
                    .bg(rgb(c.surface_default)).rounded(px(5.))
                    .border_1().border_color(rgb(c.surface_active))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.is_open = !this.is_open;
                        cx.notify();
                    }))
                    .child(
                        div().flex_1().text_size(rems(0.923))
                            .text_color(rgb(if label.is_some() { c.text_default } else { c.text_muted }))
                            .child(label.unwrap_or_else(|| self.placeholder.clone()))
                    )
                    .child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child("◇"))
            )
            // ── inline list ──────────────────────────────────────────
            .when(is_open, |d| {
                let list = div()
                    .id("dropdown-list")
                    .mt(px(2.))
                    .max_h(px(150.)).overflow_y_scroll()
                    .bg(rgb(c.surface_default)).rounded(px(4.))
                    .border_1().border_color(rgb(c.surface_active));

                let list = if self.loading {
                    list.child(
                        div().px(px(10.)).py(px(8.))
                            .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                            .child("Loading…")
                    )
                } else if self.items.is_empty() {
                    list.child(
                        div().px(px(10.)).py(px(8.))
                            .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                            .child(self.empty_message.clone())
                    )
                } else {
                    list.children(self.items.iter().map(|item| {
                        let id       = item.id;
                        let selected = self.selected_id == Some(id);
                        let bg       = if selected { c.surface_active } else { c.surface_default };
                        div()
                            .id(format!("dropdown-item-{id}"))
                            .flex().flex_row().items_center()
                            .px(px(10.)).py(px(6.))
                            .bg(rgb(bg)).cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                this.selected_id = Some(id);
                                this.is_open     = false;
                                cx.emit(DropdownEvent::Selected(id));
                                cx.notify();
                            }))
                            .child(
                                div().flex_1().text_size(rems(0.923)).text_color(rgb(c.text_default))
                                    .child(item.label.clone())
                            )
                            .when_some(item.sublabel.clone(), |d, sub| {
                                d.child(div().text_size(rems(0.846)).text_color(rgb(c.text_muted)).child(sub))
                            })
                    }))
                };

                d.child(list)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_label_none_when_no_selection() {
        let d = Dropdown::new("Pick one", "Empty");
        assert!(d.selected_label().is_none());
    }

    #[test]
    fn selected_label_formats_with_sublabel() {
        let mut d = Dropdown::new("Pick one", "Empty");
        d.items = vec![DropdownItem { id: 1, label: "Alpha".into(), sublabel: Some("Acme".into()) }];
        d.selected_id = Some(1);
        assert_eq!(d.selected_label().unwrap(), "Alpha / Acme");
    }

    #[test]
    fn selected_label_without_sublabel() {
        let mut d = Dropdown::new("Pick one", "Empty");
        d.items = vec![DropdownItem { id: 2, label: "Beta".into(), sublabel: None }];
        d.selected_id = Some(2);
        assert_eq!(d.selected_label().unwrap(), "Beta");
    }

    #[test]
    fn loading_state_is_true_by_default() {
        let d = Dropdown::new("Pick one", "Empty");
        assert!(d.loading);
    }
}
