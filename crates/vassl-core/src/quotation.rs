use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quotation {
    pub id: i64,
    pub project_id: i64,
    pub reference_number: String,
    pub status: QuotationStatus,
    pub notes: Option<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotationStatus {
    Draft,
    Sent,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotationItem {
    pub id: i64,
    pub quotation_id: i64,
    pub product_id: Option<i64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price_usd: f64,
    pub total_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewQuotationItem {
    pub quotation_id: i64,
    pub product_id: Option<i64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price_usd: f64,
    pub total_usd: f64,
}
