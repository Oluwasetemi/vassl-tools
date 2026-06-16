use gpui::{
    div, prelude::*, px, rems, rgb, rgba, Context, EventEmitter, IntoElement, MouseButton,
    MouseDownEvent, Render, Window,
};
use vassl_ui::ThemeHandle;

const CHANGELOG: &str = r#"
## v0.1.0-beta.6  —  2026-06-15

### Fixed
- Startup crash on alpha databases: products, suppliers, and projects all failed to load with "no such column" errors. Columns added in beta (end_of_life, model_number, part_number, duty_percent, replacement, address, date_started, and others) were never applied to existing databases because the migration system silently skips changed CREATE TABLE steps. Each affected table is now recreated with the full current schema on first launch after upgrade, preserving all existing data.

---

## v0.1.0-beta.5  —  2026-06-15

### Fixed
- MSI installer now correctly removes the previous installation before installing the new version. Previously, installing beta.4 over alpha.15 left both versions on disk and the old shortcut kept launching the old binary.

---

## v0.1.0-beta.4  —  2026-06-15

### Added
- Auto-updater now logs every step to the log file — channel, version comparison, GitHub response, download progress, and errors. Log: %LOCALAPPDATA%\VASSL\logs\vassl.<date>.log (Windows).

### Fixed
- "Check for updates" button now stays visible while checking (shown dimmed) instead of disappearing.
- Tab order in Product form (create mode): Initial Stock now follows Replacement visually. End of Life checkbox is now keyboard-reachable (Tab to focus, Space to toggle).
- Tab order in Line Item form: Product dropdown is now the first tab stop, matching its visual position.
- Auto-updater internal: replaced unused update_url() with supports_updates() — removes misleading placeholder URLs.

---

## v0.1.0-beta.3  —  2026-06-14

### Added
- Project CLAUDE.md with agent skill configuration (issue tracker, triage labels, domain docs).
- docs/agents/ reference docs for engineering skills: issue-tracker, triage-labels, domain.

---

## v0.1.0-beta.2  —  2026-06-12

### Added
- macOS code signing and notarization support in CI.

### Fixed
- Inventory: fall back to global low stock threshold for products with no per-product minimum.
- CI: extend macOS build artifact expiry to 14 days for the beta channel.
- CI: embed vassl.icns into Contents/Resources after cargo-bundle.
- CI: use job-level env for signing gate; fix checkout ordering.
- Release: DMG creation and bundle metadata.

---

## v0.1.0-beta.1  —  2026-06-08

### Added
- Authentication and user management.
- Quotation editing.
- Currency-aware pricing.
- Auto-focus UX improvements.

---

## v0.1.0-alpha.18  —  2026-06-12

### Fixed
- Changelog and audit log panel header/footer corners now clip correctly to the border radius.
- Modal overlays no longer propagate mouse events to the panel behind them.

---

## v0.1.0-alpha.17  —  2026-06-12

### Fixed
- All modals now auto-focus their first input when opened by button click (not just keyboard shortcut).
- Esc key now works consistently across all modals regardless of how they were opened.
- Audit log now immediately reflects a name change — no restart required.
- Windows: restored clean title frame; reduced default window height to fit 768 px laptop screens.
- About dialog now distinguishes "Up to date" (not yet checked) from "Already up to date" (checked, no update found).

---

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

pub enum ChangelogEvent {
    Dismissed,
}

impl EventEmitter<ChangelogEvent> for ChangelogPanel {}

pub struct ChangelogPanel;

impl ChangelogPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for ChangelogPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();

        // Backdrop — covers the whole window, click-away closes the panel
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000CC))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.emit(ChangelogEvent::Dismissed);
                }),
            )
            .child(
                // Panel — stops click propagation so clicking inside doesn't dismiss
                div()
                    .id("changelog-panel")
                    .w(px(600.))
                    .h(px(520.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header
                    .child(
                        div()
                            .px(px(24.))
                            .pt(px(20.))
                            .pb(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .rounded_t(px(10.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .text_size(rems(1.154))
                                            .text_color(rgb(c.text_default))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child("Changelog"),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.846))
                                            .text_color(rgb(c.text_muted))
                                            .child("What's new in VASSL"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("changelog-close")
                                    .w(px(28.))
                                    .h(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(5.))
                                    .bg(rgb(c.surface_default))
                                    .text_size(rems(1.))
                                    .text_color(rgb(c.text_muted))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                                            cx.emit(ChangelogEvent::Dismissed);
                                        }),
                                    )
                                    .child("×"),
                            ),
                    )
                    .child(div().h(px(1.)).bg(rgb(c.surface_default)))
                    // Scrollable content
                    .child(
                        div()
                            .id("changelog-scroll")
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .px(px(24.))
                            .pt(px(16.))
                            .pb(px(32.))
                            .children(render_changelog_lines(&c)),
                    ),
            )
    }
}

fn render_changelog_lines(c: &vassl_ui::ThemeColors) -> Vec<gpui::AnyElement> {
    CHANGELOG
        .lines()
        .map(|line| {
            let (size, color, bold, indent) = if line.starts_with("## ") {
                (rems(1.077f32), c.text_default, true, px(0.))
            } else if line.starts_with("### ") {
                (rems(0.923f32), c.text_default, true, px(0.))
            } else if line.starts_with("- ") {
                (rems(0.923f32), c.text_muted, false, px(16.))
            } else if line.starts_with("---") {
                return div()
                    .h(px(1.))
                    .my(px(12.))
                    .bg(rgb(c.surface_default))
                    .into_any_element();
            } else if line.trim().is_empty() {
                return div().h(px(6.)).into_any_element();
            } else {
                (rems(0.923f32), c.text_muted, false, px(0.))
            };

            let text = if line.starts_with("## ") {
                line[3..].to_string()
            } else if line.starts_with("### ") {
                line[4..].to_string()
            } else if line.starts_with("- ") {
                format!("• {}", &line[2..])
            } else {
                line.to_string()
            };

            let mut el = div()
                .ml(indent)
                .text_size(size)
                .text_color(rgb(color))
                .mb(px(3.))
                .child(text);
            if bold {
                el = el.font_weight(gpui::FontWeight::BOLD);
            }
            el.into_any_element()
        })
        .collect()
}
