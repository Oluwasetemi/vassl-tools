use gpui::{Context, IntoElement, Render, Window, div, prelude::*};

pub struct InventoryPanel;

impl InventoryPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for InventoryPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_1().h_full().child("Inventory — loading…")
    }
}
