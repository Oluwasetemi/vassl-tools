pub mod colors;
pub mod db;
pub mod panel;
pub mod price_form;
pub mod price_table;
pub mod store;

use gpui::{App, AppContext, Entity};

pub use db::PriceBookDb;
pub use store::{PriceBookStore, PriceBookStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<PriceBookStore> = cx.new(PriceBookStore::new);
    cx.set_global(PriceBookStoreHandle(store));
}
