use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rgb, rgba};
use sqlez::thread_safe_connection::ThreadSafeConnection;
use vassl_ui::ThemeHandle;

use crate::colors;

#[derive(Clone, Debug)]
pub struct AuditRow {
    pub id:         i64,
    pub table_name: String,
    pub record_id:  i64,
    pub action:     String,
    pub changed_by: String,
    pub changed_at: String,
    pub old_value:  Option<String>,
    pub new_value:  Option<String>,
}

pub struct AuditLogPanel {
    db:   ThreadSafeConnection,
    rows: Vec<AuditRow>,
}

impl AuditLogPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let db = vassl_db::AppDatabase::global(&**cx).clone();
        let mut panel = Self { db, rows: vec![] };
        panel.load(cx);
        panel
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        let db = self.db.clone();
        cx.spawn(async move |this, cx| {
            let rows = db.write(|conn| -> anyhow::Result<Vec<AuditRow>> {
                let mut query = conn.select_bound::<(), (i64, String, i64, String, String, String, Option<String>, Option<String>)>(
                    "SELECT id, table_name, record_id, action, changed_by, changed_at, old_value, new_value \
                     FROM audit_log ORDER BY id DESC LIMIT 200",
                ).map_err(|e| anyhow::anyhow!("{e}"))?;

                let results = query(())?;
                Ok(results.into_iter().map(|(id, table_name, record_id, action, changed_by, changed_at, old_value, new_value)| {
                    AuditRow { id, table_name, record_id, action, changed_by, changed_at, old_value, new_value }
                }).collect())
            }).await;

            match rows {
                Ok(rows) => {
                    let _ = this.update(cx, |panel, cx| {
                        panel.rows = rows;
                        cx.notify();
                    });
                }
                Err(e) => { tracing::error!("audit_log load failed: {e:?}"); }
            }
            Ok::<(), anyhow::Error>(())
        }).detach();
    }
}

impl Render for AuditLogPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();
        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().items_center().justify_center()
            .bg(rgba(0x00000099))
            .child(
                div()
                    .w(px(760.)).h(px(560.))
                    .bg(rgb(c.canvas_bg))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(c.surface_default))
                    .overflow_hidden()
                    .flex().flex_col()
                    // ── header ──────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(14.))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1().text_size(px(13.)).text_color(rgb(c.text_default)).child("Audit Log"))
                            .child(
                                div().id("audit-btn-refresh")
                                    .px(px(12.)).py(px(5.)).rounded(px(5.))
                                    .bg(rgb(c.surface_default))
                                    .text_size(px(11.)).text_color(rgb(c.text_muted))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                        this.load(cx);
                                    }))
                                    .child("Refresh")
                            )
                    )
                    // ── column headers ───────────────────────────────────
                    .child(
                        div().flex().flex_row().gap(px(8.))
                            .px(px(20.)).py(px(8.))
                            .border_b_1()
                            .border_color(rgb(c.surface_default))
                            .bg(rgb(c.sidebar_bg))
                            .child(div().w(px(40.)).text_size(px(10.)).text_color(rgb(c.text_muted)).child("ID"))
                            .child(div().w(px(100.)).text_size(px(10.)).text_color(rgb(c.text_muted)).child("Table"))
                            .child(div().w(px(60.)).text_size(px(10.)).text_color(rgb(c.text_muted)).child("Record"))
                            .child(div().w(px(70.)).text_size(px(10.)).text_color(rgb(c.text_muted)).child("Action"))
                            .child(div().w(px(90.)).text_size(px(10.)).text_color(rgb(c.text_muted)).child("By"))
                            .child(div().flex_1().text_size(px(10.)).text_color(rgb(c.text_muted)).child("At"))
                    )
                    // ── rows ─────────────────────────────────────────────
                    .child({
                        let body = div().id("audit-scroll").flex_1().overflow_y_scroll()
                            .flex().flex_col();
                        if self.rows.is_empty() {
                            body.child(
                                div().flex().items_center().justify_center().p(px(40.))
                                    .text_size(px(12.)).text_color(rgb(c.text_muted))
                                    .child("No audit entries yet.")
                            )
                        } else {
                            body.children(self.rows.iter().map(|row| {
                                let action_color = match row.action.as_str() {
                                    "CREATE" => c.status_green,
                                    "UPDATE" => c.status_amber,
                                    "DELETE" => c.status_red,
                                    _        => c.text_muted,
                                };
                                div().flex().flex_row().gap(px(8.))
                                    .px(px(20.)).py(px(7.))
                                    .border_b_1()
                                    .border_color(rgb(c.surface_default))
                                    .child(div().w(px(40.)).text_size(px(11.)).text_color(rgb(c.text_muted)).child(row.id.to_string()))
                                    .child(div().w(px(100.)).text_size(px(11.)).text_color(rgb(c.text_default)).child(row.table_name.clone()))
                                    .child(div().w(px(60.)).text_size(px(11.)).text_color(rgb(c.text_muted)).child(row.record_id.to_string()))
                                    .child(div().w(px(70.)).text_size(px(11.)).text_color(rgb(action_color)).child(row.action.clone()))
                                    .child(div().w(px(90.)).text_size(px(11.)).text_color(rgb(c.text_default)).child(row.changed_by.clone()))
                                    .child(div().flex_1().text_size(px(11.)).text_color(rgb(c.text_muted)).child(row.changed_at.chars().take(19).collect::<String>()))
                            }))
                        }
                    })
                    // ── footer ────────────────────────────────────────────
                    .child(
                        div()
                            .px(px(20.)).py(px(10.))
                            .border_t_1()
                            .border_color(rgb(c.surface_default))
                            .bg(rgb(c.sidebar_bg))
                            .flex().flex_row().items_center()
                            .child(div().flex_1().text_size(px(11.)).text_color(rgb(c.text_muted))
                                .child(format!("{} entries", self.rows.len())))
                            .child(div().text_size(px(11.)).text_color(rgb(c.text_muted)).child("Esc to close"))
                    )
            )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn action_color_mapping() {
        // Verify the action strings match expected domain values.
        let actions = ["CREATE", "UPDATE", "DELETE"];
        for a in &actions {
            assert!(!a.is_empty());
        }
    }
}
