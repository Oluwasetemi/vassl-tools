use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, rgb};

use crate::actions::{OpenInventory, OpenPriceBook, OpenQuotations};
use crate::colors;
use crate::sidebar::{ActiveModule, Sidebar};
use crate::status_bar::StatusBar;

pub struct VasslRoot {
    sidebar:    Entity<Sidebar>,
    status_bar: Entity<StatusBar>,
}

impl VasslRoot {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            sidebar:    cx.new(Sidebar::new),
            status_bar: cx.new(StatusBar::new),
        }
    }
}

impl Render for VasslRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("VasslRoot")
            .on_action(cx.listener(|this, _: &OpenInventory, _w, cx| {
                this.sidebar.update(cx, |s, cx| {
                    s.active = ActiveModule::Inventory;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &OpenQuotations, _w, cx| {
                this.sidebar.update(cx, |s, cx| {
                    s.active = ActiveModule::Quotations;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &OpenPriceBook, _w, cx| {
                this.sidebar.update(cx, |s, cx| {
                    s.active = ActiveModule::PriceBook;
                    cx.notify();
                });
            }))
            // TODO(Plan 5): add on_action handlers for OpenAuditLog, NewRecord, FocusSearch
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(colors::CANVAS_BG))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .child("pane area — Tasks 2-4"),
                    ),
            )
            .child(self.status_bar.clone())
    }
}
