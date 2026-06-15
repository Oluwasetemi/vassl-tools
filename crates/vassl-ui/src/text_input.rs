use std::ops::Range;

use gpui::{
    actions, div, fill, point, prelude::*, px, relative, rems, rgb, rgba, size, App, Bounds,
    ClipboardItem, Context, ElementId, ElementInputHandler, EntityInputHandler, FocusHandle,
    Focusable, GlobalElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style, TextAlign, TextRun,
    UTF16Selection, Window,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{TextContextMenuHandle, ThemeColors, ThemeHandle};

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Tab,
        BackTab,
        ShowCharacterPalette
    ]
);

pub struct TextInput {
    pub focus_handle: FocusHandle,
    pub content: SharedString,
    pub placeholder: SharedString,
    pub suppress_placeholder: bool,
    /// Mask the displayed text with bullet characters (for password fields).
    pub is_password: bool,
    // When true, the cursor is rendered even if this handle is not the focused one.
    // Used by the Dropdown search box to show a cursor while popup_focus is focused.
    pub show_cursor: bool,
    pub scroll_x: Pixels,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    pub context_menu_pos: Option<Point<Pixels>>,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_placeholder("", cx)
    }

    pub fn with_text(text: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let text: SharedString = text.into();
        let len = text.len();
        Self {
            focus_handle: cx.focus_handle(),
            content: text,
            placeholder: "".into(),
            suppress_placeholder: false,
            is_password: false,
            show_cursor: false,
            scroll_x: px(0.),
            selected_range: len..len,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            context_menu_pos: None,
        }
    }

    pub fn with_placeholder(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: placeholder.into(),
            suppress_placeholder: false,
            is_password: false,
            show_cursor: false,
            scroll_x: px(0.),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            context_menu_pos: None,
        }
    }

    /// Keep cursor visible as text overflows the input bounds.
    /// `cursor_x` — cursor position in text space (pixels from text start).
    /// `line_width` — total text width in pixels.
    /// `bounds_width` — visible viewport width in pixels.
    pub fn scroll_to_cursor(&mut self, cursor_x: Pixels, line_width: Pixels, bounds_width: Pixels) {
        if cursor_x < self.scroll_x {
            self.scroll_x = cursor_x
        } else if cursor_x > self.scroll_x + bounds_width {
            self.scroll_x = cursor_x - bounds_width
        }
        // Clamp scroll_x to [px(0.), (line_width - bounds_width).max(px(0.))].
        let max_scroll = (line_width - bounds_width).max(px(0.));
        self.scroll_x = self.scroll_x.clamp(px(0.), max_scroll);
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        let len = self.content.len();
        self.selected_range = len..len;
        cx.notify();
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.content = "".into();
        self.scroll_x = px(0.);
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        // Viewport x → text-space x by adding back the scroll offset
        line.closest_index_for_x(position.x - bounds.left() + self.scroll_x)
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        window.focus(&self.focus_handle, cx);
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }
    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
        }
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }
    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    /// Copy selected text to clipboard (no Window needed — safe to call from context menu).
    pub fn do_copy(&mut self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    /// Cut selected text to clipboard (no Window needed).
    pub fn do_cut(&mut self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            let r = self.selected_range.clone();
            self.content = (self.content[..r.start].to_owned() + &self.content[r.end..]).into();
            self.selected_range = r.start..r.start;
            self.marked_range.take();
            cx.notify();
        }
    }

    /// Paste clipboard text at cursor (no Window needed).
    pub fn do_paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
            let t = text.replace('\n', " ");
            let r = self.selected_range.clone();
            self.content =
                (self.content[..r.start].to_owned() + &t + &self.content[r.end..]).into();
            self.selected_range = r.start + t.len()..r.start + t.len();
            self.marked_range.take();
            cx.notify();
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|r| self.range_to_utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_sel: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_sel
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| r.start + range.start..r.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let sx = self.scroll_x;
        Some(Bounds::from_corners(
            point(
                bounds.left() + last.x_for_index(range.start) - sx,
                bounds.top(),
            ),
            point(
                bounds.left() + last.x_for_index(range.end) - sx,
                bounds.bottom(),
            ),
        ))
    }
    fn character_index_for_point(
        &mut self,
        pt: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let line_pt = self.last_bounds?.localize(&pt)?;
        let last = self.last_layout.as_ref()?;
        let utf8 = last.index_for_x(pt.x - line_pt.x)?;
        Some(self.offset_to_utf16(utf8))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(gpui::CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    this.context_menu_pos = Some(event.position);
                    let entity = cx.entity();
                    let pos = event.position;
                    let has_sel = !this.selected_range.is_empty();
                    let menu_hdl = cx
                        .try_global::<TextContextMenuHandle>()
                        .map(|h| h.0.clone());
                    if let Some(h) = menu_hdl {
                        h.update(cx, |state, cx| {
                            state.position = Some(pos);
                            state.input = Some(entity);
                            state.has_selection = has_sel;
                            cx.notify();
                        });
                    }
                    cx.stop_propagation();
                }),
            )
            .child(TextElement { input: cx.entity() })
    }
}

// ── Custom Element ──────────────────────────────────────────────────────────────

pub struct TextElement {
    pub input: gpui::Entity<TextInput>,
}

pub struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
    cursor_pos: Pixels,
    line_width: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let is_password = input.is_password;
        let sel = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let scroll_x = input.scroll_x;
        let suppress_ph = input.suppress_placeholder;
        let style = window.text_style();
        let c: ThemeColors = cx.global::<ThemeHandle>().0.clone();

        let (display, color) = if content.is_empty() {
            if suppress_ph {
                (SharedString::from(""), style.color)
            } else {
                (
                    input.placeholder.clone(),
                    rgba(((c.text_muted << 8) | 0x99) as u32).into(),
                )
            }
        } else if is_password {
            let n = content.chars().count();
            (SharedString::from("•".repeat(n)), style.color)
        } else {
            (content.clone(), style.color)
        };

        let run = TextRun {
            len: display.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display, font_size, &[run], None);

        let cursor_pos_raw: Pixels = line.x_for_index(cursor);
        let line_width: Pixels = if content.is_empty() {
            px(0.)
        } else {
            line.x_for_index(content.len())
        };
        let cursor_fill = rgb(c.surface_active);
        let sel_fill = rgba(((c.surface_active << 8) | 0x66) as u32);

        // All positions are in viewport space: text_space_x - scroll_x
        let (selection, cursor_quad) = if sel.is_empty() {
            let vx = bounds.left() + cursor_pos_raw - scroll_x;
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(vx, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    cursor_fill,
                )),
            )
        } else {
            let start_vx = bounds.left() + line.x_for_index(sel.start) - scroll_x;
            let end_vx = bounds.left() + line.x_for_index(sel.end) - scroll_x;
            (
                Some(fill(
                    Bounds::from_corners(
                        point(start_vx, bounds.top()),
                        point(end_vx, bounds.bottom()),
                    ),
                    sel_fill,
                )),
                None,
            )
        };

        PrepaintState {
            line: Some(line),
            cursor: cursor_quad,
            selection,
            scroll_x,
            cursor_pos: cursor_pos_raw,
            line_width,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        let scroll_x = prepaint.scroll_x;

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        if let Some(sel) = prepaint.selection.take() {
            window.paint_quad(sel);
        }

        if let Some(line) = prepaint.line.take() {
            // Shift text left by scroll_x so the visible window shows the right portion
            let text_origin = point(bounds.origin.x - scroll_x, bounds.origin.y);
            let _ = line.paint(
                text_origin,
                window.line_height(),
                TextAlign::Left,
                None,
                window,
                cx,
            );

            let cursor_pos = prepaint.cursor_pos;
            let line_width = prepaint.line_width;
            let bw = bounds.size.width;
            self.input.update(cx, |input, _| {
                input.last_layout = Some(line);
                input.last_bounds = Some(bounds);
                input.scroll_to_cursor(cursor_pos, line_width, bw);
            });
        }

        let show_cursor = self.input.read(cx).show_cursor;
        if focus_handle.is_focused(window) || show_cursor {
            if let Some(cur) = prepaint.cursor.take() {
                window.paint_quad(cur);
            }
        }
    }
}

/// Convenience: a labelled text input widget for use in forms.
/// Returns a `div` wrapping the focusable text element, with an optional label above.
pub fn text_field(
    label: &str,
    input: gpui::Entity<TextInput>,
    focused: bool,
    error: bool,
    cx: &gpui::App,
) -> impl IntoElement {
    let c = cx.global::<ThemeHandle>().0.clone();
    let border_color = if error {
        c.status_red
    } else if focused {
        c.surface_active
    } else {
        c.surface_default
    };
    let field_bg = if focused {
        c.canvas_bg
    } else {
        c.surface_default
    };
    gpui::div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .when(!label.is_empty(), |d| {
            d.child(
                gpui::div()
                    .text_size(rems(0.846))
                    .text_color(gpui::rgb(c.text_muted))
                    .child(label.to_string()),
            )
        })
        .child(
            gpui::div()
                .px(px(8.))
                .py(px(4.))
                .bg(gpui::rgb(field_bg))
                .rounded(px(4.))
                .when(focused || error, |d| d.border_2())
                .when(!focused && !error, |d| d.border_1())
                .border_color(gpui::rgb(border_color))
                .text_size(rems(1.))
                .text_color(gpui::rgb(c.text_default))
                .overflow_hidden()
                .child(input),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_content_is_empty() {
        let content: SharedString = "".into();
        assert!(content.is_empty());
    }

    #[test]
    fn replace_text_updates_content_string() {
        let mut content = "hello".to_string();
        let range = 2..4;
        content = content[..range.start].to_owned() + "r" + &content[range.end..];
        assert_eq!(content, "hero");
    }

    #[test]
    fn previous_grapheme_boundary() {
        // "café" — c(0), a(1), f(2), é(3) where é is 2 UTF-8 bytes starting at byte 3
        let s = "café";
        let indices: Vec<usize> = s.grapheme_indices(true).map(|(i, _)| i).collect();
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn empty_content_previous_boundary_is_zero() {
        let s = "";
        let prev = s
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < 1).then_some(i))
            .unwrap_or(0);
        assert_eq!(prev, 0);
    }
}
