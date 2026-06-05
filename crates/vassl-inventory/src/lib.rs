pub mod colors;
pub mod db;
pub mod importer;
pub mod panel;
pub mod product_form;
pub mod product_list;
pub mod restock;
pub mod stock_form;
pub mod store;

use gpui::{App, AppContext, Entity};

pub use db::InventoryDb;
pub use store::{InventoryStore, InventoryStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<InventoryStore> = cx.new(InventoryStore::new);
    cx.set_global(InventoryStoreHandle(store));
}
