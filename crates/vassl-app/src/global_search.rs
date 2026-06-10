use gpui::{Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
           div, prelude::*, px, rems, rgb, rgba};
use vassl_core::{Supplier, Project};
use vassl_ui::{TextInput, ThemeHandle, text_field};

use vassl_inventory::store::{InventoryStoreHandle, ProductWithStock};
use vassl_quotations::store::QuotationStoreHandle;
use vassl_suppliers::store::SupplierStoreHandle;

use crate::actions::{ConfirmSelection, EscapeModal, SelectNext, SelectPrev};
use crate::sidebar::ActiveModule;

#[derive(Clone, Debug, PartialEq)]
pub enum SearchResultKind {
    Product  { id: i64, sku: String },
    Supplier { id: i64, email: Option<String> },
    Project  { id: i64, client: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub module: ActiveModule,
    pub kind:   SearchResultKind,
    pub label:  String,
    pub sub:    String,
}

pub enum GlobalSearchEvent {
    Dismissed,
    Navigate(SearchHit),
}
impl EventEmitter<GlobalSearchEvent> for GlobalSearch {}

pub struct GlobalSearch {
    pub query:    gpui::Entity<TextInput>,
    selected_idx: usize,
    focus_handle: FocusHandle,
}

pub fn build_hits(
    query:     &str,
    products:  &[ProductWithStock],
    suppliers: &[Supplier],
    projects:  &[Project],
) -> Vec<SearchHit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() { return vec![]; }

    let mut hits = Vec::new();

    for p in products {
        if p.product.name.to_lowercase().contains(&q)
            || p.product.sku.to_lowercase().contains(&q)
        {
            hits.push(SearchHit {
                module: ActiveModule::Inventory,
                kind:   SearchResultKind::Product { id: p.product.id, sku: p.product.sku.clone() },
                label:  p.product.name.clone(),
                sub:    p.product.sku.clone(),
            });
        }
    }

    for s in suppliers {
        if s.name.to_lowercase().contains(&q)
            || s.email.as_ref().map(|e| e.to_lowercase().contains(&q)).unwrap_or(false)
        {
            hits.push(SearchHit {
                module: ActiveModule::Suppliers,
                kind:   SearchResultKind::Supplier { id: s.id, email: s.email.clone() },
                label:  s.name.clone(),
                sub:    s.email.clone().unwrap_or_default(),
            });
        }
    }

    for p in projects {
        if p.name.to_lowercase().contains(&q)
            || p.client_name.to_lowercase().contains(&q)
        {
            hits.push(SearchHit {
                module: ActiveModule::Quotations,
                kind:   SearchResultKind::Project { id: p.id, client: p.client_name.clone() },
                label:  p.name.clone(),
                sub:    p.client_name.clone(),
            });
        }
    }

    hits.truncate(50);
    hits
}

impl GlobalSearch {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            query:        cx.new(|cx| TextInput::with_placeholder("Search products, suppliers, projects…", cx)),
            selected_idx: 0,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for GlobalSearch {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for GlobalSearch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.global::<ThemeHandle>().0.clone();

        let query_text = self.query.read(cx).text().to_string();
        let hits = {
            let inv  = cx.global::<InventoryStoreHandle>().0.read(cx);
            let sup  = cx.global::<SupplierStoreHandle>().0.read(cx);
            let quot = cx.global::<QuotationStoreHandle>().0.read(cx);
            build_hits(&query_text, &inv.products, &sup.suppliers, &quot.projects)
        };

        if self.selected_idx >= hits.len() && !hits.is_empty() {
            self.selected_idx = hits.len() - 1;
        }

        let query_focused = self.query.read(cx).focus_handle.is_focused(window);

        div()
            .absolute().top_0().left_0().right_0().bottom_0()
            .flex().justify_center().pt(px(100.))
            .bg(rgba(0x00000099))
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_, _, _, cx| {
                cx.emit(GlobalSearchEvent::Dismissed);
            }))
            .child(
                div()
                    .id("global-search-popup")
                    .key_context("GlobalSearch")
                    .on_action(cx.listener(|_, _: &EscapeModal, _, cx| {
                        cx.emit(GlobalSearchEvent::Dismissed);
                    }))
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                        let hits = build_hits(
                            this.query.read(cx).text(),
                            &cx.global::<InventoryStoreHandle>().0.read(cx).products,
                            &cx.global::<SupplierStoreHandle>().0.read(cx).suppliers,
                            &cx.global::<QuotationStoreHandle>().0.read(cx).projects,
                        );
                        let max = hits.len().saturating_sub(1);
                        if this.selected_idx < max { this.selected_idx += 1; cx.notify(); }
                    }))
                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                        if this.selected_idx > 0 { this.selected_idx -= 1; cx.notify(); }
                    }))
                    .on_action(cx.listener(|this, _: &ConfirmSelection, _, cx| {
                        let hits = build_hits(
                            this.query.read(cx).text(),
                            &cx.global::<InventoryStoreHandle>().0.read(cx).products,
                            &cx.global::<SupplierStoreHandle>().0.read(cx).suppliers,
                            &cx.global::<QuotationStoreHandle>().0.read(cx).projects,
                        );
                        if let Some(hit) = hits.into_iter().nth(this.selected_idx) {
                            cx.emit(GlobalSearchEvent::Navigate(hit));
                        }
                    }))
                    .w(px(520.))
                    .bg(rgb(c.canvas_bg)).rounded(px(8.)).p(px(12.))
                    .flex().flex_col().gap(px(8.))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                    .child(text_field("", self.query.clone(), query_focused, false, cx))
                    .child({
                        let results = div()
                            .id("global-search-results")
                            .flex().flex_col().gap(px(2.))
                            .max_h(px(360.)).overflow_y_scroll();

                        if query_text.trim().is_empty() {
                            results.child(
                                div().px(px(10.)).py(px(8.))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                    .child("Type to search products, suppliers, and projects.")
                            )
                        } else if hits.is_empty() {
                            results.child(
                                div().px(px(10.)).py(px(8.))
                                    .text_size(rems(0.923)).text_color(rgb(c.text_muted))
                                    .child(format!("No results for \"{}\".", query_text.trim()))
                            )
                        } else {
                            let selected_idx = self.selected_idx;
                            let hover_bg     = rgb(c.surface_hover);
                            results.children(hits.iter().enumerate().map(|(idx, hit)| {
                                let selected  = idx == selected_idx;
                                let bg        = if selected { c.surface_active } else { c.surface_default };
                                let hit_clone = hit.clone();
                                let module_label = match hit.module {
                                    ActiveModule::Inventory  => "Product",
                                    ActiveModule::Suppliers  => "Supplier",
                                    ActiveModule::Quotations => "Project",
                                    _                        => "",
                                };
                                div()
                                    .id(format!("gs-item-{idx}"))
                                    .px(px(10.)).py(px(7.)).rounded(px(4.))
                                    .bg(rgb(bg))
                                    .when(!selected, |d| d.hover(move |s| s.bg(hover_bg)))
                                    .flex().flex_row().items_center().gap(px(8.))
                                    .cursor_pointer()
                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |_, _, _, cx| {
                                        cx.emit(GlobalSearchEvent::Navigate(hit_clone.clone()));
                                    }))
                                    .child(
                                        div().w(px(56.)).text_size(rems(0.769))
                                            .text_color(rgb(c.text_muted))
                                            .child(module_label)
                                    )
                                    .child(
                                        div().flex_1().text_size(rems(1.)).text_color(rgb(c.text_default))
                                            .child(hit.label.clone())
                                    )
                                    .child(
                                        div().text_size(rems(0.846)).text_color(rgb(c.text_muted))
                                            .child(hit.sub.clone())
                                    )
                            }))
                        }
                    })
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vassl_core::{Product, ProjectStatus};
    use vassl_inventory::store::StockStatus;

    fn make_pws(id: i64, sku: &str, name: &str) -> ProductWithStock {
        ProductWithStock {
            product: Product {
                id, sku: sku.into(), name: name.into(),
                category: None, unit: "pcs".into(),
                min_stock_level: 0.0, description: None, notes: None,
                preferred_supplier_id: None,
                created_at: "2026-01-01T00:00:00Z".into(),
                model_number: None, part_number: None, duty_percent: 0.0,
            },
            current_stock: 5.0,
            status: StockStatus::Healthy,
        }
    }

    fn make_supplier(id: i64, name: &str, email: Option<&str>) -> Supplier {
        Supplier { id, name: name.into(), contact_person: None,
            email: email.map(String::from), phone: None, notes: None,
            created_at: "2026-01-01T00:00:00Z".into() }
    }

    fn make_project(id: i64, name: &str, client: &str) -> Project {
        Project { id, name: name.into(), client_name: client.into(),
                  description: None, status: ProjectStatus::Active,
                  created_at: "2026-01-01T00:00:00Z".into(),
            client_address: Some("no1, address test".into()),
            client_attn: Some("client atten".into()),
            client_tel: Some("123456789".into()) }
    }

    #[test]
    fn empty_query_returns_empty_vec() {
        let hits = build_hits("", &[make_pws(1, "CAM-001", "Camera")], &[], &[]);
        assert!(hits.is_empty());
    }

    #[test]
    fn matches_product_by_name() {
        let hits = build_hits("camera", &[make_pws(1, "CAM-001", "IP Camera")], &[], &[]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label, "IP Camera");
        assert_eq!(hits[0].module, ActiveModule::Inventory);
    }

    #[test]
    fn matches_product_by_sku() {
        let hits = build_hits("cam-", &[make_pws(1, "CAM-001", "IP Camera"), make_pws(2, "NVR-001", "NVR")], &[], &[]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].sub, "CAM-001");
    }

    #[test]
    fn matches_supplier_by_name() {
        let hits = build_hits("acme", &[], &[make_supplier(1, "Acme Ltd", Some("a@acme.com"))], &[]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].module, ActiveModule::Suppliers);
    }

    #[test]
    fn matches_project_by_client() {
        let hits = build_hits("sony", &[], &[], &[make_project(1, "CCTV Install", "Sony Corp")]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].module, ActiveModule::Quotations);
    }

    #[test]
    fn max_50_hits() {
        let products: Vec<_> = (0..60).map(|i| make_pws(i, "SKU", "Camera")).collect();
        let hits = build_hits("camera", &products, &[], &[]);
        assert_eq!(hits.len(), 50);
    }

    #[test]
    fn global_search_event_navigate_carries_hit() {
        let hit = SearchHit {
            module: ActiveModule::Inventory,
            kind:   SearchResultKind::Product { id: 1, sku: "CAM-001".into() },
            label:  "Camera".into(),
            sub:    "CAM-001".into(),
        };
        let ev = GlobalSearchEvent::Navigate(hit.clone());
        assert!(matches!(ev, GlobalSearchEvent::Navigate(_)));
    }
}
