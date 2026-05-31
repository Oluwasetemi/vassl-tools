pub mod db;
pub mod store;        // stub — populated in Task 2
pub mod panel;        // stub — populated in Task 5
pub mod product_list; // stub — populated in Task 4
pub mod stock_form;   // stub — populated in Task 6
pub mod restock;      // stub — populated in Task 5

use gpui::App;

pub use db::InventoryDb;

pub fn init(cx: &mut App) {
    // InventoryStore created in Task 2 and registered here
    let _ = cx;
}
