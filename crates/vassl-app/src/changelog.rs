use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rems, rgb};
use vassl_ui::ThemeHandle;

const CHANGELOG: &str = r#"
## v0.1.0-alpha.6  —  2026-06-10

### Added
- Delete records: products, suppliers, price entries, quotations and projects can now be deleted. Enable in Settings → General → Allow Deleting Records.
- Edit price entries: existing price book entries can now be edited. Enable in Settings → General → Allow Editing Price Entries.
- Validation red outlines on required form fields.
- Scroll-to-selected: clicking a product or price row now scrolls the list to bring the item into full view.
- Context menu viewport clamping across all panels — menus never overflow the window edge.

### Fixed
- Esc key now properly restores focus to the main window after closing a form modal.
- Light mode text contrast improvements.
- User name now populates correctly on first launch without requiring a restart.
- Windows: top frame no longer overlaps the titlebar on DWM-transparent windows.

---

## v0.1.0-alpha.5  —  2026-05-01

### Added
- Price book with cost, duty, markup, and selling price tracking.
- Quotations module: create, manage, and add line items to quotations.
- Projects module: link quotations to client projects.
- Supplier management.
- Global search across products, suppliers, and quotations.
- Audit log of all create/update/delete actions.

### Fixed
- Database migration ordering issues on fresh installs.
"#;

pub struct ChangelogPanel;

impl ChangelogPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self { Self }
}

impl Render for ChangelogPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();

        div()
            .flex_1().flex().flex_col()
            .bg(rgb(c.canvas_bg))
            .child(
                div()
                    .px(px(32.)).pt(px(24.)).pb(px(12.))
                    .child(div().text_size(rems(1.385)).text_color(rgb(c.text_default)).child("Changelog"))
                    .child(div().text_size(rems(0.923)).text_color(rgb(c.text_muted)).mt(px(4.)).child("What's new in VASSL"))
            )
            .child(div().h(px(1.)).mx(px(32.)).bg(rgb(c.surface_default)))
            .child(
                div()
                    .id("changelog-scroll")
                    .flex_1().overflow_y_scroll()
                    .px(px(32.)).pt(px(16.)).pb(px(32.))
                    .children(render_changelog_lines(&c))
            )
    }
}

fn render_changelog_lines(c: &vassl_ui::ThemeColors) -> Vec<gpui::AnyElement> {
    CHANGELOG.lines().map(|line| {
        let (size, color, bold, indent) = if line.starts_with("## ") {
            (rems(1.154f32), c.text_default, true, px(0.))
        } else if line.starts_with("### ") {
            (rems(1.0f32), c.text_default, true, px(0.))
        } else if line.starts_with("- ") {
            (rems(0.923f32), c.text_muted, false, px(16.))
        } else if line.starts_with("---") {
            return div().h(px(1.)).my(px(12.)).bg(rgb(c.surface_default)).into_any_element();
        } else if line.trim().is_empty() {
            return div().h(px(8.)).into_any_element();
        } else {
            (rems(0.923f32), c.text_muted, false, px(0.))
        };

        let text = if line.starts_with("## ") { line[3..].to_string() }
            else if line.starts_with("### ") { line[4..].to_string() }
            else if line.starts_with("- ") { format!("• {}", &line[2..]) }
            else { line.to_string() };

        let mut el = div()
            .ml(indent)
            .text_size(size)
            .text_color(rgb(color))
            .mb(px(4.))
            .child(text);
        if bold { el = el.font_weight(gpui::FontWeight::BOLD); }
        el.into_any_element()
    }).collect()
}
