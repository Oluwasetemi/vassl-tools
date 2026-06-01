use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::{Project, QuotationItem, QuotationStatus};

use crate::db::{QuotationDb, QuotationRow};

pub struct QuotationStore {
    pub quotations: Vec<QuotationRow>,
    pub selected_id: Option<i64>,
    pub line_items:  Vec<QuotationItem>,
    pub projects:    Vec<Project>,
    pub loading:     bool,
}

pub struct QuotationStoreHandle(pub Entity<QuotationStore>);
impl Global for QuotationStoreHandle {}

pub fn status_badge_color(status: QuotationStatus) -> u32 {
    match status {
        QuotationStatus::Draft    => crate::colors::STATUS_GREY,
        QuotationStatus::Sent     => crate::colors::STATUS_AMBER,
        QuotationStatus::Accepted => crate::colors::STATUS_GREEN,
        QuotationStatus::Rejected => crate::colors::STATUS_RED,
    }
}

#[derive(Debug)]
pub enum QuotationEvent {
    QuotationsLoaded,
    ItemsLoaded,
}

impl EventEmitter<QuotationEvent> for QuotationStore {}

impl QuotationStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            quotations:  Vec::new(),
            selected_id: None,
            line_items:  Vec::new(),
            projects:    Vec::new(),
            loading:     false,
        }
    }

    pub fn load_quotations(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let db2 = db.clone();
            let quot_result = cx.background_executor().spawn(async move { db.list_quotations_with_project() }).await;
            let proj_result = cx.background_executor().spawn(async move { db2.list_projects() }).await;

            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match quot_result {
                    Ok(rows) => { store.quotations = rows; }
                    Err(e)   => tracing::error!("list_quotations_with_project failed: {e:?}"),
                }
                match proj_result {
                    Ok(projects) => { store.projects = projects; }
                    Err(e)       => tracing::error!("list_projects failed: {e:?}"),
                }
                cx.emit(QuotationEvent::QuotationsLoaded);
                cx.notify();
            });
        }).detach();
    }

    pub fn select_quotation(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.selected_id == Some(id) { return; }
        self.selected_id = Some(id);
        self.line_items.clear();
        cx.notify();

        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_items_for_quotation(id) })
                .await;
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(items) => { store.line_items = items; cx.emit(QuotationEvent::ItemsLoaded); }
                    Err(e)    => tracing::error!("list_items_for_quotation failed: {e:?}"),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn load_line_items(&mut self, quotation_id: i64, cx: &mut Context<Self>) {
        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_items_for_quotation(quotation_id) })
                .await;
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(items) => { store.line_items = items; cx.emit(QuotationEvent::ItemsLoaded); }
                    Err(e)    => tracing::error!("load_line_items failed: {e:?}"),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn transition_status(&mut self, id: i64, new_status: QuotationStatus, cx: &mut Context<Self>) {
        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.update_status(id, new_status).await;
            if let Err(e) = result {
                tracing::error!("update_status failed: {e:?}");
                return Ok::<(), anyhow::Error>(());
            }
            let _ = this.update(cx, |store, cx| store.load_quotations(cx));
            Ok(())
        }).detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::QuotationStatus;
    use crate::db::QuotationRow;

    fn make_row(id: i64, status: QuotationStatus) -> QuotationRow {
        QuotationRow {
            id,
            reference_number: format!("VASSL-2026-{id:04}"),
            status,
            project_id:   1,
            project_name: "Test Project".to_string(),
            client_name:  "Test Client".to_string(),
            total_usd:    0.0,
            created_at:   "2026-01-01T00:00:00Z".to_string(),
            notes:        None,
        }
    }

    #[test]
    fn quotation_store_starts_empty() {
        let store = QuotationStore {
            quotations: vec![],
            selected_id: None,
            line_items: vec![],
            projects: vec![],
            loading: false,
        };
        assert!(store.quotations.is_empty());
        assert!(store.selected_id.is_none());
    }

    #[test]
    fn selected_quotation_lookup() {
        let rows = vec![
            make_row(1, QuotationStatus::Draft),
            make_row(2, QuotationStatus::Sent),
        ];
        let found = rows.iter().find(|r| r.id == 2);
        assert!(found.is_some());
        assert_eq!(found.unwrap().status, QuotationStatus::Sent);
    }

    #[test]
    fn status_badge_color_mapping() {
        assert_eq!(status_badge_color(QuotationStatus::Draft),    crate::colors::STATUS_GREY);
        assert_eq!(status_badge_color(QuotationStatus::Sent),     crate::colors::STATUS_AMBER);
        assert_eq!(status_badge_color(QuotationStatus::Accepted), crate::colors::STATUS_GREEN);
        assert_eq!(status_badge_color(QuotationStatus::Rejected), crate::colors::STATUS_RED);
    }
}
