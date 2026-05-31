use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewProduct {
    pub sku: String,
    pub name: String,
    pub category: Option<String>,
    pub unit: String,
    pub min_stock_level: f64,
    pub notes: Option<String>,
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
}

#[derive(Debug, Clone)]
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
