use gpui::{Context, Entity, EventEmitter, Global};
use vassl_core::Supplier;
use crate::db::SupplierDb;

pub struct SupplierStore {
    pub suppliers:            Vec<Supplier>,
    pub selected_supplier_id: Option<i64>,
    pub loading:              bool,
}

pub struct SupplierStoreHandle(pub Entity<SupplierStore>);
impl Global for SupplierStoreHandle {}

#[derive(Debug)]
pub enum SupplierEvent { SuppliersLoaded }
impl EventEmitter<SupplierEvent> for SupplierStore {}

impl SupplierStore {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { suppliers: Vec::new(), selected_supplier_id: None, loading: false }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplier_event_loaded_variant() {
        let ev = SupplierEvent::SuppliersLoaded;
        assert!(matches!(ev, SupplierEvent::SuppliersLoaded));
    }
}
