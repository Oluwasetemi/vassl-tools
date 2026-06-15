use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quotation {
    pub id: i64,
    pub project_id: i64,
    pub reference_number: String,
    pub status: QuotationStatus,
    pub notes: Option<String>,
    pub quotation_date: Option<String>,
    pub exchange_rate_jmd: f64,
    pub discount_percent: f64,
    pub gct_percent: f64,
    pub validity_days: i64,
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
    pub unit: Option<String>,
    pub unit_price_usd: f64,
    pub discount_percent: f64,
    pub total_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewQuotationItem {
    pub quotation_id: i64,
    pub product_id: Option<i64>,
    pub description: String,
    pub quantity: f64,
    pub unit: Option<String>,
    pub unit_price_usd: f64,
    pub discount_percent: f64,
    pub total_usd: f64,
}

/// Quotation-level financial settings fetched for the detail view.
#[derive(Debug, Clone)]
pub struct QuotationExtras {
    pub exchange_rate_jmd: f64,
    pub discount_percent: f64,
    pub gct_percent: f64,
    pub validity_days: i64,
    pub quotation_date: Option<String>,
}

impl Default for QuotationExtras {
    fn default() -> Self {
        Self {
            exchange_rate_jmd: 156.0,
            discount_percent: 0.0,
            gct_percent: 15.0,
            validity_days: 30,
            quotation_date: None,
        }
    }
}
