use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub preferred_supplier_id: Option<i64>,
    pub created_at: String,
    pub model_number: Option<String>,
    pub part_number: Option<String>,
    pub duty_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProduct {
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub preferred_supplier_id: Option<i64>,
    pub model_number: Option<String>,
    pub part_number: Option<String>,
    pub duty_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockEntry {
    pub id: i64,
    pub product_id: i64,
    pub quantity: f64,
    pub unit_cost_usd: f64,
    pub supplier: Option<String>,
    pub acquired_at: String,
    pub acquisition_type: AcquisitionType,
    pub project_id: Option<i64>,
    pub invoice_ref: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionType {
    Project,
    Restock,
    Adjustment, // manual stock correction; quantity may be negative
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewStockEntry {
    pub product_id: i64,
    pub quantity: f64,
    pub unit_cost_usd: f64,
    pub supplier: Option<String>,
    pub acquired_at: String,
    pub acquisition_type: AcquisitionType,
    pub project_id: Option<i64>,
    pub invoice_ref: Option<String>,
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_description_is_optional() {
        let p = Product {
            id: 1,
            sku: "CAM-001".into(),
            name: "IP Camera".into(),
            category: None,
            unit: "pcs".into(),
            min_stock_level: 0.0,
            description: Some("Wide-angle, 24mm".into()),
            notes: None,
            preferred_supplier_id: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            model_number: None,
            part_number: None,
            duty_percent: 0.0,
        };
        assert_eq!(p.description.as_deref(), Some("Wide-angle, 24mm"));
    }

    #[test]
    fn new_product_description_is_optional() {
        let np = NewProduct {
            sku: "CAM-001".into(),
            name: "IP Camera".into(),
            category: None,
            unit: "pcs".into(),
            min_stock_level: 0.0,
            description: None,
            notes: None,
            preferred_supplier_id: None,
            model_number: None,
            part_number: None,
            duty_percent: 0.0,
        };
        assert!(np.description.is_none());
    }
}
