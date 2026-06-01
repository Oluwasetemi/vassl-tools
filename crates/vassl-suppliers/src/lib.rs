pub mod db;
pub mod panel;
pub mod store;
pub mod supplier_form;
pub mod supplier_list;

use gpui::{App, AppContext as _, Entity};

pub use db::SupplierDb;
pub use store::{SupplierStore, SupplierStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<SupplierStore> = cx.new(SupplierStore::new);
    cx.set_global(SupplierStoreHandle(store));
}
