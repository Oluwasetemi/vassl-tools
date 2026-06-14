pub mod colors;
pub mod db;
pub mod line_item_form;
pub mod panel;
pub mod project_form;
pub mod project_list;
pub mod project_panel;
pub mod quotation_detail;
pub mod quotation_form;
pub mod quotation_list;
pub mod store;

use gpui::{App, AppContext, Entity};

pub use db::QuotationDb;
pub use project_panel::ProjectPanel;
pub use store::{QuotationStore, QuotationStoreHandle};

pub fn init(cx: &mut App) {
    let store: Entity<QuotationStore> = cx.new(QuotationStore::new);
    cx.set_global(QuotationStoreHandle(store));
}
