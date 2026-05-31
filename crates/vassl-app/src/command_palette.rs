use gpui::{Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba};
use vassl_ui::{TextInput, text_field};

use crate::colors;

/// The set of top-level commands the palette can dispatch.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteCommand {
    OpenInventory,
    OpenQuotations,
    OpenPriceBook,
    OpenAuditLog,
}

impl PaletteCommand {
    fn label(&self) -> &'static str {
        match self {
            PaletteCommand::OpenInventory  => "Open Inventory",
            PaletteCommand::OpenQuotations => "Open Quotations",
            PaletteCommand::OpenPriceBook  => "Open Price Book",
            PaletteCommand::OpenAuditLog   => "Open Audit Log",
        }
    }

    fn all() -> &'static [PaletteCommand] {
        &[
            PaletteCommand::OpenInventory,
            PaletteCommand::OpenQuotations,
            PaletteCommand::OpenPriceBook,
            PaletteCommand::OpenAuditLog,
        ]
    }
}

#[derive(Debug)]
pub enum PaletteEvent {
    Dismissed,
    Execute(PaletteCommand),
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

pub struct CommandPalette {
    query:        gpui::Entity<TextInput>,
    selected_idx: usize,
    focus_handle: FocusHandle,
}

/// Returns commands whose label contains all words of `query` (case-insensitive).
pub fn filter_commands(query: &str) -> Vec<&'static PaletteCommand> {
    let lower = query.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    PaletteCommand::all()
        .iter()
        .filter(|cmd| {
            let label = cmd.label().to_lowercase();
            words.iter().all(|w| label.contains(w))
        })
        .collect()
}

impl CommandPalette {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            query:        cx.new(|cx| TextInput::with_placeholder("Search commands…", cx)),
            selected_idx: 0,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query_text = self.query.read(cx).text().to_string();
        let matches    = filter_commands(&query_text);

        // Clamp selected_idx to valid range.
        if self.selected_idx >= matches.len() && !matches.is_empty() {
            self.selected_idx = matches.len() - 1;
        }

        let query_focused = self.query.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().justify_center()
            .pt(px(100.))
            .bg(rgba(0x00000099))
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
                cx.emit(PaletteEvent::Dismissed);
            }))
            .child(
                div()
                    .id("palette-popup")
                    .w(px(480.))
                    .bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(12.))
                    .flex().flex_col().gap(px(8.))
                    // Prevent click-through to the backdrop dismiss handler.
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                    .child(text_field("", self.query.clone(), query_focused, window))
                    .child({
                        let results = div().id("palette-results")
                            .flex().flex_col().gap(px(2.));
                        if matches.is_empty() {
                            results.child(
                                div().px(px(10.)).py(px(8.))
                                    .text_size(px(12.)).text_color(rgb(colors::TEXT_MUTED))
                                    .child("No commands match.")
                            )
                        } else {
                            results.children(matches.iter().enumerate().map(|(idx, cmd)| {
                                let selected = idx == self.selected_idx;
                                let bg = if selected { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT };
                                let cmd_clone = (*cmd).clone();
                                div()
                                    .id(format!("palette-item-{idx}"))
                                    .px(px(10.)).py(px(7.)).rounded(px(4.))
                                    .bg(rgb(bg))
                                    .text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
                                        cx.emit(PaletteEvent::Execute(cmd_clone.clone()));
                                    }))
                                    .child(cmd.label())
                            }))
                        }
                    })
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_returns_all() {
        let r = filter_commands("");
        assert_eq!(r.len(), PaletteCommand::all().len());
    }

    #[test]
    fn filter_by_word() {
        let r = filter_commands("inv");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].label(), "Open Inventory");
    }

    #[test]
    fn filter_case_insensitive() {
        let r = filter_commands("QUOT");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].label(), "Open Quotations");
    }

    #[test]
    fn filter_no_match() {
        let r = filter_commands("zzzzz");
        assert!(r.is_empty());
    }

    #[test]
    fn filter_multi_word() {
        let r = filter_commands("open price");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].label(), "Open Price Book");
    }
}
