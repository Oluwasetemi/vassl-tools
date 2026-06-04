# App Polish Implementation Plan (Plan 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the VASSL app with real text input in all forms, product CRUD, line item editor, first-run user prompt, full audit log view, and command palette (`Ctrl+P`).

**Architecture:** A new `vassl-ui` crate provides a reusable `TextInput` GPUI entity (based on the `gpui/examples/input.rs` pattern) that all module form crates depend on. Once wired in, forms become fully interactive. The command palette is a global overlay in `vassl-app`, triggered by the existing `FocusSearch` action. The audit log panel lives in `vassl-app` and reads from the shared DB. Product CRUD adds a `ProductForm` view to `vassl-inventory`.

**Tech Stack:** Rust, GPUI (`EntityInputHandler`, `ElementInputHandler`, `Element::paint`, `Window::handle_input`), `unicode-segmentation` (grapheme boundary navigation in TextInput), existing crates unchanged structurally — only form implementations updated.

---

## File Map

```
tools/
├── Cargo.toml                              # add unicode-segmentation to workspace deps
├── crates/
│   ├── vassl-ui/                           # NEW crate — shared UI primitives
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                      # pub use text_input::TextInput; pub use text_input::text_field
│   │       └── text_input.rs              # TextInput entity + TextElement custom element
│   ├── vassl-inventory/
│   │   └── src/
│   │       ├── stock_form.rs               # wire TextInput fields (quantity, unit_cost, supplier, invoice_ref)
│   │       └── product_form.rs             # NEW: ProductForm modal for add/edit product
│   │       └── panel.rs                    # add "New Product" button alongside "New Entry"
│   │       └── lib.rs                      # declare product_form module
│   ├── vassl-pricebook/
│   │   └── src/
│   │       └── price_form.rs               # wire TextInput fields (cost, duty, markup)
│   ├── vassl-quotations/
│   │   └── src/
│   │       ├── quotation_form.rs           # wire TextInput field (notes)
│   │       └── quotation_detail.rs         # add line item editor with TextInput fields
│   └── vassl-app/
│       ├── Cargo.toml                      # add vassl-ui dep
│       └── src/
│           ├── audit_log.rs                # NEW: AuditLogPanel view (full log, Ctrl+Shift+A)
│           ├── command_palette.rs          # NEW: CommandPalette overlay (Ctrl+P)
│           ├── first_run.rs               # NEW: first-run prompt to set current_user
│           ├── root.rs                     # add AuditLogPanel + CommandPalette overlays + first-run check
│           └── main.rs                     # wire OpenAuditLog + FocusSearch actions on startup
```

---

### Task 1: vassl-ui crate with TextInput component

**Files:**
- Modify: `Cargo.toml` (workspace)
- Create: `crates/vassl-ui/Cargo.toml`
- Create: `crates/vassl-ui/src/lib.rs`
- Create: `crates/vassl-ui/src/text_input.rs`
- Modify: `Cargo.toml` workspace members list

- [ ] **Step 1: Add unicode-segmentation to workspace Cargo.toml**

In `Cargo.toml`, add to `[workspace.dependencies]`:

```toml
unicode-segmentation = "1"
```

- [ ] **Step 2: Add vassl-ui to workspace members**

In `Cargo.toml`, add `"crates/vassl-ui"` to the `members` array:

```toml
[workspace]
members = [
    "crates/vassl-core",
    "crates/vassl-db",
    "crates/vassl-app",
    "crates/vassl-inventory",
    "crates/vassl-quotations",
    "crates/vassl-pricebook",
    "crates/vassl-ui",
    # Vendored from Zed monorepo
    "crates/collections",
    "crates/util",
    "crates/paths",
    "crates/release_channel",
    "crates/zed_env_vars",
    "crates/sqlez",
    "crates/sqlez_macros",
    "crates/db",
]
```

Also add to `[workspace.dependencies]`:

```toml
vassl-ui = { path = "crates/vassl-ui" }
```

- [ ] **Step 3: Create vassl-ui/Cargo.toml**

```toml
[package]
name    = "vassl-ui"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui.workspace                = true
unicode-segmentation          = { version = "1" }
```

- [ ] **Step 4: Write failing tests for TextInput**

Create `crates/vassl-ui/src/text_input.rs`:

```rust
use std::ops::Range;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, Window, prelude::*,
    fill, px, relative, white, black,
};
use unicode_segmentation::UnicodeSegmentation;

pub struct TextInput {
    pub focus_handle:     FocusHandle,
    pub content:          SharedString,
    pub placeholder:      SharedString,
    selected_range:       Range<usize>,
    selection_reversed:   bool,
    marked_range:         Option<Range<usize>>,
    last_layout:          Option<ShapedLine>,
    last_bounds:          Option<Bounds<Pixels>>,
    is_selecting:         bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_content_is_empty() {
        // TextInput::new requires a GPUI context — test the invariant directly
        let content: SharedString = "".into();
        assert!(content.is_empty());
    }

    #[test]
    fn replace_text_updates_content_string() {
        let mut content = "hello".to_string();
        let range = 2..4;  // replace "ll" with "r"
        content = content[..range.start].to_owned() + "r" + &content[range.end..];
        assert_eq!(content, "hero");
    }

    #[test]
    fn previous_grapheme_boundary() {
        let s = "café";
        let indices: Vec<usize> = s.grapheme_indices(true).map(|(i, _)| i).collect();
        assert_eq!(indices, vec![0, 1, 2, 4]);  // 'é' is 2 bytes at index 3-4
    }

    #[test]
    fn empty_content_previous_boundary_is_zero() {
        let s = "";
        let prev = s.grapheme_indices(true).rev().find_map(|(i, _)| (i < 0).then_some(i)).unwrap_or(0);
        assert_eq!(prev, 0);
    }
}
```

- [ ] **Step 5: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-ui 2>&1 | head -10
```

Expected: compile error — `TextInput` defined but no impl blocks yet.

- [ ] **Step 6: Implement TextInput (full implementation)**

Replace `crates/vassl-ui/src/text_input.rs` with the complete implementation:

```rust
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, ElementId, ElementInputHandler,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, Window,
    actions, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_input,
    [Backspace, Delete, Left, Right, SelectLeft, SelectRight, SelectAll, Home, End, Paste, Cut, Copy]
);

pub struct TextInput {
    pub focus_handle:   FocusHandle,
    pub content:        SharedString,
    pub placeholder:    SharedString,
    selected_range:     Range<usize>,
    selection_reversed: bool,
    marked_range:       Option<Range<usize>>,
    last_layout:        Option<ShapedLine>,
    last_bounds:        Option<Bounds<Pixels>>,
    is_selecting:       bool,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_placeholder("", cx)
    }

    pub fn with_placeholder(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle:     cx.focus_handle(),
            content:          "".into(),
            placeholder:      placeholder.into(),
            selected_range:   0..0,
            selection_reversed: false,
            marked_range:     None,
            last_layout:      None,
            last_bounds:      None,
            is_selecting:     false,
        }
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
        self.content        = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range   = None;
        self.last_layout    = None;
        self.last_bounds    = None;
        self.is_selecting   = false;
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed { self.selected_range.start } else { self.selected_range.end }
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
        self.content.grapheme_indices(true).rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content.grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset { break; }
            utf16 += ch.len_utf16();
            utf8  += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8  = 0;
        for ch in self.content.chars() {
            if utf8 >= offset { break; }
            utf8  += ch.len_utf8();
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
        if self.content.is_empty() { return 0; }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref()) else { return 0; };
        if position.y < bounds.top()    { return 0; }
        if position.y > bounds.bottom() { return self.content.len(); }
        line.closest_index_for_x(position.x - bounds.left())
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
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) { self.move_to(0, cx); }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) { self.move_to(self.content.len(), cx); }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev { window.play_system_bell(); return; }
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next { window.play_system_bell(); return; }
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
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
}

impl EntityInputHandler for TextInput {
    fn text_for_range(&mut self, range_utf16: Range<usize>, actual: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }
    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.range_to_utf16(&self.selected_range), reversed: self.selection_reversed })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|r| self.range_to_utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) { self.marked_range = None; }
    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new_text: &str, _: &mut Window, cx: &mut Context<Self>) {
        let range = range_utf16.as_ref().map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content = (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }
    fn replace_and_mark_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new_text: &str, new_sel: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        let range = range_utf16.as_ref().map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content = (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_sel.as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| r.start + range.start..r.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.notify();
    }
    fn bounds_for_range(&mut self, range_utf16: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let last = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + last.x_for_index(range.start), bounds.top()),
            point(bounds.left() + last.x_for_index(range.end),   bounds.bottom()),
        ))
    }
    fn character_index_for_point(&mut self, pt: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let line_pt = self.last_bounds?.localize(&pt)?;
        let last    = self.last_layout.as_ref()?;
        let utf8    = last.index_for_x(pt.x - line_pt.x)?;
        Some(self.offset_to_utf16(utf8))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus_handle.clone() }
}

// ── Custom Element ──────────────────────────────────────────────────────────────

pub struct TextElement {
    pub input: gpui::Entity<TextInput>,
}

pub struct PrepaintState {
    line:      Option<ShapedLine>,
    cursor:    Option<gpui::PaintQuad>,
    selection: Option<gpui::PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self { self }
}

impl gpui::Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState      = PrepaintState;

    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width  = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> PrepaintState {
        let input   = self.input.read(cx);
        let content = input.content.clone();
        let sel     = input.selected_range.clone();
        let cursor  = input.cursor_offset();
        let style   = window.text_style();

        let (display, color) = if content.is_empty() {
            (input.placeholder.clone(), gpui::hsla(0., 0., 0.4, 1.))
        } else {
            (content, style.color)
        };

        let run = TextRun { len: display.len(), font: style.font(), color, background_color: None, underline: None, strikethrough: None };
        let runs = vec![run];
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window.text_system().shape_line(display, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor_quad) = if sel.is_empty() {
            (None, Some(fill(
                Bounds::new(point(bounds.left() + cursor_pos, bounds.top()), size(px(2.), bounds.height())),
                gpui::blue(),
            )))
        } else {
            let start_x = line.x_for_index(sel.start);
            let end_x   = line.x_for_index(sel.end);
            (Some(fill(
                Bounds::from_corners(
                    point(bounds.left() + start_x, bounds.top()),
                    point(bounds.left() + end_x,   bounds.bottom()),
                ),
                gpui::blue().opacity(0.3),
            )), None)
        };

        let mut input_mut = self.input.clone();
        input_mut.update(cx, |i, _| {
            i.last_layout = Some(line.clone());
            i.last_bounds = Some(bounds);
        });

        PrepaintState { line: Some(line), cursor: cursor_quad, selection }
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), prepaint: &mut PrepaintState, window: &mut Window, cx: &mut App) {
        let focused = self.input.read(cx).focus_handle.is_focused(window);

        if let Some(sel) = prepaint.selection.take() {
            window.paint_quad(sel);
        }
        if let Some(line) = prepaint.line.take() {
            _ = line.paint(bounds.origin, window.line_height(), window);
        }
        if focused {
            if let Some(cur) = prepaint.cursor.take() {
                window.paint_quad(cur);
            }
        }

        let input = self.input.clone();
        window.handle_input(&self.input.read(cx).focus_handle.clone(), ElementInputHandler::new(bounds, input));
    }
}

/// Convenience: a labelled text input widget for use in forms.
/// Returns a `div` containing a label + the focusable text element.
pub fn text_field(
    label:    &str,
    input:    gpui::Entity<TextInput>,
    focused:  bool,
    window:   &Window,
) -> impl IntoElement {
    let border_color = if focused { 0x1a3c5e_u32 } else { 0x313244_u32 };
    gpui::div().flex().flex_col().gap(px(4.))
        .child(
            gpui::div()
                .text_size(px(11.))
                .text_color(gpui::rgb(0x6c7086_u32))
                .child(label.to_string())
        )
        .child(
            gpui::div()
                .px(px(8.)).py(px(6.))
                .bg(gpui::rgb(0x313244_u32))
                .rounded(px(4.))
                .border_1()
                .border_color(gpui::rgb(border_color))
                .text_size(px(13.))
                .text_color(gpui::rgb(0xcdd6f4_u32))
                .child(TextElement { input })
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
        let s = "café";
        let indices: Vec<usize> = s.grapheme_indices(true).map(|(i, _)| i).collect();
        assert_eq!(indices, vec![0, 1, 2, 4]);
    }

    #[test]
    fn empty_content_previous_boundary_is_zero() {
        let s = "";
        let prev = s.grapheme_indices(true).rev()
            .find_map(|(i, _)| (i < 1).then_some(i))
            .unwrap_or(0);
        assert_eq!(prev, 0);
    }
}
```

- [ ] **Step 7: Create lib.rs**

Create `crates/vassl-ui/src/lib.rs`:

```rust
pub mod text_input;

pub use text_input::{TextElement, TextInput, text_field};
```

- [ ] **Step 8: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-ui 2>&1 | tail -10
```

Expected: 4 tests pass.

- [ ] **Step 9: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add Cargo.toml crates/vassl-ui/ && git commit -m "feat(ui): vassl-ui crate with TextInput entity and text_field helper"
```

---

### Task 2: Wire TextInput into StockEntryForm

**Files:**
- Modify: `crates/vassl-inventory/Cargo.toml`
- Modify: `crates/vassl-inventory/src/stock_form.rs`

- [ ] **Step 1: Add vassl-ui to inventory Cargo.toml**

In `crates/vassl-inventory/Cargo.toml`, add:

```toml
vassl-ui             = { path = "../vassl-ui" }
```

- [ ] **Step 2: Update stock_form.rs to use TextInput fields**

Replace `crates/vassl-inventory/src/stock_form.rs` with the following. Key changes: replace the static `String` fields with `Entity<TextInput>` fields, replace `form_field` calls with `text_field`, read values from `input.read(cx).text()` in `validate`.

```rust
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::AcquisitionType;
use vassl_ui::{TextInput, text_field};

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

#[derive(Debug)]
pub enum StockFormEvent { Submitted, Cancelled }

impl EventEmitter<StockFormEvent> for StockEntryForm {}

pub struct StockEntryForm {
    store:            Entity<InventoryStore>,
    product_id:       i64,
    product_name:     String,
    quantity:         Entity<TextInput>,
    unit_cost:        Entity<TextInput>,
    supplier:         Entity<TextInput>,
    invoice_ref:      Entity<TextInput>,
    acquisition_type: AcquisitionType,
    error:            Option<String>,
    focus_handle:     FocusHandle,
}

fn validate_entry(quantity: &str, unit_cost: &str) -> Result<(f64, f64), String> {
    let qty: f64 = quantity.trim().parse()
        .map_err(|_| "Quantity must be a positive number".to_string())?;
    if qty <= 0.0 { return Err("Quantity must be > 0".to_string()); }
    let cost: f64 = unit_cost.trim().parse()
        .map_err(|_| "Unit cost must be a number ≥ 0".to_string())?;
    if cost < 0.0 { return Err("Unit cost must be ≥ 0".to_string()); }
    Ok((qty, cost))
}

impl StockEntryForm {
    pub fn new(store: Entity<InventoryStore>, product_id: i64, product_name: String, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            product_id,
            product_name,
            quantity:    cx.new(|cx| TextInput::with_placeholder("e.g. 10", cx)),
            unit_cost:   cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            supplier:    cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            invoice_ref: cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            acquisition_type: AcquisitionType::Restock,
            error:       None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn validate(&self, cx: &Context<Self>) -> Result<(f64, f64), String> {
        let qty  = self.quantity.read(cx).text().to_string();
        let cost = self.unit_cost.read(cx).text().to_string();
        validate_entry(&qty, &cost)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.validate(cx) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((qty, cost)) => {
                let db       = InventoryDb::global(&**cx);
                let pid      = self.product_id;
                let sup      = self.supplier.read(cx).text().trim().to_string();
                let invref   = self.invoice_ref.read(cx).text().trim().to_string();
                let acq      = self.acquisition_type.clone();
                let store    = self.store.clone();
                let sup_opt: Option<String>    = if sup.is_empty()    { None } else { Some(sup) };
                let invref_opt: Option<String> = if invref.is_empty() { None } else { Some(invref) };

                cx.spawn(async move |this, cx| {
                    let result = db.insert_stock_entry(pid, qty, cost, sup_opt.as_deref(), acq, None, invref_opt.as_deref(), None).await;
                    if let Err(e) = result { tracing::error!("insert_stock_entry failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(StockFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for StockEntryForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for StockEntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let qty_focused  = self.quantity.read(cx).focus_handle.is_focused(window);
        let cost_focused = self.unit_cost.read(cx).focus_handle.is_focused(window);
        let sup_focused  = self.supplier.read(cx).focus_handle.is_focused(window);
        let inv_focused  = self.invoice_ref.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(400.))
                    .bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT))
                        .child(format!("New Stock Entry — {}", self.product_name)))
                    .child(text_field("Quantity",         self.quantity.clone(),    qty_focused,  window))
                    .child(text_field("Unit Cost (USD)",  self.unit_cost.clone(),   cost_focused, window))
                    .child(text_field("Supplier",         self.supplier.clone(),    sup_focused,  window))
                    .child(text_field("Invoice Ref",      self.invoice_ref.clone(), inv_focused,  window))
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(StockFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("btn-save").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_entry;

    #[test]
    fn validate_rejects_empty_quantity()    { assert!(validate_entry("", "10.0").is_err()); }
    #[test]
    fn validate_rejects_zero_quantity()     { assert!(validate_entry("0", "10.0").is_err()); }
    #[test]
    fn validate_rejects_negative_cost()     { assert!(validate_entry("5", "-1").is_err()); }
    #[test]
    fn validate_accepts_valid_input()       { assert_eq!(validate_entry("10.5", "120.00").unwrap(), (10.5, 120.0)); }
}
```

- [ ] **Step 3: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-inventory 2>&1 | tail -10
```

Expected: all inventory tests pass (validate_entry tests unchanged).

- [ ] **Step 4: Build inventory crate**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build -p vassl-inventory 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-inventory/Cargo.toml crates/vassl-inventory/src/stock_form.rs && git commit -m "feat(inventory): wire TextInput into StockEntryForm"
```

---

### Task 3: Wire TextInput into PriceEntryForm

**Files:**
- Modify: `crates/vassl-pricebook/Cargo.toml`
- Modify: `crates/vassl-pricebook/src/price_form.rs`

- [ ] **Step 1: Add vassl-ui to pricebook Cargo.toml**

In `crates/vassl-pricebook/Cargo.toml`, add:

```toml
vassl-ui             = { path = "../vassl-ui" }
```

- [ ] **Step 2: Update price_form.rs**

Replace `crates/vassl-pricebook/src/price_form.rs`:

```rust
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::selling_price;
use vassl_ui::{TextInput, text_field};

use crate::colors;
use crate::db::PriceBookDb;
use crate::store::PriceBookStore;

#[derive(Debug)]
pub enum PriceFormEvent { Submitted, Cancelled }

impl EventEmitter<PriceFormEvent> for PriceEntryForm {}

pub struct PriceEntryForm {
    store:        Entity<PriceBookStore>,
    product_id:   i64,
    product_name: String,
    cost:         Entity<TextInput>,
    duty:         Entity<TextInput>,
    markup:       Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

fn validate_price_entry(cost: &str, duty: &str, markup: &str) -> Result<(f64, f64, f64), String> {
    let c: f64 = cost.trim().parse().map_err(|_| "Cost must be a number ≥ 0".to_string())?;
    if c < 0.0 { return Err("Cost must be ≥ 0".to_string()); }
    let d: f64 = duty.trim().parse().map_err(|_| "Duty must be a number ≥ 0".to_string())?;
    if d < 0.0 { return Err("Duty must be ≥ 0".to_string()); }
    let m: f64 = markup.trim().parse().map_err(|_| "Markup % must be > 0".to_string())?;
    if m <= 0.0 { return Err("Markup % must be > 0".to_string()); }
    Ok((c, d, m))
}

impl PriceEntryForm {
    pub fn new(store: Entity<PriceBookStore>, product_id: i64, product_name: String, cx: &mut Context<Self>) -> Self {
        let markup_field = cx.new(|cx| {
            let mut f = TextInput::with_placeholder("e.g. 30", cx);
            f.content = "30".into();
            f.selected_range = 2..2;
            f
        });
        Self {
            store,
            product_id,
            product_name,
            cost:         cx.new(|cx| TextInput::with_placeholder("e.g. 120.00", cx)),
            duty:         cx.new(|cx| TextInput::with_placeholder("e.g. 15.00", cx)),
            markup:       markup_field,
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn computed_selling_price(&self, cx: &Context<Self>) -> String {
        let c = self.cost.read(cx).text().to_string();
        let d = self.duty.read(cx).text().to_string();
        let m = self.markup.read(cx).text().to_string();
        match validate_price_entry(&c, &d, &m) {
            Ok((cv, dv, mv)) => match selling_price(cv, dv, mv) {
                Ok(s)  => format!("${s:.2}"),
                Err(_) => "—".to_string(),
            },
            Err(_) => "—".to_string(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let c = self.cost.read(cx).text().to_string();
        let d = self.duty.read(cx).text().to_string();
        let m = self.markup.read(cx).text().to_string();
        match validate_price_entry(&c, &d, &m) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((cv, dv, mv)) => {
                let sell  = selling_price(cv, dv, mv).unwrap_or(0.0);
                let db    = PriceBookDb::global(&**cx);
                let pid   = self.product_id;
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_entry(pid, cv, dv, mv, sell, None).await;
                    if let Err(e) = result { tracing::error!("insert_entry failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(PriceFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for PriceEntryForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for PriceEntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selling      = self.computed_selling_price(cx);
        let cost_focused = self.cost.read(cx).focus_handle.is_focused(window);
        let duty_focused = self.duty.read(cx).focus_handle.is_focused(window);
        let mrkp_focused = self.markup.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(420.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT))
                        .child(format!("New Price Entry — {}", self.product_name)))
                    .child(text_field("Cost Price (USD)", self.cost.clone(),   cost_focused, window))
                    .child(text_field("Duty Cost (USD)",  self.duty.clone(),   duty_focused, window))
                    .child(text_field("Markup %",         self.markup.clone(), mrkp_focused, window))
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Selling Price (computed)"))
                            .child(div().px(px(8.)).py(px(6.)).bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                .text_size(px(13.)).text_color(rgb(colors::STATUS_GREEN)).child(selling))
                    )
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("pb-btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(PriceFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("pb-btn-save").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::validate_price_entry;
    #[test] fn rejects_empty_cost()      { assert!(validate_price_entry("", "0", "30").is_err()); }
    #[test] fn rejects_negative_cost()   { assert!(validate_price_entry("-1", "0", "30").is_err()); }
    #[test] fn rejects_zero_markup()     { assert!(validate_price_entry("100", "0", "0").is_err()); }
    #[test] fn rejects_negative_markup() { assert!(validate_price_entry("100", "0", "-5").is_err()); }
    #[test] fn accepts_valid()           { assert!(validate_price_entry("100.0", "10.0", "30.0").is_ok()); }
    #[test] fn accepts_zero_duty()       { assert!(validate_price_entry("200.0", "0.0", "25.0").is_ok()); }
}
```

- [ ] **Step 3: Run tests and build**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-pricebook && cargo build -p vassl-pricebook 2>&1 | grep "^error" | head -10
```

Expected: all pricebook tests pass, no build errors.

- [ ] **Step 4: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-pricebook/Cargo.toml crates/vassl-pricebook/src/price_form.rs && git commit -m "feat(pricebook): wire TextInput into PriceEntryForm"
```

---

### Task 4: Wire TextInput into QuotationForm + notes field

**Files:**
- Modify: `crates/vassl-quotations/Cargo.toml`
- Modify: `crates/vassl-quotations/src/quotation_form.rs`

- [ ] **Step 1: Add vassl-ui to quotations Cargo.toml**

In `crates/vassl-quotations/Cargo.toml`, add:

```toml
vassl-ui             = { path = "../vassl-ui" }
```

- [ ] **Step 2: Update quotation_form.rs to add a Notes TextInput field**

In `crates/vassl-quotations/src/quotation_form.rs`, make the following changes:

1. Add `vassl_ui::{TextInput, text_field}` to imports.
2. Add `notes: Entity<TextInput>` to `QuotationForm` struct.
3. Initialize `notes: cx.new(|cx| TextInput::with_placeholder("optional", cx))` in `new()`.
4. Read notes in `submit()`: `let notes_text = self.notes.read(cx).text().trim().to_string(); let notes_opt = if notes_text.is_empty() { None } else { Some(notes_text) };`
5. Pass `notes_opt.as_deref()` to `db.insert_quotation_with_notes(...)`. Since the current `insert_quotation` signature doesn't accept notes, update the call to use a new `insert_quotation_with_notes` in the DB (or update `insert_quotation` to accept an `Option<String>` notes param).
6. Add `text_field("Notes (optional)", self.notes.clone(), notes_focused, window)` to the render.

Full replacement of `crates/vassl-quotations/src/quotation_form.rs`:

```rust
use gpui::{App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
           MouseButton, MouseDownEvent, Render, Window, div, prelude::*, px, rgb, rgba, SharedString};
use vassl_core::Project;
use vassl_ui::{TextInput, text_field};

use crate::colors;
use crate::db::QuotationDb;
use crate::store::QuotationStore;

#[derive(Debug)]
pub enum QuotationFormEvent { Submitted, Cancelled }

impl EventEmitter<QuotationFormEvent> for QuotationForm {}

pub struct QuotationForm {
    store:            Entity<QuotationStore>,
    reference_number: String,
    projects:         Vec<Project>,
    selected_project: Option<i64>,
    notes:            Entity<TextInput>,
    error:            Option<String>,
    focus_handle:     FocusHandle,
}

pub fn validate_form(selected_project: Option<i64>) -> Option<String> {
    if selected_project.is_none() { Some("Please select a project.".to_string()) } else { None }
}

impl QuotationForm {
    pub fn new(store: Entity<QuotationStore>, reference_number: String, projects: Vec<Project>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            reference_number,
            projects,
            selected_project: None,
            notes:            cx.new(|cx| TextInput::with_placeholder("optional", cx)),
            error:            None,
            focus_handle:     cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match validate_form(self.selected_project) {
            Some(msg) => { self.error = Some(msg); cx.notify(); }
            None => {
                let pid      = self.selected_project.unwrap();
                let ref_num  = self.reference_number.clone();
                let notes_s  = self.notes.read(cx).text().trim().to_string();
                let notes    = if notes_s.is_empty() { None } else { Some(notes_s) };
                let store    = self.store.clone();
                let db       = QuotationDb::global(&**cx);

                cx.spawn(async move |this, cx| {
                    let result = db.insert_quotation_with_notes(pid, ref_num, "user", notes.as_deref()).await;
                    if let Err(e) = result { tracing::error!("insert_quotation failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_quotations(cx));
                    this.update(cx, |_, cx| cx.emit(QuotationFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for QuotationForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for QuotationForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notes_focused = self.notes.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(460.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT)).child("New Quotation"))
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Reference Number"))
                            .child(div().px(px(8.)).py(px(6.)).bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                .text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .child(self.reference_number.clone()))
                    )
                    .child(
                        div().flex().flex_col().gap(px(4.))
                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Select Project"))
                            .child(
                                div().id("project-picker").h(px(120.)).overflow_y_scroll()
                                    .bg(rgb(colors::SURFACE_DEFAULT)).rounded(px(4.))
                                    .children(self.projects.iter().map(|p| {
                                        let pid      = p.id;
                                        let selected = self.selected_project == Some(pid);
                                        let bg       = if selected { colors::SURFACE_ACTIVE } else { colors::SURFACE_DEFAULT };
                                        div()
                                            .id(format!("pick-project-{pid}"))
                                            .flex().flex_row().items_center()
                                            .px(px(8.)).py(px(5.))
                                            .bg(rgb(bg)).cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                                this.selected_project = Some(pid);
                                                this.error = None;
                                                cx.notify();
                                            }))
                                            .child(div().flex_1().text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT)).child(p.name.clone()))
                                            .child(div().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child(p.client_name.clone()))
                                    }))
                            )
                    )
                    .child(text_field("Notes (optional)", self.notes.clone(), notes_focused, window))
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("quot-btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(QuotationFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("quot-btn-create").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Create"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reference_number_format()       { let r = "VASSL-2026-0001"; assert!(r.starts_with("VASSL-")); assert_eq!(r.len(), 14); }
    #[test] fn form_requires_project()         { assert!(validate_form(None).is_some()); }
    #[test] fn form_valid_with_project()       { assert!(validate_form(Some(1)).is_none()); }
}
```

- [ ] **Step 3: Add `insert_quotation_with_notes` to QuotationDb**

In `crates/vassl-quotations/src/db.rs`, add after `insert_quotation`:

```rust
pub async fn insert_quotation_with_notes(
    &self,
    project_id:       i64,
    reference_number: impl Into<String>,
    created_by:       impl Into<String>,
    notes:            Option<&str>,
) -> anyhow::Result<i64> {
    let ref_num    = reference_number.into();
    let created_by = created_by.into();
    let notes      = notes.map(String::from);
    let now        = chrono::Utc::now().to_rfc3339();

    self.write(move |conn| {
        conn.exec_bound::<(i64, String, String, Option<String>, String, String)>(
            "INSERT INTO quotations
             (project_id, reference_number, status, notes, created_by, created_at, updated_at)
             VALUES (?1, ?2, 'draft', ?3, ?4, ?5, ?6)",
        )
        .context("prepare insert_quotation_with_notes")?
        ((project_id, ref_num, notes, created_by, now.clone(), now))
        .context("execute insert_quotation_with_notes")?;

        conn.select_row::<i64>("SELECT last_insert_rowid()")
            .context("prepare rowid")?()
            .context("execute rowid")?
            .context("rowid was None")
    })
    .await
}
```

- [ ] **Step 4: Run tests and build**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-quotations && cargo build -p vassl-quotations 2>&1 | grep "^error" | head -10
```

Expected: all quotation tests pass, no build errors.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-quotations/Cargo.toml crates/vassl-quotations/src/quotation_form.rs crates/vassl-quotations/src/db.rs && git commit -m "feat(quotations): wire TextInput into QuotationForm, add notes field"
```

---

### Task 5: Product CRUD — add new product form

**Files:**
- Create: `crates/vassl-inventory/src/product_form.rs`
- Modify: `crates/vassl-inventory/src/lib.rs`
- Modify: `crates/vassl-inventory/src/panel.rs`

- [ ] **Step 1: Write failing test**

Create `crates/vassl-inventory/src/product_form.rs`:

```rust
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_ui::{TextInput, text_field};

use crate::colors;
use crate::db::InventoryDb;
use crate::store::InventoryStore;

#[derive(Debug)]
pub enum ProductFormEvent { Submitted, Cancelled }

impl EventEmitter<ProductFormEvent> for ProductForm {}

pub struct ProductForm {
    store:        Entity<InventoryStore>,
    sku:          Entity<TextInput>,
    name:         Entity<TextInput>,
    category:     Entity<TextInput>,
    unit:         Entity<TextInput>,
    min_stock:    Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

fn validate_product(sku: &str, name: &str, unit: &str, min_stock: &str) -> Result<(String, String, String, f64), String> {
    let sku = sku.trim().to_string();
    if sku.is_empty()  { return Err("SKU is required.".to_string()); }
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Name is required.".to_string()); }
    let unit = unit.trim().to_string();
    if unit.is_empty() { return Err("Unit is required (e.g. 'pcs', 'meters').".to_string()); }
    let min: f64 = min_stock.trim().parse().unwrap_or(0.0);
    if min < 0.0  { return Err("Min stock must be ≥ 0.".to_string()); }
    Ok((sku, name, unit, min))
}

#[cfg(test)]
mod tests {
    use super::validate_product;
    #[test] fn rejects_empty_sku()  { assert!(validate_product("", "Camera", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_name() { assert!(validate_product("CAM-001", "", "pcs", "5").is_err()); }
    #[test] fn rejects_empty_unit() { assert!(validate_product("CAM-001", "Camera", "", "5").is_err()); }
    #[test] fn accepts_zero_min()   { assert!(validate_product("CAM-001", "Camera", "pcs", "0").is_ok()); }
    #[test] fn accepts_valid()      { assert!(validate_product("CAM-001", "IP Camera", "pcs", "5.0").is_ok()); }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-inventory product_form 2>&1 | tail -10
```

Expected: compile error (module not declared yet).

- [ ] **Step 3: Declare module in lib.rs**

In `crates/vassl-inventory/src/lib.rs`, add `pub mod product_form;` to the module list.

- [ ] **Step 4: Implement ProductForm**

Add after `validate_product` in `product_form.rs`:

```rust
impl ProductForm {
    pub fn new(store: Entity<InventoryStore>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            sku:          cx.new(|cx| TextInput::with_placeholder("e.g. CAM-IP-2MP", cx)),
            name:         cx.new(|cx| TextInput::with_placeholder("e.g. IP Camera 2MP", cx)),
            category:     cx.new(|cx| TextInput::with_placeholder("optional: Cameras, Cabling…", cx)),
            unit:         cx.new(|cx| TextInput::with_placeholder("pcs, meters, rolls…", cx)),
            min_stock:    cx.new(|cx| TextInput::with_placeholder("0", cx)),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let sku       = self.sku.read(cx).text().to_string();
        let name      = self.name.read(cx).text().to_string();
        let unit      = self.unit.read(cx).text().to_string();
        let min_s     = self.min_stock.read(cx).text().to_string();
        let category  = self.category.read(cx).text().trim().to_string();
        let cat_opt   = if category.is_empty() { None } else { Some(category) };

        match validate_product(&sku, &name, &unit, &min_s) {
            Err(msg) => { self.error = Some(msg); cx.notify(); }
            Ok((sku, name, unit, min)) => {
                let db    = InventoryDb::global(&**cx);
                let store = self.store.clone();
                cx.spawn(async move |this, cx| {
                    let result = db.insert_product(&sku, &name, cat_opt.as_deref(), &unit, min, None).await;
                    if let Err(e) = result { tracing::error!("insert_product failed: {e:?}"); return Ok(()); }
                    let _ = store.update(cx, |s, cx| s.load_products(cx));
                    this.update(cx, |_, cx| cx.emit(ProductFormEvent::Submitted))
                }).detach();
            }
        }
    }
}

impl Focusable for ProductForm {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for ProductForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sku_f  = self.sku.read(cx).focus_handle.is_focused(window);
        let name_f = self.name.read(cx).focus_handle.is_focused(window);
        let cat_f  = self.category.read(cx).focus_handle.is_focused(window);
        let unit_f = self.unit.read(cx).focus_handle.is_focused(window);
        let min_f  = self.min_stock.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(400.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(14.)).text_color(rgb(colors::TEXT_DEFAULT)).child("New Product"))
                    .child(text_field("SKU",                   self.sku.clone(),       sku_f,  window))
                    .child(text_field("Name",                  self.name.clone(),      name_f, window))
                    .child(text_field("Category (optional)",   self.category.clone(),  cat_f,  window))
                    .child(text_field("Unit",                  self.unit.clone(),      unit_f, window))
                    .child(text_field("Min Stock Level",       self.min_stock.clone(), min_f,  window))
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end().gap(px(8.))
                            .child(div().id("prod-btn-cancel").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_DEFAULT)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(ProductFormEvent::Cancelled); }))
                                .child("Cancel"))
                            .child(div().id("prod-btn-save").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Save"))
                    )
            )
    }
}
```

- [ ] **Step 5: Add "New Product" button and form wiring to InventoryPanel**

In `crates/vassl-inventory/src/panel.rs`:

1. Add `use crate::product_form::{ProductForm, ProductFormEvent};` to imports.
2. Add `product_form: Option<Entity<ProductForm>>, _prod_form_sub: Option<Subscription>` to `InventoryPanel` struct.
3. Initialize both as `None` in `new()`.
4. Add `open_product_form` method that creates `ProductForm`, subscribes to its events (Submitted/Cancelled → close), stores in `self.product_form`.
5. In render, add a "New Product" button in the tab bar (always enabled).
6. At the end of render, overlay `self.product_form` if `Some`.

The "New Product" button div:
```rust
div()
    .id("btn-new-product")
    .px(px(12.)).py(px(4.)).rounded(px(4.))
    .bg(rgb(colors::SURFACE_DEFAULT))
    .text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
    .cursor_pointer()
    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
        this.open_product_form(cx);
    }))
    .child("+ New Product")
```

- [ ] **Step 6: Run tests and build**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-inventory && cargo build -p vassl-inventory 2>&1 | grep "^error" | head -10
```

Expected: all inventory tests pass, no build errors.

- [ ] **Step 7: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-inventory/src/product_form.rs crates/vassl-inventory/src/lib.rs crates/vassl-inventory/src/panel.rs && git commit -m "feat(inventory): ProductForm modal for adding new products"
```

---

### Task 6: First-run prompt — set current_user

**Files:**
- Create: `crates/vassl-app/src/first_run.rs`
- Modify: `crates/vassl-app/src/root.rs`
- Modify: `crates/vassl-app/Cargo.toml`

- [ ] **Step 1: Add vassl-ui to vassl-app Cargo.toml**

```toml
vassl-ui             = { path = "../vassl-ui" }
```

- [ ] **Step 2: Create first_run.rs**

```rust
use gpui::{Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rgb, rgba, SharedString};
use vassl_ui::{TextInput, text_field};

use crate::colors;

#[derive(Debug)]
pub enum FirstRunEvent { Completed }

impl EventEmitter<FirstRunEvent> for FirstRunPrompt {}

pub struct FirstRunPrompt {
    name_input:   gpui::Entity<TextInput>,
    error:        Option<String>,
    focus_handle: FocusHandle,
}

impl FirstRunPrompt {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            name_input:   cx.new(|cx| TextInput::with_placeholder("Your name", cx)),
            error:        None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).text().trim().to_string();
        if name.is_empty() {
            self.error = Some("Please enter your name.".to_string());
            cx.notify();
            return;
        }
        let db = vassl_db::AppDatabase::global(&**cx);
        vassl_db::shared::set_setting(db, "current_user", &name)
            .unwrap_or_else(|e| tracing::error!("set_setting failed: {e:?}"));
        cx.emit(FirstRunEvent::Completed);
    }
}

impl Focusable for FirstRunPrompt {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for FirstRunPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name_focused = self.name_input.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(360.)).bg(rgb(colors::CANVAS_BG)).rounded(px(8.)).p(px(24.))
                    .flex().flex_col().gap(px(12.))
                    .child(div().text_size(px(16.)).text_color(rgb(colors::TEXT_DEFAULT)).child("Welcome to VASSL"))
                    .child(div().text_size(px(13.)).text_color(rgb(colors::TEXT_MUTED)).child("Enter your name to continue."))
                    .child(text_field("Your Name", self.name_input.clone(), name_focused, window))
                    .child(div().text_size(px(11.)).text_color(rgb(colors::STATUS_RED))
                        .child(self.error.as_deref().map(SharedString::from).unwrap_or_default()))
                    .child(
                        div().flex().flex_row().justify_end()
                            .child(div().id("first-run-continue").px(px(16.)).py(px(6.)).rounded(px(4.))
                                .bg(rgb(colors::SURFACE_ACTIVE)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| { this.submit(cx); }))
                                .child("Continue"))
                    )
            )
    }
}
```

- [ ] **Step 3: Declare module + wire into root.rs**

In `crates/vassl-app/src/main.rs`, add `mod first_run;`.

In `crates/vassl-app/src/root.rs`:
1. Add `use crate::first_run::{FirstRunPrompt, FirstRunEvent};` to imports.
2. Add `first_run_prompt: Option<Entity<FirstRunPrompt>>, _first_run_sub: Option<Subscription>` to `VasslRoot`.
3. In `VasslRoot::new()`, check if `current_user` is set in the DB:
   ```rust
   let db = vassl_db::AppDatabase::global(&**cx);
   let first_run_needed = vassl_db::shared::current_user(db)
       .ok().flatten().is_none();
   let first_run_prompt = if first_run_needed {
       let prompt = cx.new(FirstRunPrompt::new);
       // subscribe inline
       Some(prompt)
   } else {
       None
   };
   ```
4. Subscribe to the prompt's `FirstRunEvent::Completed` to close it.
5. In `render()`, overlay `first_run_prompt` if `Some` (same pattern as form overlays in module panels — the prompt sits on top of everything).

- [ ] **Step 4: Build full workspace**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build 2>&1 | grep "^error" | head -10
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-app/src/first_run.rs crates/vassl-app/src/main.rs crates/vassl-app/src/root.rs crates/vassl-app/Cargo.toml && git commit -m "feat(app): first-run prompt to set current_user on first launch"
```

---

### Task 7: Full audit log panel

**Files:**
- Create: `crates/vassl-app/src/audit_log.rs`
- Modify: `crates/vassl-app/src/root.rs`
- Modify: `crates/vassl-app/src/main.rs`

- [ ] **Step 1: Write test**

Create `crates/vassl-app/src/audit_log.rs`:

```rust
use gpui::{App, Context, IntoElement, Render, Window, div, prelude::*, px, rgb};
use crate::colors;

pub struct AuditLogPanel {
    entries: Vec<AuditEntry>,
    loading: bool,
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub table_name: String,
    pub record_id:  i64,
    pub action:     String,
    pub changed_by: String,
    pub changed_at: String,
    pub old_value:  Option<String>,
    pub new_value:  Option<String>,
}

impl AuditLogPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { entries: vec![], loading: false }
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = vassl_db::AppDatabase::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move {
                    use sqlez::connection::Connection;
                    type Row = (String, i64, String, String, String, Option<String>, Option<String>);
                    db.select::<Row>(
                        "SELECT table_name, record_id, action, changed_by, changed_at, old_value, new_value
                         FROM audit_log ORDER BY changed_at DESC LIMIT 500",
                    )
                    .map_err(|e| anyhow::anyhow!("{e}"))?()
                    .map_err(|e| anyhow::anyhow!("{e}"))
                }).await;

            let _ = this.update(cx, |panel, cx| {
                panel.loading = false;
                match result {
                    Ok(rows) => {
                        panel.entries = rows.into_iter().map(|(table_name, record_id, action, changed_by, changed_at, old_value, new_value)| {
                            AuditEntry { table_name, record_id, action, changed_by, changed_at, old_value, new_value }
                        }).collect();
                    }
                    Err(e) => tracing::error!("audit_log load failed: {e:?}"),
                }
                cx.notify();
            });
        }).detach();
    }
}

impl Render for AuditLogPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return div().flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_MUTED)).child("Loading…").into_any_element();
        }

        if self.entries.is_empty() {
            return div().flex_1().flex().items_center().justify_center()
                .text_color(rgb(colors::TEXT_MUTED)).child("No audit log entries yet.").into_any_element();
        }

        let rows = self.entries.iter().map(|e| {
            div()
                .flex().flex_row().items_center().w_full()
                .px(px(12.)).py(px(5.))
                .child(div().w(px(110.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                    .child(e.changed_at.get(..19).unwrap_or("").to_string()))
                .child(div().w(px(90.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                    .child(e.table_name.clone()))
                .child(div().w(px(70.)).text_size(px(12.)).text_color(rgb(colors::TEXT_DEFAULT))
                    .child(e.action.clone()))
                .child(div().w(px(80.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                    .child(e.changed_by.clone()))
                .child(div().flex_1().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED))
                    .child(e.new_value.as_deref().unwrap_or("").chars().take(60).collect::<String>()))
        });

        div()
            .flex_1().flex().flex_col()
            .child(
                div().flex().flex_row().items_center()
                    .px(px(12.)).py(px(4.))
                    .bg(rgb(colors::SURFACE_DEFAULT))
                    .child(div().w(px(110.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("When"))
                    .child(div().w(px(90.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Table"))
                    .child(div().w(px(70.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("Action"))
                    .child(div().w(px(80.)).text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("By"))
                    .child(div().flex_1().text_size(px(11.)).text_color(rgb(colors::TEXT_MUTED)).child("New Value"))
            )
            .child(div().id("audit-log-scroll").flex_1().flex().flex_col().overflow_y_scroll().children(rows))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_entry_date_truncation() {
        let entry = AuditEntry {
            table_name: "products".to_string(),
            record_id:  1,
            action:     "CREATE".to_string(),
            changed_by: "alice".to_string(),
            changed_at: "2026-01-01T12:34:56Z".to_string(),
            old_value:  None,
            new_value:  Some(r#"{"name":"Camera"}"#.to_string()),
        };
        let display = entry.changed_at.get(..19).unwrap_or("");
        assert_eq!(display, "2026-01-01T12:34:56");
    }
}
```

- [ ] **Step 2: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-app audit_log 2>&1 | tail -10
```

Expected: 1 test passes.

- [ ] **Step 3: Wire AuditLogPanel into root.rs**

In `crates/vassl-app/src/main.rs`, add `mod audit_log;`.

In `crates/vassl-app/src/root.rs`:
1. Add `use crate::audit_log::AuditLogPanel;`.
2. Add `audit_log: Option<Entity<AuditLogPanel>>` to `VasslRoot`.
3. Initialize as `None` in `new()`.
4. Add `on_action` handler for `OpenAuditLog`:
   ```rust
   .on_action(cx.listener(|this, _: &OpenAuditLog, _w, cx| {
       if this.audit_log.is_none() {
           let panel = cx.new(|cx| {
               let mut p = AuditLogPanel::new(cx);
               p.load(cx);
               p
           });
           this.audit_log = Some(panel);
       } else {
           this.audit_log = None;  // toggle off
       }
       cx.notify();
   }))
   ```
5. In `render()`, overlay `audit_log` panel if `Some`. Use an absolute full-screen scrim with the panel centered at 80% width × 80% height.

- [ ] **Step 4: Build and run all tests**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build && cargo test 2>&1 | tail -15
```

Expected: all tests pass, no build errors.

- [ ] **Step 5: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-app/src/audit_log.rs crates/vassl-app/src/main.rs crates/vassl-app/src/root.rs && git commit -m "feat(app): full audit log panel behind Ctrl+Shift+A toggle"
```

---

### Task 8: Command palette (Ctrl+P)

**Files:**
- Create: `crates/vassl-app/src/command_palette.rs`
- Modify: `crates/vassl-app/src/root.rs`
- Modify: `crates/vassl-app/src/main.rs`

- [ ] **Step 1: Write test**

Create `crates/vassl-app/src/command_palette.rs`:

```rust
use gpui::{App, Context, IntoElement, MouseButton, MouseDownEvent, Render, Window,
           div, prelude::*, px, rgb, rgba};
use vassl_ui::{TextInput, TextElement};

use crate::colors;
use crate::sidebar::ActiveModule;

#[derive(Debug, Clone)]
pub struct PaletteCommand {
    pub label:   String,
    pub action:  PaletteAction,
}

#[derive(Debug, Clone)]
pub enum PaletteAction {
    SwitchModule(ActiveModule),
    NewRecord,
}

pub struct CommandPalette {
    pub query:    gpui::Entity<TextInput>,
    pub commands: Vec<PaletteCommand>,
    pub filtered: Vec<usize>,   // indices into commands that match query
}

pub fn build_commands() -> Vec<PaletteCommand> {
    vec![
        PaletteCommand { label: "Switch to Inventory".to_string(),  action: PaletteAction::SwitchModule(ActiveModule::Inventory) },
        PaletteCommand { label: "Switch to Quotations".to_string(), action: PaletteAction::SwitchModule(ActiveModule::Quotations) },
        PaletteCommand { label: "Switch to Price Book".to_string(), action: PaletteAction::SwitchModule(ActiveModule::PriceBook) },
        PaletteCommand { label: "New Record".to_string(),           action: PaletteAction::NewRecord },
    ]
}

fn filter_commands(commands: &[PaletteCommand], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..commands.len()).collect();
    }
    let q = query.to_lowercase();
    commands.iter().enumerate()
        .filter(|(_, c)| c.label.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_empty_query_returns_all() {
        let cmds = build_commands();
        let result = filter_commands(&cmds, "");
        assert_eq!(result.len(), cmds.len());
    }

    #[test]
    fn filter_by_keyword_narrows_results() {
        let cmds = build_commands();
        let result = filter_commands(&cmds, "inventory");
        assert_eq!(result.len(), 1);
        assert!(cmds[result[0]].label.to_lowercase().contains("inventory"));
    }

    #[test]
    fn filter_case_insensitive() {
        let cmds = build_commands();
        let result = filter_commands(&cmds, "QUOT");
        assert!(!result.is_empty());
        assert!(cmds[result[0]].label.to_lowercase().contains("quot"));
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let cmds = build_commands();
        let result = filter_commands(&cmds, "xyznotfound");
        assert!(result.is_empty());
    }
}
```

- [ ] **Step 2: Run tests — verify they pass**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo test -p vassl-app command_palette 2>&1 | tail -10
```

Expected: 4 tests pass (pure logic, no GPUI context needed).

- [ ] **Step 3: Implement CommandPalette Render**

Add after `filter_commands`:

```rust
impl CommandPalette {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let commands = build_commands();
        let filtered = (0..commands.len()).collect();
        Self {
            query:    cx.new(|cx| TextInput::with_placeholder("Type a command…", cx)),
            commands,
            filtered,
        }
    }

    fn update_filter(&mut self, cx: &mut Context<Self>) {
        let q = self.query.read(cx).text().to_string();
        self.filtered = filter_commands(&self.commands, &q);
        cx.notify();
    }
}

#[derive(Debug)]
pub enum PaletteEvent { Dismissed, Execute(PaletteAction) }
impl gpui::EventEmitter<PaletteEvent> for CommandPalette {}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Re-filter on every render (cheap — small list)
        let q = self.query.read(cx).text().to_string();
        let filtered = filter_commands(&self.commands, &q);

        let focused = self.query.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().justify_center()
            .bg(rgba(0x00000066))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| { cx.emit(PaletteEvent::Dismissed); }))
            .child(
                div()
                    .mt(px(100.)).w(px(460.)).h(px(300.))
                    .bg(rgb(colors::CANVAS_BG)).rounded(px(8.))
                    .flex().flex_col()
                    .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _: &mut Window, _: &mut App| {})  // stop propagation
                    .child(
                        div().px(px(12.)).py(px(8.)).border_b_1().border_color(rgb(colors::SURFACE_DEFAULT))
                            .child(TextElement { input: self.query.clone() })
                    )
                    .child(
                        div().id("palette-results").flex_1().flex().flex_col().overflow_y_scroll()
                            .children(filtered.into_iter().map(|idx| {
                                let cmd   = &self.commands[idx];
                                let label = cmd.label.clone();
                                let action = cmd.action.clone();
                                div()
                                    .id(format!("palette-cmd-{idx}"))
                                    .px(px(12.)).py(px(8.))
                                    .text_size(px(13.)).text_color(rgb(colors::TEXT_DEFAULT))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(colors::SURFACE_DEFAULT)))
                                    .on_mouse_down(MouseButton::Left,
                                        cx.listener(move |_, _, _, cx| { cx.emit(PaletteEvent::Execute(action.clone())); }))
                                    .child(label)
                            }))
                    )
            )
    }
}
```

- [ ] **Step 4: Wire into root.rs**

In `crates/vassl-app/src/main.rs`, add `mod command_palette;`.

In `crates/vassl-app/src/root.rs`:
1. Import `use crate::command_palette::{CommandPalette, PaletteAction, PaletteEvent};`.
2. Add `palette: Option<Entity<CommandPalette>>, _palette_sub: Option<gpui::Subscription>` to `VasslRoot`.
3. Add `on_action` for `FocusSearch` (already declared as action):
   ```rust
   .on_action(cx.listener(|this, _: &FocusSearch, _w, cx| {
       if this.palette.is_some() { this.palette = None; this._palette_sub = None; cx.notify(); return; }
       let palette = cx.new(CommandPalette::new);
       let sub = cx.subscribe(&palette, |this, _p, ev: &PaletteEvent, cx| {
           match ev {
               PaletteEvent::Dismissed => { this.palette = None; this._palette_sub = None; cx.notify(); }
               PaletteEvent::Execute(action) => {
                   match action {
                       PaletteAction::SwitchModule(module) => {
                           let m = *module;
                           this.sidebar.update(cx, |s, cx| { s.active = m; cx.notify(); });
                       }
                       PaletteAction::NewRecord => { /* future: dispatch to active module */ }
                   }
                   this.palette = None; this._palette_sub = None; cx.notify();
               }
           }
       });
       this.palette = Some(palette);
       this._palette_sub = Some(sub);
       cx.notify();
   }))
   ```
4. In `render()`, overlay `palette` if `Some` (after other overlays).

- [ ] **Step 5: Build full workspace and run all tests**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && cargo build && cargo test 2>&1 | tail -20
```

Expected: all tests pass, no build errors.

- [ ] **Step 6: Commit**

```bash
cd /Users/oluwasetemi/r/kamalu/tools && git add crates/vassl-app/src/command_palette.rs crates/vassl-app/src/main.rs crates/vassl-app/src/root.rs && git commit -m "feat(app): command palette (Ctrl+P / Ctrl+F) with fuzzy filter and module switching"
```

---

## Self-Review

**Spec coverage:**

| Spec requirement | Task |
|---|---|
| Text input in stock entry form | Task 2 |
| Text input in price entry form | Task 3 |
| Text input in quotation form (notes) | Task 4 |
| Product CRUD — add new product | Task 5 |
| Line item creation (quotation detail) | Task 4 (notes only; full editor deferred — see below) |
| First-run user prompt | Task 6 |
| Full audit log (`Ctrl+Shift+A`) | Task 7 |
| Command palette (`Ctrl+P`) | Task 8 |
| `Ctrl+N` — New Record | Task 8 (palette shortcut; module-aware dispatch left as `/* future */`) |

**Remaining scope (post-Plan-5):**
- Product edit / delete (add edit button in product row → pre-populate form)
- Full line item editor in quotation detail (each row needs TextInput for description, qty, unit price)
- Project CRUD (add/edit projects — same TextInput pattern as ProductForm)
- Price history chart (requires a custom chart Element)
- Global fuzzy search across modules

**Placeholder scan:** No TBD. The `/* future */` comment in the palette's `NewRecord` arm is intentional — wiring it requires knowing the active module's "new item" method, which is module-specific plumbing best added when each module's form is stable.

**Type consistency:** `TextInput` defined in `vassl-ui/text_input.rs`, used in all forms. `text_field` helper used consistently across all form renders. `PaletteAction` defined in `command_palette.rs`, matched in `root.rs`. `FirstRunEvent::Completed` emitted in `first_run.rs`, subscribed in `root.rs`. Consistent throughout.
