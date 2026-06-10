use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::{Project, QuotationExtras, QuotationItem, QuotationStatus};
use vassl_inventory::InventoryStoreHandle;

use crate::db::{QuotationDb, QuotationRow};

#[derive(Debug, Clone)]
pub struct QuotationContextMenu {
    pub quotation_id:  i64,
    pub reference_num: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct ProjectContextMenu {
    pub project_id:   i64,
    pub project_name: String,
    pub x: f32,
    pub y: f32,
}

pub struct QuotationStore {
    pub quotations:               Vec<QuotationRow>,
    pub selected_id:              Option<i64>,
    pub line_items:               Vec<QuotationItem>,
    pub selected_extras:          Option<QuotationExtras>,
    pub projects:                 Vec<Project>,
    pub loading:                  bool,
    pub context_menu_quotation:   Option<QuotationContextMenu>,
    pub context_menu_project:     Option<ProjectContextMenu>,
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
            quotations:             Vec::new(),
            selected_id:            None,
            line_items:             Vec::new(),
            selected_extras:        None,
            projects:               Vec::new(),
            loading:                false,
            context_menu_quotation: None,
            context_menu_project:   None,
        }
    }

    pub fn load_quotations(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let db2 = db.clone();
            // Spawn both before awaiting either so they run concurrently.
            let quot_task = cx.background_executor().spawn(async move { db.list_quotations_with_project() });
            let proj_task = cx.background_executor().spawn(async move { db2.list_projects() });
            let quot_result = quot_task.await;
            let proj_result = proj_task.await;

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
        self.selected_id     = Some(id);
        self.line_items      = Vec::new();
        self.selected_extras = None;
        cx.notify();

        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let db2    = db.clone();
            let items  = cx.background_executor().spawn(async move { db.list_items_for_quotation(id) }).await;
            let extras = cx.background_executor().spawn(async move { db2.get_quotation_extras(id) }).await;
            let _ = this.update(cx, |store, cx| {
                match items {
                    Ok(i)  => { store.line_items = i; cx.emit(QuotationEvent::ItemsLoaded); }
                    Err(e) => tracing::error!("list_items_for_quotation failed: {e:?}"),
                }
                match extras {
                    Ok(e)  => { store.selected_extras = Some(e); }
                    Err(e) => tracing::error!("get_quotation_extras failed: {e:?}"),
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn select_next(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        if self.quotations.is_empty() { return None; }
        let cur = self.selected_id
            .and_then(|id| self.quotations.iter().position(|q| q.id == id));
        let next = match cur { None => 0, Some(i) => (i + 1).min(self.quotations.len() - 1) };
        self.select_quotation(self.quotations[next].id, cx);
        Some(next)
    }

    pub fn select_prev(&mut self, cx: &mut Context<Self>) -> Option<usize> {
        if self.quotations.is_empty() { return None; }
        let cur = self.selected_id
            .and_then(|id| self.quotations.iter().position(|q| q.id == id));
        let next = match cur { None => 0, Some(0) => 0, Some(i) => i - 1 };
        self.select_quotation(self.quotations[next].id, cx);
        Some(next)
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
        let db        = QuotationDb::global(&**cx);
        let inv_store = cx.global::<InventoryStoreHandle>().0.clone();
        let is_accept = new_status == QuotationStatus::Accepted;
        cx.spawn(async move |this, cx| {
            let result = if is_accept {
                db.accept_quotation(id).await
            } else {
                db.update_status(id, new_status).await
            };
            if let Err(e) = result {
                tracing::error!("transition_status failed: {e:?}");
                return Ok::<(), anyhow::Error>(());
            }
            let _ = this.update(cx, |store, cx| {
                store.load_quotations(cx);
                if is_accept {
                    let _ = inv_store.update(cx, |s, cx| s.load_products(cx));
                }
            });
            Ok(())
        }).detach();
    }

    pub fn set_quotation_context_menu(&mut self, target: QuotationContextMenu, cx: &mut Context<Self>) { self.context_menu_quotation = Some(target); cx.notify(); }
    pub fn clear_quotation_context_menu(&mut self, cx: &mut Context<Self>) { self.context_menu_quotation = None; cx.notify(); }
    pub fn set_project_context_menu(&mut self, target: ProjectContextMenu, cx: &mut Context<Self>) { self.context_menu_project = Some(target); cx.notify(); }
    pub fn clear_project_context_menu(&mut self, cx: &mut Context<Self>) { self.context_menu_project = None; cx.notify(); }

    pub fn delete_quotation(&mut self, quotation_id: i64, cx: &mut Context<Self>) {
        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.delete_quotation(quotation_id).await;
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(_) => {
                        store.quotations.retain(|q| q.id != quotation_id);
                        if store.selected_id == Some(quotation_id) {
                            store.selected_id = None;
                            store.line_items.clear();
                            store.selected_extras = None;
                        }
                        cx.notify();
                    }
                    Err(e) => tracing::error!("delete_quotation failed: {e:?}"),
                }
            });
        }).detach();
    }

    pub fn delete_project(&mut self, project_id: i64, cx: &mut Context<Self>) {
        let db = QuotationDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = db.delete_project(project_id).await;
            let _ = this.update(cx, |store, cx| {
                match result {
                    Ok(_) => {
                        store.projects.retain(|p| p.id != project_id);
                        store.quotations.retain(|q| q.project_id != project_id);
                        if let Some(sel_id) = store.selected_id {
                            if !store.quotations.iter().any(|q| q.id == sel_id) {
                                store.selected_id = None;
                                store.line_items.clear();
                                store.selected_extras = None;
                            }
                        }
                        cx.notify();
                    }
                    Err(e) => tracing::error!("delete_project failed: {e:?}"),
                }
            });
        }).detach();
    }

    pub fn delete_item(&mut self, item_id: i64, cx: &mut Context<Self>) {
        let quotation_id = match self.selected_id {
            Some(id) => id,
            None     => return,
        };
        let db        = QuotationDb::global(&**cx);
        let inv_store = cx.global::<InventoryStoreHandle>().0.clone();
        cx.spawn(async move |this, cx| {
            if let Err(e) = db.delete_item(item_id).await {
                tracing::error!("delete_item failed: {e:?}");
                return Ok::<(), anyhow::Error>(());
            }
            let _ = this.update(cx, |store, cx| {
                store.load_line_items(quotation_id, cx);
                store.load_quotations(cx);
                let _ = inv_store.update(cx, |s, cx| s.load_products(cx));
            });
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
            quotations:             vec![],
            selected_id:            None,
            line_items:             vec![],
            selected_extras:        None,
            projects:               vec![],
            loading:                false,
            context_menu_quotation: None,
            context_menu_project:   None,
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
