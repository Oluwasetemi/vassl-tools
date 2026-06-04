use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceEntry {
    pub id: i64,
    pub product_id: i64,
    pub quantity: f64,
    pub cost_price_usd: f64,
    pub duty_cost_usd: f64,
    pub markup_percent: f64,
    pub selling_price_usd: f64,
    pub effective_date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPriceEntry {
    pub product_id: i64,
    pub cost_price_usd: f64,
    pub duty_cost_usd: f64,
    pub markup_percent: f64,
    pub effective_date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Error)]
pub enum PriceEntryError {
    #[error("markup_percent must be > 0, got {0}")]
    InvalidMarkup(f64),
    #[error("cost_price_usd must be >= 0, got {0}")]
    InvalidCostPrice(f64),
    #[error("duty_cost_usd must be >= 0, got {0}")]
    InvalidDuty(f64),
}

pub fn selling_price(cost: f64, duty: f64, markup_percent: f64) -> Result<f64, PriceEntryError> {
    if markup_percent <= 0.0 {
        return Err(PriceEntryError::InvalidMarkup(markup_percent));
    }
    if cost < 0.0 {
        return Err(PriceEntryError::InvalidCostPrice(cost));
    }
    if duty < 0.0 {
        return Err(PriceEntryError::InvalidDuty(duty));
    }
    Ok((cost + duty) * (1.0 + markup_percent / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selling_price_default_markup() {
        let price = selling_price(100.0, 10.0, 30.0).unwrap();
        assert!((price - 143.0).abs() < 1e-10);
    }

    #[test]
    fn selling_price_zero_duty() {
        let price = selling_price(200.0, 0.0, 30.0).unwrap();
        assert!((price - 260.0).abs() < 1e-10);
    }

    #[test]
    fn selling_price_rejects_zero_markup() {
        assert!(matches!(
            selling_price(100.0, 0.0, 0.0),
            Err(PriceEntryError::InvalidMarkup(_))
        ));
    }

    #[test]
    fn selling_price_rejects_negative_cost() {
        assert!(matches!(
            selling_price(-1.0, 0.0, 30.0),
            Err(PriceEntryError::InvalidCostPrice(_))
        ));
    }

    #[test]
    fn selling_price_rejects_negative_duty() {
        assert!(matches!(
            selling_price(100.0, -1.0, 30.0),
            Err(PriceEntryError::InvalidDuty(_))
        ));
    }
}
