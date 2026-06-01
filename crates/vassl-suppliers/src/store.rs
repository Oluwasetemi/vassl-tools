use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::Supplier;
use crate::db::SupplierDb;

pub struct SupplierStore {
    pub suppliers:            Vec<Supplier>,
    pub selected_supplier_id: Option<i64>,
    pub loading:              bool,
    pub search_query:         String,
}

pub struct SupplierStoreHandle(pub Entity<SupplierStore>);
impl Global for SupplierStoreHandle {}

#[derive(Debug)]
pub enum SupplierEvent { SuppliersLoaded }
impl EventEmitter<SupplierEvent> for SupplierStore {}

impl SupplierStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { suppliers: Vec::new(), selected_supplier_id: None, loading: false, search_query: String::new() }
    }

    pub fn load_suppliers(&mut self, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        cx.notify();

        let db = SupplierDb::global(&**cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor()
                .spawn(async move { db.list_suppliers() })
                .await;
            let _ = this.update(cx, |store, cx| {
                store.loading = false;
                match result {
                    Ok(rows) => {
                        store.suppliers = rows;
                        cx.emit(SupplierEvent::SuppliersLoaded);
                    }
                    Err(e) => tracing::error!("load_suppliers failed: {e:?}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select_supplier(&mut self, id: i64, cx: &mut Context<Self>) {
        self.selected_supplier_id = Some(id);
        cx.notify();
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.search_query == query { return; }
        self.search_query = query;
        cx.notify();
    }

    pub fn filtered_suppliers(&self) -> Vec<&Supplier> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            return self.suppliers.iter().collect();
        }
        self.suppliers.iter().filter(|s| {
            s.name.to_lowercase().contains(&q)
                || s.email.as_ref().map(|e| e.to_lowercase().contains(&q)).unwrap_or(false)
                || s.phone.as_ref().map(|p| p.to_lowercase().contains(&q)).unwrap_or(false)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_supplier_entry(id: i64, name: &str, email: Option<&str>) -> Supplier {
        Supplier {
            id, name: name.into(),
            contact_person: None,
            email: email.map(String::from),
            phone: None, notes: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn filtered_suppliers_empty_query_returns_all() {
        let store = SupplierStore {
            suppliers: vec![
                make_supplier_entry(1, "Acme Ltd",  Some("a@acme.com")),
                make_supplier_entry(2, "Beta Corp", None),
            ],
            selected_supplier_id: None,
            loading: false,
            search_query: String::new(),
        };
        assert_eq!(store.filtered_suppliers().len(), 2);
    }

    #[test]
    fn filtered_suppliers_matches_name() {
        let store = SupplierStore {
            suppliers: vec![
                make_supplier_entry(1, "Acme Ltd",  Some("a@acme.com")),
                make_supplier_entry(2, "Beta Corp", None),
            ],
            selected_supplier_id: None,
            loading: false,
            search_query: "acme".into(),
        };
        assert_eq!(store.filtered_suppliers().len(), 1);
        assert_eq!(store.filtered_suppliers()[0].name, "Acme Ltd");
    }

    #[test]
    fn filtered_suppliers_matches_email() {
        let store = SupplierStore {
            suppliers: vec![make_supplier_entry(1, "Acme Ltd", Some("orders@acme.com"))],
            selected_supplier_id: None,
            loading: false,
            search_query: "orders".into(),
        };
        assert_eq!(store.filtered_suppliers().len(), 1);
    }

    #[test]
    fn supplier_event_loaded_variant() {
        let ev = SupplierEvent::SuppliersLoaded;
        assert!(matches!(ev, SupplierEvent::SuppliersLoaded));
    }
}
